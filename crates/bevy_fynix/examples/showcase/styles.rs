//! Every button look, a style of its own, and how rules reach views.

use bevy::color::Color;
use bevy::ui::{AlignItems, FlexDirection, UiRect, percent, px};
use bevy_fynix::icons::PLUS;
use bevy_fynix::tokens::{
    Motion, SpacingTokens, SurfaceTokens, Tone,
};
use bevy_fynix::views::{
    Frame, FrameProps, Label, LineColor, Span, button, column,
    danger, frame, ghost, icon, icon_button, label, primary, rich,
    row, scroll, tint,
};
use bevy_fynix::{
    AnyView, Bevy, Hovered, Pressed, ScopedExt, StateExt, Style,
    View, ViewExt, style,
};

use crate::common::Monokai;
use crate::{Build, icons, section};

pub fn styles() -> AnyView<Bevy, Monokai> {
    scroll((
        section("Text in several styles", rich_text()),
        section("Button looks, as rule bundles", looks()),
        section("One state rule across a button's parts", parts()),
        section("Rules in a scope", scoped()),
    ))
    .width(percent(100.0))
    .height(percent(100.0))
    .padding(UiRect::all(px(16.0)))
    .gap(20.0)
    .boxed()
}

fn rich_text() -> impl View<Bevy, Monokai> {
    rich(vec![
        Span::new("One text in "),
        Span::new("bold").bold(),
        Span::new(", "),
        Span::new("italic").italic(),
        Span::new(", "),
        Span::new("underlined").underline(LineColor::Ink),
        Span::new(" and "),
        Span::new("green").color(Color::srgb_u8(0xA9, 0xDC, 0x76)),
        Span::new("."),
    ])
    .tone(Tone::Dim)
}

/// A look of its own: a hover fill and a press fill from the theme, a
/// press that shrinks it a little, and a label that goes critical
/// under the finger.
fn chunky() -> Style<Monokai> {
    style::<Monokai>()
        .fill(|theme| theme.panel())
        .frame(|frame, theme| {
            frame
                .padding(UiRect::axes(
                    px(theme.gap() * 2.0),
                    px(theme.gap()),
                ))
                .radius(theme.radius() * 3.0)
                .border(1.0)
                .border_color(theme.hairline())
        })
        .hovered(|s| s.fill(|theme| theme.fill()).tone(Tone::Accent))
        .pressed(|s| {
            s.fill(|theme| theme.pressed())
                .frame(|frame, _| frame.scale(0.96))
                .tone(Tone::Critical)
        })
        .transition(Motion::Interact)
}

fn looks() -> impl View<Bevy, Monokai> {
    column((
        row((
            button(label("Default"))
                .padding(UiRect::axes(px(16.0), px(8.0))),
            button(label("Ghost")).rules(ghost),
            button(row((
                icon(icons::LAYER).size(12.0),
                label("Tint"),
            )))
            .rules(tint),
            button(icon(PLUS)).rules(icon_button),
            button(label("Primary")).rules(primary),
            button(label("Danger")).rules(danger),
            button(label("Custom")).rules(chunky().bundle()),
        ))
        .gap(8.0)
        .align(AlignItems::Center),
        label("Hover to light up, hold the button down to press")
            .tone(Tone::Faint),
    ))
    .gap(8.0)
}

/// Hovering lights every label in the button that did not choose its
/// own tone, and pressing darkens the frame over its hover fill.
fn parts() -> impl View<Bevy, Monokai> {
    row((
        button(
            row((label("Save"), label("Cmd+S").tone(Tone::Dim)))
                .gap(12.0),
        )
        .padding(UiRect::axes(px(10.0), px(4.0)))
        .when::<Hovered, _>(|cx: &mut Build| {
            cx.set::<Label>(|label, _| label.tone(Tone::Accent));
        })
        .when::<Pressed, _>(|cx: &mut Build| {
            cx.root(|cx| {
                cx.set::<Frame>(|frame, theme| {
                    frame.fill(theme.panel())
                });
            });
        })
        .transition(Motion::Interact),
        label("The key keeps its dim tone").tone(Tone::Faint),
    ))
    .gap(10.0)
    .align(AlignItems::Center)
}

/// Rules set in a scope end with it, and an explicit value still
/// beats them.
fn scoped() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        let root = cx
            .build(frame().direction(FlexDirection::Column).gap(4.0));
        cx.under(root, |cx| {
            cx.scope(|cx| {
                cx.set::<Label>(|label, _| label.tone(Tone::Dim));
                cx.build(label(
                    "Inside the scope: dim, from one rule",
                ));
                cx.build(
                    label("An explicit tone still wins")
                        .tone(Tone::Accent),
                );
            });
            cx.build(label("After the scope: back to the preamble"));
        });
        root
    })
}
