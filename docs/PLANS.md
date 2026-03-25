# Plans

| Area                                 | Status                            |
|--------------------------------------|-----------------------------------|
| Unit system (`src/unit.rs`)          | Planned, not started              |
| `ctx.scope()`                        | Planned, not started              |
| `LayoutSolver` / rectree integration | Pending rectree API change        |
| `fynix_elements` layout impls        | Blocked on rectree                |
| `fynix_vello` rendering              | Blocked on layout                 |
| Reactivity (`Signals`)               | Deferred until after first render |

---

## Rectree API change (next major task)

The current `LayoutWorld::get_solver() -> &dyn LayoutSolver`
signature forces heap allocation due to lifetime constraints.
The plan is to push the two solver methods directly onto
`LayoutWorld`:

```rust
pub trait LayoutWorld {
    fn constraint(&self, id: &NodeId, parent: Constraint) -> Constraint;
    fn build(&self, id: &NodeId, node: &RectNode,
             tree: &Rectree, pos: &mut Positioner) -> Size;
}
```

`LayoutSolver` becomes a fynix-internal trait (removed from rectree).
`Fynix` implements `LayoutWorld` by dispatching to a
`HashMap<TypeId, Box<dyn LayoutSolver>>` keyed on element type.
`BuildCtx` gains a `tree: &'a mut Rectree` reference so `add` also
inserts a `RectNode`.

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
