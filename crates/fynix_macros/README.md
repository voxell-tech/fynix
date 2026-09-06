# Fynix Macros

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix_macros.svg)](https://crates.io/crates/fynix_macros)
[![Downloads](https://img.shields.io/crates/d/fynix_macros.svg)](https://crates.io/crates/fynix_macros)
[![Docs](https://docs.rs/fynix_macros/badge.svg)](https://docs.rs/fynix_macros/latest/fynix_macros/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix Macros** provides the derive macros for
[`fynix`](https://github.com/voxell-tech/fynix/tree/main/crates/fynix).

## `#[element]`

Marks a struct as an element built against one backend. The struct is
re-emitted with `#[derive(Lenz)]`, then `ElementBase` and `Element` are
written for it. The backend is `crate::FynixHost` unless
`#[element(host = <path>)]` names another.

- `#[element(build = <fn>)]` - a structural hook run once at build,
  `fn(&Self, &mut Build<Host, Self>)`.
- `#[elem(child)]` - a field that is an element in its own right.
- `#[elem(patch = <tag>)]` - a type implementing
  `FieldPatch<Host, Target = FieldTy>` that writes the field at build
  and on change.
- `#[elem(default = <expr>)]` - the value the field starts from, with
  `theme` in scope.
- `#[elem(ignore)]` - a field no path can name, read only by `build`.

`#[derive(Lenz)]` itself lives in the
[`lenz`](https://github.com/nixonyh/lenz) crate, re-exported by
`fynix`.

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`fynix_macros` is dual-licensed under either:

- MIT License ([LICENSE-MIT](/LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](/LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
