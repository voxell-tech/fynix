# Plans

| Area                                 | Status                            |
|--------------------------------------|-----------------------------------|
| Unit system (`src/unit.rs`)          | Planned, not started              |
| Interactions & Events                | Planned, not started              |
| Reactive scopes (`ctx.reactive`)     | Implemented                       |
| Field bindings (`Bindings`)          | Planned, not started              |

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

Registered via `inventory::submit!` / `inventory::iter`.

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

`Events` is Fynix-owned, backed by `TypePool`. Handlers push
typed messages; the backend drains them with full world access:

```rust
fn process(fynix: Res<Fynix>, mut commands: Commands) {
    for _ in fynix.events.drain::<SpawnEnemy>() {
        commands.spawn(Enemy::default());
    }
}

// Example consumer system.
fn system(events: Res<Events>, bindings: ResMut<Bindings>) {
    for msg in events.iter::<Increment>() {}

    for msg in events.iter::<Decrement>() {
        bindings.set(..);
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

## Bindings

A binding connects an external value to an element field. `BindingId`
is a plain type alias (same pattern as `ElementId`), not a typed
wrapper - type safety comes from the accessor at binding time.

`Fynix` gains a `bindings: Bindings` field:

```
Bindings
- values: TypePool<BindingId>
    - current value per (BindingId, T)
- targets: HashMap<BindingId, (ElementId, UntypedAccessor)>
    - which element field each binding writes to
- dirty: HashSet<BindingId>
    - changed since last flush
- layout_dirty: HashSet<ElementId>
    - populated during flush, consumed by the backend
- render_dirty: HashSet<ElementId>
    - populated during flush, consumed by the backend
- id_generator: BindingIdGenerator
```

**Field bindings** - bind a single element field to an external value.
Flushing writes the value directly into `Elements` in-place, then
marks the element layout/render dirty:

```rust
let label_text: BindingId = ctx.bind(
    field_accessor!(<Label>::text),
    "hello".to_string(),
);

// From outside Fynix:
fynix.bindings.set(label_text, "world".to_string());
```

A `flush_bindings()` would be called by the backend each frame to
apply field bindings in-place, limiting re-layout to the affected
subtree.

> **Reactive scopes** (the change-detection counterpart) are already
> implemented as `ctx.reactive(changed_fn, build_fn)` +
> `Fynix::update_scopes::<W>(world)`, backed by `Scopes` / `Scope` /
> `ScopeElement` and `Elements::dirty_elements`. The remaining
> dirty-propagation and depth-ordering work is tracked in issue #38.
> See `crates/fynix/src/scope.rs`.

