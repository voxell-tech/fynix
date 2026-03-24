# Fynix Vision

Fynix is a composable, declarative UI framework with a novel
rule-driven architecture. Inspired by [Typst](https://typst.app),
the core philosophy is that widget appearance and layout should be
expressible as cascading, scoped defaults — not explicit per-instance
configuration.

---

## Workspace Layout

```
fynix/
├── Cargo.toml               — workspace root
└── crates/
    ├── fynix/               — core: elements, style system, build ctx
    ├── fynix_elements/      — default element types (Horizontal, Vertical, Label)
    └── fynix_vello/         — vello rendering backend (skeleton, not yet wired)
```

All crates are `no_std` + `extern crate alloc`.

---

## Architecture Overview

```
Fynix
├── tree: Rectree            — spatial layout tree (not yet wired)
├── elements: Elements       — type-erased heterogeneous element storage
└── styles: Styles           — cascading style registry
```

Entry point is `Fynix::new()`. A `BuildCtx<'_>` is obtained via
`fynix.root_ctx()` and is the primary handle for adding elements and
declaring style defaults.

---

## Intended API

```rust
let mut fynix = Fynix::new();
let mut ctx = fynix.root_ctx();

// Add an element:
let id = ctx.add::<Horizontal>();

// Set a style default — all elements of that type added afterwards
// within this scope inherit it:
ctx.set(field_accessor!(<Horizontal>::gap), 8.0f32);
let id = ctx.add::<Horizontal>(); // gap == 8.0

// Inline mutations are done with direct field assignment:
let id = ctx.add_with::<Horizontal>(|e, ctx| {
    e.gap = 4.0;
    let child = ctx.add::<Label>();
    e.add(child);
});

// Rules can be scoped — they only apply within the closure:
// (not yet implemented)
ctx.scope(|ctx| {
    ctx.set(field_accessor!(<Horizontal>::gap), 2.0f32);
    let id = ctx.add::<Horizontal>(); // gap == 2.0
});
// Outside the scope, the previous defaults are restored.
```

---

## Elements

`Element` is the marker trait for widget types:

```rust
pub trait Element: 'static {
    fn new() -> Self where Self: Sized;
}
```

`new()` returns a default (unstyled) instance. Styles are applied
immediately after construction by `BuildCtx`.

Elements are stored type-erased inside `Elements`, keyed by
`ElementId` (a generational ID):

```
Elements
├── elements: TypeTable<ElementId>
│     — actual element instances, one column per concrete type
├── element_types: HashMap<ElementId, TypeId>
│     — which concrete type each ID holds
├── element_getters: HashMap<TypeId, GetDynElementFn>
│     — monomorphized fn ptrs for polymorphic &dyn Element access
└── id_generator: ElementIdGenerator
      — generational ID allocation + recycling
```

`get_typed::<E>(id)` is the preferred accessor when the type is
known — it skips getter dispatch and takes `&self` instead of
`&mut self`.

### Default elements (`fynix_elements`)

| Type         | Public fields | Notes                           |
|--------------|---------------|---------------------------------|
| `Horizontal` | —             | `children: Vec<ElementId>` (private), `add()` |
| `Vertical`   | —             | same                            |
| `Label`      | `text: String`| —                               |

---

## BuildCtx

`BuildCtx<'a>` is the build-time context. It holds:

```
BuildCtx<'a>
├── parent_style_id: Option<StyleId>  — current position in the style chain
├── elements: &'a mut Elements
└── styles: &'a mut Styles
```

Key methods:

| Method | What it does |
|--------|-------------|
| `add::<E>()` | Commit pending styles, construct `E::new()`, apply style chain, store, return `ElementId` |
| `add_with::<E>(f)` | Same as `add`, but also runs closure `f(&mut E, &mut BuildCtx)` for inline mutations; saves/restores `parent_style_id` around `f` |
| `set(field_accessor, value)` | Queue a style default for element type `E` |

### Style scoping in `add_with`

`add_with` saves `parent_style_id` before calling `f` and restores
it afterward. Any `set` calls or nested `add_with` calls inside `f`
do not leak their style changes outward.

### `create_element` (private)

Shared by `add` and `add_with`. The key sequencing:

1. If `styles.should_commit()`: capture `current_id` *before*
   calling `commit_styles`, set `parent_style_id` to that captured ID.
2. Construct `E::new()`.
3. If `parent_style_id` is `Some`, call `styles.apply(&mut element, id)`.

Capturing the ID before `commit_styles` is critical — `commit_styles`
advances `current_id` to a fresh value; capturing after would point
at an empty node.

---

## Styles

### SetStyle

`SetStyle<E>` is the atomic unit of field mutation. It holds a
monomorphized function pointer (`SetStyleFn<E>`) that knows how to
write a single typed value `T` into a field of element `E`, fetching
the value from `TypeTable<StyleId>` via an `UntypedAccessor`.

```rust
pub type SetStyleFn<E> = fn(
    &mut E,
    &UntypedAccessor,
    &StyleId,
    &TypeTable<StyleId>,
) -> bool;
```

Returns `true` if both the accessor and the stored value were found.
Monomorphized once per `(E, T)` pair.

### Type erasure

Since many different `T`s may target the same `E`, `SetStyle<E>` is
stored type-erased as `UntypedSetStyle` in the registry. Recovered
at apply time via a runtime `TypeId` check:

```
SetStyle<E>  ←────────────────────→  UntypedSetStyle
  .untyped()                            .typed::<E>()
```

### Styles struct

```
Styles
├── registry: HashMap<UntypedField, (UntypedAccessor, UntypedSetStyle)>
│     — registered once per field; "how" to read and write each field
├── style_values: TypeTable<StyleId>
│     — actual stored values, keyed by (StyleId, T)
├── styles: HashMap<StyleId, Style>
│     — committed style nodes forming an inheritance chain
├── style_builder: StyleBuilder
│     — accumulates pending field changes before the next commit
├── current_id: StyleId
│     — the "open" style node being built
└── id_generator: StyleIdGenerator
      — generational ID allocation + recycling
```

### Style nodes

A `Style` is an immutable, committed snapshot of a set of field
changes:

```rust
pub struct Style {
    parent_id: Option<StyleId>,
    index_map: HashMap<TypeId, Span>,  // TypeId → field slice range
    fields: Box<[UntypedField]>,       // flat field list
}
```

`get_fields(&TypeId)` slices `fields` via `index_map` to return the
active fields for a given element type. Nodes form a singly-linked
chain via `parent_id`.

### StyleBuilder

`StyleBuilder` (private) accumulates pending changes in a
`HashMap<TypeId, HashSet<UntypedField>>` (deduplicating fields) and
produces an immutable `Style` via `build(parent_id)`. It replaces the
old `FieldIndexBuilder` + `FieldIndex` split — they were exclusively
internal to `Style`/`Styles`, so the abstraction was collapsed.

`Span` (private) is a `[start, end)` index range into
`Style::fields`.

### Two-phase commit

Style changes are accumulated in `StyleBuilder` and not committed
until the next `add()` / `add_with()` detects pending changes via
`should_commit()`. At that point, `commit_styles(parent_id)` calls
`StyleBuilder::build(parent_id)`, inserts the resulting `Style`, and
advances to a fresh `StyleId`.

### apply\<E\> — leaf-wins cascade

`apply<E>(element, id)` walks the parent chain from the given node up
to the root. A `HashSet<UntypedField>` tracks which fields have
already been written; the first value seen for each field wins (leaf
takes precedence over ancestors). No collection + reversal needed.

---

## Storage Infrastructure

### TypeTable\<K\>

Heterogeneous key→value storage. One key can hold values of multiple
types simultaneously. Internally one `TypeMap<K, T>` column (backed
by a `SparseMap`) is allocated per type `T` ever inserted.

Operations: `insert::<T>`, `get::<T>`, `remove::<T>`,
`remove_all(key)`, `dyn_remove(type_id, key)`.

`DynTypeMap<K>` is the object-safe trait stored as
`Box<dyn DynTypeMap<K>>` that adds a type-erased `dyn_remove`.
`AnyTypeMap<K>` (generated by `any_wrapper!`) provides sealed
`downcast_ref` / `downcast_mut`.

### any\_wrapper! macro

Generates a sealed downcasting trait for a generic wrapper type.
Enables safe `downcast_ref` / `downcast_mut` on `dyn Trait` without
`std::any::Any`. The trait is sealed (via a private `Seal` trait) so
only the wrapper type can implement it, making pointer casts sound.

### GenId\<T\> / IdGenerator\<T\>

Generational IDs with a phantom type parameter. The `generation`
counter is bumped on recycle to prevent ABA problems. Phantom `T`
makes `ElementId` and `StyleId` incompatible at the type level.
Display format: `{id}v{generation}`.

---

## Units (planned)

> Status: **not yet implemented**. The style system must be
> complete first.

Units are zero-sized marker types. Their `TypeId` serves as the
registry key.

```rust
struct Px;       // foundational base — ratio = 1.0
struct Mm;
struct Cm;
struct M;
struct WidthFr;  // 1.0 × current container width in Px
struct HeightFr; // 1.0 × current container height in Px
```

### Registry

```
HashMap<TypeId, (TypeId, f32)>
         ^unit    ^parent  ^factor ("1 of me = factor × parent")
```

Ratios are resolved **eagerly** to `Px` at `insert_unit` time:

```rust
ctx.insert_unit::<Mm, Px>(4.0);    // Mm → Px: 4.0
ctx.insert_unit::<Cm, Mm>(10.0);   // resolves: Cm → Px: 40.0
ctx.insert_unit::<M, Cm>(100.0);   // resolves: M  → Px: 4000.0
```

`get_unit::<M>()` is O(1) — returns the stored `f32` directly.

### Context-aware units

`WidthFr` and `HeightFr` are re-inserted at each container scope
boundary. No special-casing — they are just units with a dynamically
updated ratio.

### Cycle prevention

Cycles are caught at **compile time** via a `Parent` associated type
and a `ToPx` trait. A cycle causes the trait solver to recurse
infinitely — a compile error, not a runtime panic.
