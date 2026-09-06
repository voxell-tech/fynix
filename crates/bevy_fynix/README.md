# Bevy Fynix

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/bevy_fynix.svg)](https://crates.io/crates/bevy_fynix)
[![Downloads](https://img.shields.io/crates/d/bevy_fynix.svg)](https://crates.io/crates/bevy_fynix)
[![Docs](https://docs.rs/bevy_fynix/badge.svg)](https://docs.rs/bevy_fynix/latest/bevy_fynix/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Bevy Fynix** is the Bevy backend for [`fynix`](https://github.com/voxell-tech/fynix/tree/main/crates/fynix).

The seam, and nothing else: nodes are entities, the world is Bevy's
`World`, and the kernel is a resource flushed once a frame. Elements
and styles live above this.

- `FynixPlugin<Theme>` - runs `Fynix::flush` in `FynixSet` every
  `Update`, starting the kernel with `Theme::default()`.
- `BevyHost` - the `Host` impl.
- `tag` - Bevy pointer events mapped to fynix tags.

`Theme` is the app's own type, never a `Resource` and never read back
out of `World`; edit it after the fact through `theme_mut`.

## Version Matrix

| Bevy | Bevy Fynix |
| ---- | ---------- |
| 0.19 | 0.1        |

## License

Dual-licensed under either MIT ([LICENSE-MIT](/LICENSE-MIT)) or Apache
2.0 ([LICENSE-APACHE](/LICENSE-APACHE)), at your option.
