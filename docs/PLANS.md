# Plans

| Area                                 | Status                            |
|--------------------------------------|-----------------------------------|
| Unit system (`src/unit.rs`)          | Planned, not started              |
| Interaction layer                    | Designed, see [INTERACTION.md](INTERACTION.md) |
| Focus system                         | Planned, not started              |
| Field bindings (`Bindings`)          | Planned, not started              |

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
