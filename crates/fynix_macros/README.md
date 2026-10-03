# Fynix Macros

[![License](https://img.shields.io/badge/license-MIT%2FApache-blue.svg)](https://github.com/voxell-tech/fynix#license)
[![Crates.io](https://img.shields.io/crates/v/fynix_macros.svg)](https://crates.io/crates/fynix_macros)
[![Downloads](https://img.shields.io/crates/d/fynix_macros.svg)](https://crates.io/crates/fynix_macros)
[![Docs](https://docs.rs/fynix_macros/badge.svg)](https://docs.rs/fynix_macros/latest/fynix_macros/)
[![CI](https://github.com/voxell-tech/fynix/workflows/CI/badge.svg)](https://github.com/voxell-tech/fynix/actions)
[![Discord](https://img.shields.io/discord/442334985471655946.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/Mhnyp6VYEQ)

**Fynix Macros** provides the attribute macros for
[`fynix`](https://github.com/voxell-tech/fynix/tree/main/crates/fynix).

## `#[element]`

Marks a struct of props as an element of one backend. Every field is
a prop, and says how it is written:

```rust,ignore
#[element(backend = Bevy, theme = SpacingTokens)]
pub struct Frame {
    #[elem(patch = PatchWidth)]
    pub width: Prop<Val>,
    #[elem(default = theme.gap(), patch = PatchGap)]
    pub gap: Prop<f32>,
    #[elem(default = Color::NONE, patch = PatchFill, blend = blend_color)]
    pub fill: Prop<Color>,
}
```

On the struct:

- `backend = <type>` - the backend the element is written for.
- `theme = <bounds>` - what its defaults read from a theme.
- `prepare = <fn>` - run on the node before any prop is written.

On a field:

- `patch = <type>` - the `Patch` that writes the prop. It becomes the
  field's `lenz` tag.
- `default = <expr>` - the value when unset. It may read `theme`.
  Without it, the prop type's `Default`.
- `blend = <fn>` - how two values blend, so the prop travels under a
  transition rule. Without it, the prop snaps.
- `shown = <type>, with = <fn>` - writes a value made from the prop
  and the theme, such as a colour from a tone.

It writes the struct with `#[derive(Lenz)]`, a builder method per
prop, `Styled`, `Layered` and `Element`, and a `{Struct}Props` trait
that gives the same builder methods to any composite holding one.

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`fynix_macros` is dual-licensed under either:

- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
