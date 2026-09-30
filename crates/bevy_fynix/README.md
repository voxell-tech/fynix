# Bevy Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/bevy_fynix.svg)](https://crates.io/crates/bevy_fynix)
[![Downloads](https://img.shields.io/crates/d/bevy_fynix.svg)](https://crates.io/crates/bevy_fynix)
[![Docs](https://docs.rs/bevy_fynix/badge.svg)](https://docs.rs/bevy_fynix/latest/bevy_fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Bevy Fynix** is the Bevy backend for [`fynix`](https://github.com/voxell-tech/fynix/tree/main/crates/fynix).

Nodes are entities and the world is Bevy's `World`. On top of that
seam it ships elements written against `bevy_ui` (`Label`, `Frame`,
`Icon`), composites (`row`, `column`, `button`, `foldable`), the
token traits they read, and the states rules wait on.

- `FynixPlugin<T>` - keeps every mounted view in step with the world
  each `Update`, for views built with the theme `T`.
- `Theme<T>` - the theme, as a resource the app inserts.
- `mount` - builds a view at the root of the UI.
- `Hovered`, `Pressed`, `Entering`, `Leaving` - states for
  `.when::<S, _>(..)`, and any component of your own works too.

`cargo run -p bevy_fynix --example gallery` shows every idea in a
window.

## Version Matrix

| Bevy | Bevy Fynix |
| ---- | ---------- |
| 0.19 | 0.0.1      |

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`bevy_fynix` is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
