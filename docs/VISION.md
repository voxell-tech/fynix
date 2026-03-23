# Fynix Vision

Fynix is a composable, declarative UI framework with a novel rule-driven architecture.
Inspired by [Typst](https://typst.app), the core philosophy is that widget appearance and
layout should be expressible as cascading, scoped defaults — not explicit per-instance
configuration.

---

## Architecture Overview

```
Fynix
├── tree: Rectree          — spatial layout tree
├── elements: Elements     — type-erased heterogeneous element storage
└── styles: Styles         — cascading style registry
```

Entry point is `Fynix::new()`. A `BuildCtx<'_>` is obtained via `fynix.root_ctx()` and
is the primary handle for adding elements and declaring style defaults.

---

## Intended API

The high-level API Fynix is working towards:

```rust
let mut fynix = Fynix::new();
let mut ctx = fynix.root_ctx();

// Add an element:
let id = ctx.add::<Horizontal>();

// Set a style default — all elements of that type added afterwards inherit it:
ctx.set(field_accessor!(<Horizontal>::gap), 8.0f32);
let id = ctx.add::<Horizontal>(); // gap == 8.0

// Add with inline mutations directly on the element:
let id = ctx.add_with::<Horizontal>(|e, ctx| {
    e.gap = 4.0;
    e.add(child_id);
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

The `Element` trait is the marker for widget types:

```rust
pub trait Element: 'static {
    fn new() -> Self where Self: Sized;
}
```

Elements are stored type-erased inside `Elements`, keyed by `ElementId`
(a generational ID). A registry of `GetDynElementFn` function pointers (one per concrete
type, monomorphized at first insertion) allows polymorphic access via `&dyn Element`
without boxing every element.

```
Elements
├── elements: TypeTable<ElementId>           — actual element instances
├── element_types: HashMap<ElementId, TypeId>  — which concrete type each ID holds
├── element_getters: HashMap<TypeId, GetDynElementFn>  — how to return &dyn Element
└── id_generator: IdGenerator                — generational ID allocation + recycling
```

---

## Styles

### SetStyle

`SetStyle<E>` is the atomic unit of field mutation. It holds a monomorphized function
pointer (`SetStyleFn<E>`) that knows how to write a single typed value `T` into a field
of element `E`, fetching the value from `TypeTable<StyleId>` via an `UntypedAccessor`.

```rust
pub type SetStyleFn<E> = fn(
    &mut E,
    &UntypedAccessor,
    &StyleId,
    &TypeTable<StyleId>,
) -> bool;
```

Returns `true` if both the accessor and the stored value were found. The function is
monomorphized once per `(E, T)` pair — multiple `SetStyle` instances for the same pair
share the same binary function.

### Type erasure

Since many different `T`s may target the same `E`, `SetStyle<E>` is stored type-erased as
`UntypedSetStyle` in the registry. It can be recovered at apply time via a runtime `TypeId`
check:

```
SetStyle<E>  <────────────────────>  UntypedSetStyle
  .untyped()                           .typed::<E>()
```

### Styles struct

`Styles` is the central style manager. It owns:

```
Styles
├── registry: HashMap<UntypedField, (UntypedAccessor, UntypedSetStyle)>
│     — registered once per field; "how" to read and write each field
├── style_values: TypeTable<StyleId>
│     — the actual stored values, keyed by (StyleId, T)
├── field_indices: HashMap<StyleId, Style>
│     — committed style nodes, each with a parent and a FieldIndex
├── field_index_builder: FieldIndexBuilder
│     — accumulates pending field changes before the next commit
├── current_id: StyleId
│     — the "open" style node being built
└── id_generator: StyleIdGenerator
      — generational ID allocation + recycling
```

### Style nodes

A `Style` is an immutable, committed snapshot of a set of field changes:

```rust
pub struct Style {
    parent_id: Option<StyleId>,  // inherited from
    field_index: FieldIndex,     // which fields are active
}
```

Style nodes form a singly-linked chain. Applying a style to an element walks the chain
from the current node up to the root, applying each active field.

### Two-phase commit

Style changes are accumulated in `FieldIndexBuilder` and not committed until the next
`add()` or `add_with()` call detects pending changes via `should_commit()`. At that
point, `commit_styles()` compiles the builder into an immutable `FieldIndex`, stores the
`Style` node, and advances to a fresh `StyleId`. This batching reduces allocations and
enables future scoping.

---

## Storage Infrastructure

### TypeTable\<K\>

Heterogeneous key→value storage. A single key can hold values of multiple types
simultaneously. Internally, one `TypeMap<K, T>` (backed by a `SparseMap`) exists per
type `T` ever inserted.

Operations: `insert::<T>(key, val)`, `get::<T>(key)`, `remove::<T>(key)`,
`remove_all(key)` (removes all types for a key), `dyn_remove(type_id, key)`.

### FieldIndex

Maps `TypeId` → `Span` over a flat `Box<[UntypedField]>`. Tells you which fields of a
given element type are active in a style node. Built by `FieldIndexBuilder` (mutable,
uses a `HashSet` to deduplicate) and compiled to an immutable `FieldIndex` at commit
time.

### GenId\<T\> / IdGenerator\<T\>

Generational IDs with a phantom type parameter. `IdGenerator` recycles raw IDs (bumping
the generation to prevent ABA problems). Used for both `ElementId` and `StyleId`.

---

## Units (planned)

> Status: **not yet implemented**. The style system must be complete first.

Units are zero-sized marker types. Their `TypeId` serves as the registry key.

```rust
struct Px; // foundational base — always present, ratio = 1.0
struct Mm;
struct Cm;
struct M;
struct WidthFr;  // 1.0 × current container width in Px
struct HeightFr; // 1.0 × current container height in Px
```

### Registry

```
HashMap<TypeId, (TypeId, f32)>
         ^unit    ^parent  ^factor ("1 of me = factor of parent")
```

Ratios are resolved **eagerly** to `Px` at `insert_unit` time:

```rust
ctx.insert_unit::<Mm, Px>(4.0);      // Mm → Px: 4.0
ctx.insert_unit::<Cm, Mm>(10.0);     // resolves: Cm → Px: 40.0
ctx.insert_unit::<M, Cm>(100.0);     // resolves: M  → Px: 4000.0
```

This makes `get_unit::<M>()` O(1) — it returns the stored `f32` directly.

### Context-aware units

`WidthFr` and `HeightFr` are re-inserted at each container scope boundary:

```rust
// Entering a container of width 800px:
ctx.insert_unit::<WidthFr, Px>(800.0);
// get_unit::<WidthFr>() == 800.0

// Nested container of width 400px:
ctx.scope(|ctx| {
    ctx.insert_unit::<WidthFr, Px>(400.0);
    // get_unit::<WidthFr>() == 400.0
});
```

No special-casing needed — they're just units with a dynamically-updated ratio.

### Module

The unit system will live in its own module: `src/unit.rs`.

### Cycle prevention

Cycles are prevented at **compile time** via a `Parent` associated type and a `ToPx` trait:

```rust
pub trait Unit: 'static {
    type Parent: ToPx;
    const FACTOR: f32;
}

pub trait ToPx {
    fn to_px() -> f32;
}

impl ToPx for Px {
    fn to_px() -> f32 { 1.0 }
}

impl<U: Unit> ToPx for U {
    fn to_px() -> f32 { U::FACTOR * U::Parent::to_px() }
}
```

A cycle (e.g. `Mm::Parent = Cm`, `Cm::Parent = Mm`) causes the trait solver to recurse
infinitely — a **compile error**, not a runtime panic. Non-cyclic chains terminate at `Px`.

Context-aware units (`WidthFr`, `HeightFr`) have a dynamic pixel value and require
separate handling outside of this static chain.

### Open questions

- Scoping mechanism for context-aware units (tied to `BuildCtx` / `Fynix`).
