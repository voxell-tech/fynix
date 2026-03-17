# Fynix Vision

Fynix is a composable, declarative UI framework with a novel rule-driven architecture.
Inspired by [Typst](https://typst.app), the core philosophy is that widget appearance and
layout should be expressible as cascading, scoped defaults — not explicit per-instance
configuration.

---

## Intended API

The high-level API Fynix is working towards:

```rust
// Instantiate an element with an inline rule override:
let mut frame_a = ctx.instantiate_with::<Frame>(|frame| {
    frame.height = Some(10.0);
});
let frame_b = ctx.instantiate::<Frame>();

// Set a rule at any point — all subsequent instantiations inherit it:
ctx.set_rule::<Frame>(|frame| { frame.height = Some(20.0) });
let frame_c = ctx.instantiate::<Frame>(); // height == 20.0

// Rules can be scoped — they only apply within the closure:
ctx.scope(|ctx| {
    ctx.set_rule::<Frame>(|frame| { frame.height = Some(5.0) });
    let frame_in_scope = ctx.instantiate::<Frame>(); // height == 5.0
});
// Outside the scope, the previous rule is restored.
```

---

## Setters

A **Setter** is the atomic unit of field mutation. It knows how to write a single typed
value `T` into a single field of a struct `S`, looked up by a `ValueId<K>` at apply time.

```rust
pub struct Setter<K, S> {
    pub set_fn: SetFn<K, S>,
}

pub type SetFn<K, S> =
    fn(&mut S, &ValueId<K>, &FieldAccessorRegistry, &ValueTable<K>) -> bool;
```

- `K` — the key type that identifies a node/scope (e.g. a node ID).
- `S` — the source/target struct type (e.g. `Frame`, `Label`).
- `SetFn` returns `bool` indicating whether the apply succeeded (both accessor and value
  were found).

A `Setter<K, S>` is always created for a specific value type `T` via `Setter::new::<T>()`.
The resulting function pointer is monomorphized once per `<K, S, T>` triple — multiple
`Setter` instances with the same triple share the same binary function.

### Type erasure

Since many different `T`s may target the same `S`, setters are stored type-erased as
`UntypedSetter<K>` (keyed by `UntypedField`) in the `FieldSetterRegistry`. They can be
recovered back to `Setter<K, S>` at apply time via a runtime `TypeId` check.

```
Setter<K, S>  <──────────────────>  UntypedSetter<K>
   .untyped()                          .typed::<S>()
```

---

## Rules

Rules are Fynix's equivalent of [Typst's `set` rules](https://typst.app/docs/reference/styling/#set-rules).

In Typst:
```typst
#set text(size: 14pt)
// All text from here forward defaults to 14pt.
```

In Fynix, a rule is a scoped default applied to a widget's fields, identified by a key `K`.
Rather than threading values explicitly through every widget instantiation, you declare
what the defaults are and they propagate forward.

### Data model

```
FieldRegistries<K>          — global, shared, grow-only
  ├── FieldSetterRegistry<K>   HashMap<UntypedField, UntypedSetter<K>>
  └── FieldAccessorRegistry    HashMap<UntypedField, UntypedAccessor>

RuleSet<K>                  — per-key, mutable
  ├── values: TypeTable<ValueId<K>>       actual stored values, keyed by (K, field)
  └── field_indices: HashMap<K, FieldIndex<TypeId>>   which fields are active per key
```

`FieldRegistries` is global and shared — it holds the "how" (how to access and set a field).
`RuleSet` is local and mutable — it holds the "what" (what value is set for a given key).

### Usage

```rust
// Begin editing rules for node key `1`.
let mut builder = rule_set.edit(1u32, &mut registries);
builder.add(field_accessor!(<Frame>::width), 200.0f32);
builder.add(field_accessor!(<Frame>::opacity), 0.9f32);
builder.commit();

// Apply all rules for key `1` onto a Frame instance.
rule_set.apply_styles(&1u32, &mut frame, &registries);
```

### Key properties

- **Scoped**: rules are keyed — different nodes/scopes can have different defaults.
- **Multi-type**: a single key can hold rules for multiple widget types (`Frame`, `Label`, etc.)
  simultaneously; `apply_styles::<S>` only applies the fields relevant to `S`.
- **Composable**: rules can be built incrementally via the builder pattern, and overridden
  by re-editing the same key.
- **Type-safe**: field accessors carry full `<S, T>` type information at registration time;
  only the storage and dispatch are type-erased.

### StyleId

`StyleId` captures the position of a widget in the layout tree:
- `local_rank` — index among all instantiations of the same widget type in the current scope
  (disambiguates multiple `Frame`s at the same depth).
- `depth_node` — the node's depth + ID in the `Rectree`.

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

### Cycle prevention (to be discussed further...)

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

- Scoping mechanism for context-aware units (tied to `Ctx` / `Fynix`).
