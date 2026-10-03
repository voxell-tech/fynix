# Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix.svg)](https://crates.io/crates/fynix)
[![Downloads](https://img.shields.io/crates/d/fynix.svg)](https://crates.io/crates/fynix)
[![Docs](https://docs.rs/fynix/badge.svg)](https://docs.rs/fynix/latest/fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix** is a backend agnostic reactive view tree, styled the way
[Typst](https://typst.app) styles a document with set and show rules.

https://github.com/user-attachments/assets/31153e53-c1f4-4564-8eef-4bce4155101a

## Features

- Views, rules and transitions never name an engine.
- A view asks for the token traits it reads, so it works under any
  theme that implements them.
- Set and show rules restyle every view of a kind within a scope, as
  in Typst.
- State rules hold while a node is hovered, pressed, entering,
  leaving, or in any state of your own.
- Props bound to the world are read again only when their source
  changes.
- Views can animate their changes, and animate in and out of the
  tree.
- `#![no_std]`, with `alloc` only.

The [`fynix` README](crates/fynix#concepts) walks through each of
these with examples.

## Example

```rust
// A heading and a button whose label turns accent on hover.
mount::<MyTheme>(
    world,
    column((
        label("Settings").size(20.0),
        button(label("Save"))
            .when::<Hovered, _>(|cx: &mut Cx<Bevy, MyTheme>| {
                cx.set::<Label>(|l, _| l.tone(Tone::Accent));
            })
            .transition(Motion::Interact),
    ))
    .gap(8.0),
);
```

## Where Next

- [`fynix`](crates/fynix): the core crate, with a guide to building a
  UI framework on top of it.
- [`fynix_macros`](crates/fynix_macros): the `#[element]` macro.
- [`bevy_fynix`](crates/bevy_fynix): the Bevy backend, with a gallery
  example and ready-made looks.

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`fynix` is dual-licensed under either:

- MIT License ([LICENSE-MIT](LICENSE-MIT) or
  [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer! This dual-licensing
approach is the de-facto standard in the Rust ecosystem and there are
[very good reasons](https://github.com/bevyengine/bevy/issues/2373) to
include both.
