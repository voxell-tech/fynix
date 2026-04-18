# Plans

| Area                                 | Status                            |
|--------------------------------------|-----------------------------------|
| Unit system (`src/unit.rs`)          | Planned, not started              |
| Element composers                    | Planned, not started              |
| Interactions & Events                | Planned, not started              |
| Reactivity (`Signals`)               | Deferred until after first render |
| `TypeSlot` / typed table opt.        | In progress...                    |

---

## Element composers

Element compose behavior is registered externally via
`ElementComposers`, keeping element structs as pure data and
backends decoupled from `fynix_elements`.

### Types

```rust
// Typed composer function for element E in world W.
pub type ElementComposerFn<E, W> = fn(&mut E, &mut FynixCtx<W>);

// Typed wrapper - monomorphized for (E, W).
pub struct ElementComposer<E: Element, W> { ... }

// Fully type-erased - stores TypeId for both E and W
// alongside a *const () function pointer.
// unsafe impl Sync - the pointer is always a fn pointer.
pub struct UntypedElementComposer {
    element_id: TypeId,
    world_id: TypeId,
    compose_fn: *const (),
}

// Non-generic registry keyed on element TypeId.
pub struct ElementComposers {
    composers: HashMap<TypeId, UntypedElementComposer>,
}
```

`UntypedElementComposer::new::<E, W>(f)` is `const` - both
`TypeId::of` calls are const-stable when `T: 'static`.

### Auto-registration via `linkme`

A `linkme` distributed slice collects composers across crates:

```rust
#[distributed_slice]
pub static ELEMENT_COMPOSERS: [UntypedElementComposer] = [..];
```

At startup, `ElementComposers` is populated from the slice in
one pass.

### `#[fynix(compose)]` proc-macro

A proc-macro attribute handles registration boilerplate. `E`
is inferred from the first parameter (`&mut E`), `W` from
`FynixCtx<W>` in the second:

```rust
#[fynix(compose)]
fn compose_hierarchy_bevy(
    e: &mut Hierarchy,
    ctx: &mut FynixCtx<BevyWorld>,
) {
    let h = ctx.world.query(..);
    ctx.add::<Label>();
    // ...
}

#[fynix(compose)]
fn compose_hierarchy_custom(
    e: &mut Hierarchy,
    ctx: &mut FynixCtx<CustomWorld>,
) {
    ctx.add::<Label>();
    ctx.add::<Button>();
    // ...
}
```

Expands to the function plus:

```rust
#[linkme::distributed_slice(fynix::ELEMENT_COMPOSERS)]
static _ELEMENT_COMPOSER_COMPOSE_HIERARCHY_BEVY:
    UntypedElementComposer =
    UntypedElementComposer::new::<Hierarchy, BevyWorld>(
        compose_hierarchy_bevy,
    );
```

Usage:

```rust
fn create_ui(ctx: FynixCtx<BevyWorld>) {
    ctx.add::<Hierarchy>();
}
```

The static name is derived from the function name to
guarantee uniqueness within a module.

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
