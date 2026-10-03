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

![The Bevy gallery example: bound values, hover transitions, button styles, a segmented control and scoped rules.](https://raw.githubusercontent.com/voxell-tech/fynix/main/assets/gallery.webp)

- `FynixPlugin<T>` - keeps every mounted view in step with the world
  each `Update`, for views built with the theme `T`.
- `Theme<T>` - the theme, as a resource the app inserts.
- `mount` - builds a view at the root of the UI.
- `Hovered`, `Pressed`, `Entering`, `Leaving` - states for
  `.when::<S, _>(..)`, and any component of your own works too. A
  view a `keyed` or `each` builds later keeps `Entering` until its
  space has expanded, so it fades in last, and a view they drop gets
  `Leaving`, then collapses. `ReducedMotion` skips all of it.
- `ghost`, `tint`, `icon_button`, `primary`, `danger`, `menu_bar`,
  `segment` - button looks, as bundles for `.rules(..)`. Only
  `icon_button` and `tint` never have a surface.
- `tinted_icon` - a button of one icon in the `tint` look, for an
  action beside content, such as a plus that adds to a list.
- `revealed` - a divider look that draws only while hovered or
  `Dragging`.
- `popup` - a menu surface hung off a point, over a backdrop that
  dismisses it when pressed.
- `DockPlugin` - split panes with tabbed areas, from a `DockTree`.
- `style` - builds a look of your own. See [Styling](#styling).

## Quick Start

See [`gallery.rs`](examples/gallery.rs) for a whole app, or run
`cargo run -p bevy_fynix --example gallery` to see every idea in a
window.

```rust
# use std::time::Duration;
# use bevy::color::Color;
# use bevy::ecs::world::World;
use bevy_fynix::views::{
    FrameProps, Label, button, column, label,
};
use bevy_fynix::tokens::{
    Curve, Motion, MotionTokens, SpacingTokens, SurfaceTokens,
    TextTokens, Tone,
};
use bevy_fynix::{Bevy, Cx, Hovered, ScopedExt, StateExt, mount};

# struct MyTheme;
#
# impl TextTokens for MyTheme {
#     fn tone(&self, _: Tone) -> Color {
#         Color::WHITE
#     }
#     fn body_size(&self) -> f32 {
#         14.0
#     }
#     fn small_size(&self) -> f32 {
#         11.0
#     }
# }
#
# impl SurfaceTokens for MyTheme {
#     fn fill(&self) -> Color {
#         Color::BLACK
#     }
#     fn hover(&self) -> Color {
#         Color::BLACK
#     }
#     fn panel(&self) -> Color {
#         Color::BLACK
#     }
#     fn accent(&self) -> Color {
#         Color::WHITE
#     }
# }
#
# impl SpacingTokens for MyTheme {
#     fn gap(&self) -> f32 {
#         8.0
#     }
#     fn row(&self) -> f32 {
#         24.0
#     }
#     fn radius(&self) -> f32 {
#         4.0
#     }
# }
#
# impl MotionTokens for MyTheme {
#     fn motion(&self, _: Motion) -> Curve {
#         Curve {
#             duration: Duration::from_millis(180),
#             ease: |t| t,
#         }
#     }
# }
#
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

## Styling

Three things decide how a view looks, and they stay apart:

- **Theming** says what the colours, sizes and curves of an app are.
  A theme implements token traits (`TextTokens`, `SurfaceTokens`,
  `SpacingTokens`, `MotionTokens`). Views and styles ask for a role
  (`Tone::Dim`, `theme.hover()`), never a raw colour, so changing the
  theme restyles everything.
- **Styling** says which looks a view comes in: ghost, icon, primary.
  A look is a rule bundle, applied with `.rules(..)`. It is made of
  set rules, which restyle every `Frame`, `Label` or `Icon` built
  under it, with `cx.root(..)` keeping a rule to the view's root.
- **State rules** say what a look does while the pointer is on the
  view, while a button is down on it, or while any component of your
  own is on it: `.when::<Hovered, _>(..)`, `.when::<Pressed, _>(..)`.
  `.transition(Motion::Interact)` makes the change glide.

Buttons ship with their looks as bundles. Each sets a resting, a
hovered and a pressed look from the theme, and glides between them:

```ignore
button(label("Cancel")).rules(ghost)
button(icon(save)).rules(icon_button)
button(label("Save")).rules(primary)
button(label("Delete")).rules(danger)
button(row((icon(tag), label("Tag")))).rules(tint)
```

For a look of your own, build a `Style` rather than writing `cx.set`
closures. It takes colours as closures over the theme, and a nested
look for each state:

```ignore
let chunky = style::<MyTheme>()
    .fill(|t| t.panel())
    .frame(|f, t| f.radius(t.radius() * 3.0))
    .hovered(|s| s.fill(|t| t.fill()).tone(Tone::Accent))
    .pressed(|s| s.fill(|t| t.pressed()).tone(Tone::Critical))
    .transition(Motion::Interact);

button(label("Custom")).rules(chunky.bundle())
```

`fill` and `frame` reach the root frame alone, and `tone`, `label` and
`icon` reach every label and icon in the view. Later steps win, and a
press shows its hover look unless `pressed` says otherwise. Styles
join with `then`, and `with(ghost)` starts from a bundle. Chained
`.rules(a).rules(b)` also works, but as set rules go the inner `a` wins
over `b` for resting looks, while for state looks the outer `b` wins.
Use `then` when the order matters.

## Version Matrix

| Bevy    | Fynix          | Bevy Fynix     |
| ------- | -------------- | -------------- |
| 0.19    | 0.0.1 - 0.0.2  | 0.0.1 - 0.0.2  |

## Join the community!

You can join us on the [Voxell discord server](https://discord.gg/Mhnyp6VYEQ).

## License

`bevy_fynix` is dual-licensed under either:

- MIT License ([LICENSE-MIT](../../LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

This means you can select the license you prefer!
This dual-licensing approach is the de-facto standard in the Rust ecosystem and there are [very good reasons](https://github.com/bevyengine/bevy/issues/2373) to include both.
