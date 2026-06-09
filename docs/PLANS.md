# Plans

| Area                                 | Status                            |
|--------------------------------------|-----------------------------------|
| Unit system (`src/unit.rs`)          | Planned, not started              |
| Interaction input layer              | Planned, design below             |
| Focus system                         | Planned, design below             |
| Field bindings (`Bindings`)          | Planned, not started              |

---

## Interaction input layer

Per-instance handlers (`ctx.add::<E>().on::<I>(..)`,
`HandlerFn<I> = fn(I, &mut Events)` over `TypeTable<ElementId>`) and
the `Events` queue are implemented. What remains is feeding platform
input into those handlers.

The input layer turns raw platform input into the semantic
interaction types that handlers consume. Three stages:

```
backend (winit, etc.)     fynix_input (stateful)      fynix-core
─────────────────────     ──────────────────────      ──────────
RawInput               →  recognizers + focus      →  dispatch::<I>(id, I)
```

**RawInput.** A fynix-owned enum the backend produces: the only
thing each backend must emit. `PointerMoved`, `PointerDown/Up
{ button }`, `Scroll`, `Key { code, state, mods }`, `Text`,
`ImePreedit/Commit`, `Touch { id, phase, pos }`, `Gamepad`. Mouse
and touch both lower into a unified pointer model here (mouse is
pointer id 0, each finger its own id), so everything downstream is
pointer-agnostic. Every event carries a backend-supplied monotonic
timestamp, because no_std has no clock and recognizers need timing.

**fynix-core primitives** (need the element tree, so they live in
core):

- Hit-testing. Given a pointer position, walk the tree in reverse
  paint order accumulating each `RectNode`'s local `translation`
  into an absolute rect, return the deepest hit as `ElementId` plus
  local point. Confirm whether rectree already exposes an
  absolute/global rect or `NodeState` before re-accumulating.
- Bubbling dispatch. `dispatch_bubbling::<I>(start_id, I)` walks up
  via `meta.node.parent_id` and delivers to the nearest ancestor
  with a handler for `I`, then stops. Because `HandlerFn` has no
  return value, "nearest interested ancestor" replaces a
  consume/stop-propagation API. Needs a `contains::<I>(id)` probe
  on `Interactions` (a column lookup that does not run the handler).
  A return value or richer event-context arg is deferred until a
  real multi-listener need appears.
- Focus state. `FocusState { focused: Option<ElementId> }` plus a
  focusable registry. Elements opt in per instance, mirroring
  `.on()`, e.g. a `.focusable()` on `ElementHandle`.

**Recognizer layer** (stateful, no tree, likely a `fynix_input`
crate):

- Pointer / click. Tracks button-down position and timestamps to
  synthesize `Click` / `DoubleClick` / `TripleClick` within time and
  distance thresholds. Emits `PointerEnter` / `PointerLeave` by
  diffing the current hit-test result against the previous frame.
- Drag + pointer capture. Once a drag starts the pointer is captured
  by the element that received the down, so subsequent moves and up
  route to it regardless of hit-testing. `captured: Option<(PointerId,
  ElementId)>`.
- Keyboard / text / IME. Route by focus, not position, bubbling if
  unhandled. IME also needs an outbound channel: when a text-input
  element gains focus, core asks the backend to enable IME and
  reports the caret rect derived from the element's absolute rect.
- Gamepad. Does not hit-test. Drives focus navigation (d-pad / stick
  moves focus to the next focusable, reusing the Tab traversal) and
  activation (A button dispatches `Activate`/`Click` to the focused
  element).

### Focus system (planned)

- Focusable registry built from per-instance opt-in, plus a
  traversal order. Tab order starts as in-order tree walk, later
  growing an explicit `tab_index`. Spatial navigation (gamepad /
  arrows) uses absolute rects to pick the nearest focusable in a
  direction.
- Focus changes emit `FocusGained(id)` / `FocusLost(id)` as ordinary
  interaction types through the same dispatch path.
- Focus-follows-click. On `PointerDown`, hit-test, and if the hit
  element is focusable set focus, emitting the gained/lost pair. This
  is the single bridge between the pointer world and the focus world.

Decided: input state lives in a new `fynix_input` crate holding the
recognizers and focus-input state, while fynix-core keeps only
hit-test, bubbling dispatch, and `FocusState`.

### Open decisions

1. Focus opt-in shape: `.focusable()` on `ElementHandle` (per
   instance, matches `.on()`) versus a trait/marker on the element
   type. Leaning per instance.
2. Bubbling now: start with `contains::<I>` plus nearest-ancestor
   delivery and no consume API, defer stop-propagation.
3. Time source: a `u64` monotonic stamp on every `RawInput`.

First end-to-end slice: `RawInput` enum + hit-testing + the
pointer/click recognizer + focus-follows-click, emitting `Click`
through the existing `dispatch`. Exercises every layer without IME
or gamepad.

---

## Bindings

A binding is an inline, in-place mutation of a single element, driven
by change detection. It is the lightweight counterpart to a reactive
scope: where a scope rebuilds a whole subtree when its source changes,
a binding writes directly into one element's fields and only marks
that element dirty.

### API

Bindings attach per instance through the same `ElementHandle` that
`.on` uses, mirroring the `ctx.reactive(changed, build)` shape:

```rust
ctx.add::<Label>().bind(
    |world| world.score_changed(),
    |world, label| label.text = world.score.to_string(),
);
```

- `changed: fn(&W) -> bool` reports whether the source state changed,
  exactly like a reactive scope's `changed`.
- `apply: fn(&W, &mut E)` writes the new value into this element's
  typed fields in place. No `BindingId`, no field accessor, no
  type-erased value column: the closure mutates the concrete element
  directly.

`ElementHandle` becomes `ElementHandle<'a, W, E>`, carrying a
`PhantomData<fn() -> (W, E)>` to encode both the world and element
types without owning them. That is what lets `.bind` enforce the
`apply` closure's `&mut E` against the element this handle was created
for, and `&W` against the ctx's world. (`fn() -> (W, E)` rather than
`(W, E)` keeps the handle neutral on variance and auto traits.)

### Captured data: `bind_with`

The closures are non-capturing fn pointers, so they cannot close over
per-binding state such as the ECS `Entity` whose component the binding
should read. `bind_with` carries that state explicitly: a `data: D`
value stored alongside the binding and passed by reference into both
closures.

```rust
ctx.add::<Label>().bind_with(
    entity, // data: D, stored with the binding
    |world, entity| world.score_changed(*entity),
    |world, entity, label| {
        label.text = world.score(*entity).to_string();
    },
);
```

- `changed: fn(&W, &D) -> bool`
- `apply: fn(&W, &D, &mut E)`

`.bind` is then just `bind_with` with `D = ()`. The binding store
keeps the owned `D` per entry (keyed by `ElementId`), so each pass
hands the closures a `&D` for free. `D: 'static`, consistent with the
rest of the type-keyed storage.

### Behavior

Each frame the backend runs a bindings pass (either a new
`Fynix::update_bindings::<W>(world)` or folded into the existing
`update_scopes` pass, since both are `(changed_fn, action)` keyed by
`ElementId`). For every binding whose `changed` reports true, `apply`
runs against the element fetched mutably from the pool, then the
element is marked dirty so only its subtree re-lays-out and repaints.

### Open questions

1. Whether bindings get their own store and pass, or share the scopes
   machinery. A scope's action rebuilds; a binding's action mutates in
   place, but the change-detection plumbing is identical.
2. Dirty granularity: a binding that touches only paint (e.g. a color)
   should mark render-dirty without forcing re-layout.

