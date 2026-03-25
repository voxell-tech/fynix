# Fynix Vision

Most UI frameworks ask you to configure every widget individually.
You create a button, set its color, set its font size, set its
padding, then do the same for the next button, and the next. When
the design changes, you update each one.

Fynix takes a different approach, borrowed from
[Typst](https://typst.app): you declare *rules*, and the framework
applies them. Rules are scoped, they take effect from the point of
declaration until the scope ends, then the previous defaults are
restored. The tree builds itself around your intent, not around
imperative mutations.

---

## The Style Rule Model

Instead of configuring each element, you set a rule that applies to
all elements of a given type within the current scope:

```rust
// Every Label added after this inherits font_size 16.0:
ctx.set(field_accessor!(<Label>::font_size), 16.0);

ctx.add::<Label>(); // font_size == 16.0
ctx.add::<Label>(); // font_size == 16.0
```

Rules cascade. Inner scopes can override outer ones, and the outer
defaults are automatically restored when the scope closes:

```rust
ctx.set(field_accessor!(<Label>::font_size), 16.0);

ctx.scope(|ctx| {
    ctx.set(field_accessor!(<Label>::font_size), 24.0);
    ctx.add::<Label>(); // 24.0 - inner wins
});

ctx.add::<Label>(); // 16.0 - outer restored
```

When you need to mutate one specific element rather than set a rule,
you use `add_with` and assign the field directly:

```rust
ctx.add_with::<Horizontal>(|e, ctx| {
    e.gap = 4.0;             // this element only
    e.add(ctx.add::<Label>());
});
```

No special API for "override". No selector syntax. Just Rust.

---

## Units

Lengths in Fynix are typed. You don't pass raw `f32` pixels, you
express intent:

```rust
// Note: exact API not yet confirmed
ctx.set(field_accessor!(<Label>::font_size), cm(1.2));
ctx.set(field_accessor!(<Horizontal>::gap), px(8.0));
```

Units are zero-sized types. The conversion chain is declared once and
resolved eagerly to pixels at registration time, so lookups are O(1):

```rust
ctx.insert_unit::<Mm, Px>(4.0);   // 1mm = 4px
ctx.insert_unit::<Cm, Mm>(10.0);  // resolved: 1cm = 40px
```

Context-aware units (`WidthFr`, `HeightFr`) resolve relative to the
current container and are updated automatically as the layout tree is
traversed. Cycles in the unit graph are a **compile error**, not a
runtime panic.

---

## Reactivity

UI state changes over time. Fynix will expose a backend-agnostic
`Signal<T>` primitive: a value that elements can subscribe to and
that triggers a minimal re-evaluation when it changes.

The first integration target is Bevy ECS, where signals map naturally
onto components and change detection. The core reactivity model is
deliberately kept separate from any specific game engine or runtime.

---

## Rendering

Fynix's core has no rendering dependency. Backends are separate
crates that receive the built element tree and lay it out and paint
it. The first backend, `fynix_vello`, will use
[Vello](https://github.com/linebender/vello) for GPU-accelerated 2D
rendering.

Layout is powered by [Rectree](https://github.com/voxell-tech/rectree).
Each element type registers its own layout solver; the framework
dispatches to the right one at layout time.

---

## What Fynix is Not

- Not a retained-mode widget library with a fixed set of built-in
  components. Any `struct` that implements `Element` is a first-class
  participant.
- Not tied to a specific renderer, runtime, or platform.
- Not a macro DSL. The API is plain Rust: closures, generics, field
  accessors.
