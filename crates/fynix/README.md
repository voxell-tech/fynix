# Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix.svg)](https://crates.io/crates/fynix)
[![Downloads](https://img.shields.io/crates/d/fynix.svg)](https://crates.io/crates/fynix)
[![Docs](https://docs.rs/fynix/badge.svg)](https://docs.rs/fynix/latest/fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix** is a reactive element tree: watchers, bindings, and
transitions over a backend world.

`Fynix<H>` owns every watcher and binding and the tree they maintain. A
backend implements the `Host` trait to say what a node is and how its
world is walked; `bevy_fynix` is one such backend. Elements are structs
marked with `#[element]` (from `fynix_macros`); a field is bound by the
walk that reaches it, keyed by a `lenz` field path, and a change to it
is patched onto the node rather than rebuilding.

## Key Features

- **Backend agnostic**: a backend implements the `Host` trait to say
  what a node is and how its world is walked.
- **Reactive**: a watcher rebuilds a subtree when something changes; a
  binding patches a single field onto its node instead.
- **Field-path keyed**: bindings are keyed by
  [`lenz`](https://github.com/nixonyh/lenz) field paths, resolved at
  compile time, with no `dyn Any`.
- **Transitions**: a field can travel to its target rather than snap.
- **`#![no_std]`**: `alloc` only.

## Crates

| Crate | Description |
| ----- | ----------- |
| [`fynix`](https://github.com/voxell-tech/fynix/tree/main/crates/fynix) | The element model, kernel, and reactive primitives. |
| [`fynix_macros`](https://github.com/voxell-tech/fynix/tree/main/crates/fynix_macros) | Derive macros: `#[element]`. |
| [`bevy_fynix`](https://github.com/voxell-tech/fynix/tree/main/crates/bevy_fynix) | Bevy backend. |

## Layout

- `element` - the element model; `elem!` is the builder macro.
- `ui` - `Ui`, `ElementMut`, and field patching.
- `records` - watchers and bindings.
- `store` - the element table.
- `style` - `Style`, a look as a mutation of an element.
- `anim`, `tween` - transitions.
- `composer` - building a subtree from inputs without joining the tree.
- `host`, `world_node` - the backend seam.

`fynix::lenz` re-exports the
[`lenz`](https://github.com/nixonyh/lenz) crate for field paths.

## Version Matrix

| Bevy | Fynix |
| ---- | ----- |
| 0.19 | 0.1   |

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`fynix` is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
