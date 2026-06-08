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

Rules cascade. A container's closure is its own scope: rules set
inside it override the outer ones, and the outer defaults are
automatically restored when the closure returns:

```rust
ctx.set(field_accessor!(<Label>::font_size), 16.0);

ctx.add_with::<Vertical>(|v, ctx| {
    ctx.set(field_accessor!(<Label>::font_size), 24.0);
    v.add(ctx.add::<Label>()); // 24.0 - inner wins
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

UI state changes over time. Fynix reacts through change-detection
closures: you say what changed, and the framework re-evaluates the
minimum needed. There are two granularities, a *scope* that rebuilds a
subtree and a *binding* that mutates a single element in place.

Both are world-agnostic: the core never names a specific runtime, it
only calls your closures. The first integration target is Bevy ECS,
where the change check maps naturally onto component change detection.

### Scope

A scope is a subtree paired with a change-detection closure. When the
state it reads changes, the whole subtree is rebuilt.

```rust
let panel = ctx.reactive(
    |world: &World| world.score_changed(),
    |ctx| build_score_panel(ctx),
);
```

`ctx.reactive(changed, build)` builds the subtree once and returns a
holder element that owns it. Each frame the backend calls
`Fynix::update_scopes::<W>(world)`; for every scope whose `changed`
closure reports a change, `build` reruns and the subtree is rebuilt in
place under its original style scope.

### Binding

A binding is the lightweight counterpart: instead of rebuilding a
subtree, it writes directly into one element's fields. Bindings attach
per instance through the `ElementHandle` returned by `add`:

```rust
ctx.add::<Label>().bind(
    |world| world.score_changed(),
    |world, label| label.text = world.score.to_string(),
);

// Or when we need to capture some environment data...
ctx.add::<Label>().bind_with(
    entity,
    |world, entity| world.score_changed(*entity),
    |world, entity, label| {
        label.text = world.score(*entity).to_string();
    },
);
```

---

## Rendering

Fynix's core is not bound to any GPU or windowing stack. Elements
paint themselves into [`imaging`](https://crates.io/crates/imaging), a
backend-agnostic 2D drawing abstraction: each element records its
visual layer into an `imaging` `Scene` through a `PaintSink`, parents
before children, so the framework only ever produces a renderer-neutral
description of what to draw.

A renderer is then anything that consumes an `imaging` scene. The
first target adapts to [Vello](https://github.com/linebender/vello)
for GPU-accelerated 2D rendering via `imaging_vello`, but swapping in
another backend means swapping the `imaging` adapter, not touching
fynix. Recorded scenes are cached per element and only re-recorded
when that element changes.

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
