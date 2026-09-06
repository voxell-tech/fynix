# Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix.svg)](https://crates.io/crates/fynix)
[![Downloads](https://img.shields.io/crates/d/fynix.svg)](https://crates.io/crates/fynix)
[![Docs](https://docs.rs/fynix/badge.svg)](https://docs.rs/fynix/latest/fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix** is a backend agnostic reactive element tree.

`Fynix<H>` owns every watcher and binding and the tree they maintain.
A backend implements the `Host` trait to say what a node is and how
its world is walked.

## Key Features

- **Backend agnostic**: Drives any renderer or world, elements,
  styles, and transitions stay the same across backends.
- **Reactive**: Fine grained reactivity via watchers and bindings.
- **Comptime keyed**: Powered by [Lenz](https://github.com/nixonyh/lenz),
  a nested field can be referenced with zero allocation.
- **Transitions**: A field can ease to its target rather than snap.
- **`#![no_std]`**: `alloc` only.

## Quick Start

The `Host` impl and the `Button` / `Label` elements used below are
defined in [`docs/host.rs`](docs/host.rs).

```rust
# #[path = "docs/host.rs"] mod _doc; use _doc::*;
// A host world with a root node, plus app state to drive the tree.
let (mut world, root) = World::with_root();
world.doc.title = "untitled".into();

let mut fynix = Fynix::new(());

// Declare a subtree under `root`. `once()` builds it on the first
// flush and never rebuilds, bindings keep it current after that.
fynix.watch(
    root,
    once(),
    |ui| {
        ui.elem(elem!(Button, padding = 8u32)).bind(
            |button| button.label().text(),                 // which field
            |WorldNodeRef { world, .. }| world.doc.dirty,    // when to write
            |WorldNodeRef { world, .. }| world.doc.title.clone(), // the value
        );
    },
    &mut world,
);

let button = FynixHost::children(&world, root)[0];
let label = FynixHost::children(&world, button)[0];
assert_eq!(world.get(label).text, "Label"); // the label's own base
assert_eq!(world.get(button).padding, 8); // the call site

// A change, announced. The next flush patches just that one field
// onto the node already there - no rebuild.
world.doc.title = "report.md".into();
world.doc.dirty = true;
fynix.flush(&mut world);

assert_eq!(world.get(label).text, "report.md");
```

## Reactivity

Fynix has two ways to react to a change. A watcher rebuilds a
subtree. A binding patches one field.

### Watcher

A watcher is a predicate plus a build closure, anchored to a node.
On each flush the predicate runs. A `true` reruns the closure and
rebuilds its subtree.

```rust
# #[path = "docs/host.rs"] mod _doc; use _doc::*;
let (mut world, root) = World::with_root();
let mut fynix = Fynix::<FynixHost>::new(());

// Rebuild the subtree under `root` on any flush where the
// predicate returns `true`.
fynix.watch(
    root,
    |WorldNodeRef { world, .. }| world.doc.dirty,
    |ui| {
        ui.elem(elem!(Label, text = "rebuilt"));
    },
    &mut world,
);
```

### Binding

A binding is anchored to one field of one node, named by its
[`lenz`] field path. It carries three closures: which field, when to
write, and the value. On flush, if the "when" closure returns
`true`, that one field is patched onto the live node in place.

```rust
# #[path = "docs/host.rs"] mod _doc; use _doc::*;
use fynix::prelude::*;

fn view(ui: &mut Ui<FynixHost>) {
    ui.elem(elem!(Button)).bind(
        |button| button.label().text(),                       // which field
        |WorldNodeRef { world, .. }| world.doc.dirty,          // when to write
        |WorldNodeRef { world, .. }| world.doc.title.clone(),  // the value
    );
}
```

### How they fit

A watcher lays down the tree. Bindings keep its leaves current
while the structure holds. Reach for a watcher when the shape of
the tree changes, a binding when a value does. The field path is
resolved at compile time, so a binding costs one patch call.

## Defining an element

One struct describes structure, defaults, and per-field write logic. A
`#[elem(patch = ...)]` tag is a type whose `FieldPatch` impl writes the
field, at build and on every change. A field can also carry an
`anim(...)` line, so it eases toward a different source while the node
is tagged rather than snapping.

```rust
# #[path = "docs/host.rs"] mod _doc; use _doc::*;
use fynix::prelude::*;
use fynix::motiongfx_interp::ease;

#[element(host = FynixHost)]
pub struct Field {
    // A child element: its own node, walked into one hop at a time.
    #[elem(child)]
    pub label: Label,

    // A default, and the tag that writes the field.
    #[elem(default = 4, patch = WritePadding)]
    pub padding: u32,

    // A default, the tag that writes it, and a line that eases the
    // field toward `hover_size` while the node carries `Hovered`.
    #[elem(default = 13, patch = WriteSize, anim(
        ms = 120,
        ease = ease::cubic::ease_in_out,
        on(Hovered, read = hover_size),
    ))]
    pub size: u32,

    // Element state the line reads through, nothing draws it.
    pub hover_size: u32,
}
```

## Building an element with `elem!`

An element is built as its `base()`, then a `Style`, then whatever the
call site writes. Each layer overrides the last, and `elem!` picks the
right form.

`elem!(..)` yields a builder that takes `&Theme` and returns the
element. A `Ui` hands it the theme for you, here it is passed by
hand. `FynixHost::Theme` is `()`.

```rust
# #[path = "docs/host.rs"] mod _doc; use _doc::*;
const THEME: &() = &();

let plain = elem!(Label)(THEME);
assert_eq!(plain.size, 13); // Label::base

let big = elem!(Label, size = 32u32)(THEME);
assert_eq!(big.size, 32); // the call site

// A child element field starts from its own base.
let make = elem!(Button, label = elem!(Label, text = "Save"));
let button = make(THEME);
assert_eq!(button.label.text, "Save");
assert_eq!(button.padding, 4); // Button::base
```

## Transitions

Tags on a node decide where an animated field heads. Setting one
starts or redirects the transition, and dropping it falls back to the
next active line, or to the field's base.

```rust
# #[path = "docs/host.rs"] mod _doc; use _doc::*;
# use std::time::Duration;
let (mut world, root) = World::with_root();
world.delta = Duration::from_millis(120); // a full 120ms line per flush

let mut fynix = Fynix::new(());
fynix.watch(
    root,
    once(),
    |ui| {
        ui.elem(elem!(Button, size = 13u32, hover_size = 24u32));
    },
    &mut world,
);
let button = FynixHost::children(&world, root)[0];

// Tag it: `size` heads for `hover_size` over the 120ms the line
// names, which one flush of wall time covers here.
fynix.set_tag(button, Hovered);
fynix.flush(&mut world);
assert_eq!(world.get(button).size, 24);

// Untag: it eases back to the base.
fynix.unset_tag::<Hovered>(button);
fynix.flush(&mut world);
assert_eq!(world.get(button).size, 13);
```

## Officially Supported Backends

- [Bevy Fynix](https://crates.io/crates/bevy_fynix)

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`fynix` is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.

[`lenz`]: https://github.com/nixonyh/lenz
