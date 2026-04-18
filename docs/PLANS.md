# Plans

| Area                                 | Status                            |
|--------------------------------------|-----------------------------------|
| Unit system (`src/unit.rs`)          | Planned, not started              |
| Element composers                    | Ready to start                    |
| Interactions & Events                | Planned, not started              |
| Reactivity (`Signals`)               | Deferred until after first render |
| `TypeSlot` / typed table opt.        | In progress...                    |

---

## Element composers

`Composer<W>` is a trait separate from `Element`. Element structs
remain pure data; composition logic - how children are built for a
given world - lives in the `Composer<W>` impl. This keeps
`fynix_elements` decoupled from any backend.

```rust
pub trait Composer<W> {
    fn compose(self, ctx: &mut FynixCtx<W>) -> ElementId;
}
```

`compose` takes ownership of `self`: the struct is the input data,
consumed to build the subtree.

### Usage

```rust
struct Hierarchy<'a> {
    filter: &'a str,
}

// In fynix_bevy (or any backend crate):
impl Composer<BevyWorld> for Hierarchy<'_> {
    fn compose(self, ctx: &mut FynixCtx<BevyWorld>) -> ElementId {
        let items = ctx.world.query_filtered(self.filter);
        // ...
        ctx.add::<Label>()
    }
}

// In fynix_custom:
impl Composer<CustomWorld> for Hierarchy<'_> {
    fn compose(self, ctx: &mut FynixCtx<CustomWorld>) -> ElementId {
        ctx.add::<Label>()
    }
}
```

```rust
fn create_ui(ctx: &mut FynixCtx<BevyWorld>) {
    ctx.compose(Hierarchy { filter: "enemy" });
}
```

### Reactive re-composition

When a signal that scopes a composer's subtree changes, the old
children are removed and `compose` is re-called with fresh data.
See the reactive scopes section under Signals.

---

## Interactions & Events

Incoming user interactions are handled by world-agnostic handlers
registered via `#[fynix(interaction)]`. Outgoing messages from UI
to world flow through a Fynix-owned `Events` queue.

### `#[fynix(interaction)]` proc-macro

The element type is inferred from the first parameter, and the
interaction type from the second. No world - handlers are fully
world-agnostic.

```rust
#[fynix(interaction)]
fn on_click_button(
    e: &mut Button,
    interaction: Click,
    events: &mut Events,
) {
    (e.on_click)(events);
}
```

Registered into a `ELEMENT_INTERACTIONS` distributed slice via
`linkme`, same pattern as `ELEMENT_COMPOSERS`.

```rust
pub struct UntypedInteractionHandler {
    element_id: TypeId,
    interaction_id: TypeId,
    handler_fn: *const (),
}
```

Registry keyed on `(TypeId<E>, TypeId<Ev>)` - one handler per
element + interaction pair.

### Per-instance behavior via fn pointer

Elements store `fn(&mut Events)` set per-instance via `add_with`,
allowing different buttons to produce different events:

```rust
struct Button {
    label: String,
    on_click: fn(&mut Events),
}

ctx.add_with::<Button>(|e, ctx| {
    e.on_click = |events| events.push(SpawnEnemy::default());
});
```

### Events queue (UI -> world)

`Events` is Fynix-owned, backed by `TypeTable`. Handlers push
typed messages; the backend drains them with full world access:

```rust
fn process(fynix: Res<Fynix>, mut commands: Commands) {
    for _ in fynix.events.drain::<SpawnEnemy>() {
        commands.spawn(Enemy::default());
    }
}

// Example consumer system.
fn system(events: Res<Events>, signals: ResMut<Signals>) {
    for msg in events.iter::<Increment>() {}

    for msg in events.iter::<Decrement>() {
        signals.set(..);
    }
}

pub struct Increment;
pub struct Decrement;
```

### Registering interaction types

Built-in interactions: `Click`, `Drag`, `Hover`.

Custom interactions registered via a converter that maps raw input
to `Option<MyInteraction>`. The framework only runs a converter if
at least one element has a handler registered for that interaction
type.

---

## Signals

Signals are reactive bindings from an external value to an element
field. `SignalId` is a plain type alias (same pattern as `ElementId`),
not a typed wrapper - type safety comes from the accessor at binding
time.

`Fynix` gains a `signals: Signals` field:

```
Signals
- values: TypeTable<SignalId>
    - current value per (SignalId, T)
- targets: HashMap<SignalId, (ElementId, UntypedAccessor)>
    - which element field each signal writes to
- dirty: HashSet<SignalId>
    - changed since last flush
- layout_dirty: HashSet<ElementId>
    - populated during flush, consumed by the backend
- render_dirty: HashSet<ElementId>
    - populated during flush, consumed by the backend
- id_generator: SignalIdGenerator
```

Two kinds of signals:

**Field signals** - bind a signal to a single element field. Flushing
writes the value directly into `Elements` in-place, then marks the
element layout/render dirty:

```rust
let label_text: SignalId = ctx.signal(
    field_accessor!(<Label>::text),
    "hello".to_string(),
);

// From outside Fynix:
fynix.signals.set(label_text, "world".to_string());
```

**Reactive scopes** - bind a signal to a subtree builder closure.
When the signal changes, the old children of the scope root are
removed and the closure re-runs with fresh state:

```rust
ctx.add_with::<Vertical>(|v, ctx| {
    ctx.reactive(items_signal, |items, ctx| {
        for item in items {
            v.add(ctx.add::<Label>());
        }
    });
});
```

Scope entries are stored on `Fynix`:

```
reactive_scopes: HashMap<SignalId, Vec<ScopeEntry>>
    ScopeEntry { root: ElementId, rebuild: Box<dyn FnMut(...)> }
```

`flush_signals()` is called by the backend each frame. It applies
field signals in-place and re-runs any dirty scope builders, limiting
re-layout to the affected subtree in both cases.

---

## `TypeSlot` - typed table optimisation

This enables faster specialised tables for types known at link time:

- `ElementTable` - replaces `TypeTable<ElementId>` for element storage
- `InteractionTable` - replaces the interaction handler registry
- `EventTable` - replaces the outgoing events queue

**Style values remain on `TypeTable`** - style field values are
arbitrary user types (`f32`, `String`, custom structs) that cannot
be required to `#[derive(HasSlot)]`. The hashmap stays there. This is
debatable, we will see for the time being..
