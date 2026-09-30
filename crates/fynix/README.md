# Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix.svg)](https://crates.io/crates/fynix)
[![Downloads](https://img.shields.io/crates/d/fynix.svg)](https://crates.io/crates/fynix)
[![Docs](https://docs.rs/fynix/badge.svg)](https://docs.rs/fynix/latest/fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix** is a backend agnostic reactive view tree, styled the way
[Typst](https://typst.app) styles a document: with set rules.

A backend implements `Backend` to say what a world and a node are, and
writes the elements. Everything above the elements, from composites to
rules, themes and transitions, is the same on every backend.

## Key Features

- **Backend agnostic**: views, rules and transitions never name an
  engine.
- **Themes as trait bounds**: a view asks for the token traits it
  reads (`TextTokens`, `SurfaceTokens`, ..) and works under any theme
  that implements them.
- **Set rules**: restyle every view of a kind within a scope. A
  call-site value still wins.
- **State rules**: rules that hold while a node is hovered, pressed,
  entering, leaving, or in any state of your own, reaching every view
  under it.
- **Reactive**: props bound to the world are re-read only when their
  source changes.
- **Transitions**: opt in with a rule, and views animate in and out
  too.
- **`#![no_std]`**: `alloc` only.

## Quick Start (Bevy)

See [`gallery.rs`](../bevy_fynix/examples/gallery.rs) for a whole app.

```rust,ignore
use bevy_fynix::views::{Label, button, column, label};
use bevy_fynix::tokens::{Motion, Tone};
use bevy_fynix::{Bevy, Cx, Hovered, ScopedExt, StateExt, mount};

fn setup(world: &mut World) {
    mount::<MyTheme>(
        world,
        column((
            label("Settings").size(20.0),
            button(label("Save"))
                // Every label in the button turns accent on hover.
                .when::<Hovered, _>(|cx: &mut Cx<Bevy, MyTheme>| {
                    cx.set::<Label>(|l, _| l.tone(Tone::Accent));
                })
                .transition(Motion::Interact),
        ))
        .gap(8.0),
    );
}
```

## Views

A view is a struct that owns its props. There are three kinds:

- **Elements** are one node each, written by the backend: `Label`,
  `Frame`, `Icon`.
- **Composites** are built out of other views: `row`, `column`,
  `button`, `foldable`. Their parts are ordinary views, so ordinary
  rules reach them.
- **Wrappers** add something to any view: `.when(..)`,
  `.transition(..)`, `.rules(..)`, `.on_activate(..)`.

`keyed` and `each` build and drop parts of the tree as the world
changes, keeping every view whose key stays.

## Styling

From weakest to strongest:

| Layer | Example |
|---|---|
| View default | a theme token |
| Composite defaults | `cx.defaults(..)` |
| Set rules, outer then inner | `cx.set::<Label>(\|l, _\| l.size(12.0))` |
| State rules from ancestors | `row(..).when::<Hovered, _>(..)` |
| Call site | `label("x").size(20.0)` |
| State rules on the node itself | `label("x").when::<Hovered, _>(..)` |

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
