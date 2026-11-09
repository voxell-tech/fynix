//! Text with chips in it: inline atoms in a rich text, and in a field
//! that can be edited.

use core::sync::atomic::{AtomicUsize, Ordering};

use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::world::World;
use bevy::ui::{AlignItems, UiRect, px};
use bevy_fynix::tokens::{SpacingTokens, SurfaceTokens, Tone};
use bevy_fynix::views::{
    Atom, BehaviorExt, FrameProps, Piece, RichField, Span, button,
    column, icon, label, rich, rich_field, row,
};
use bevy_fynix::{AnyView, Bevy, View, resource};

use crate::common::Monokai;
use crate::{Build, icons};

/// The plain text of the field, as it was at its last change.
#[derive(Resource, Default)]
pub struct Typed(pub String);

#[derive(Component)]
struct Field;

fn chip_view(
    name: &'static str,
    size: &'static str,
) -> AnyView<Bevy, Monokai> {
    AnyView::new(move |cx: &mut Build| {
        let theme = cx.theme();
        let fill = theme.fill();
        let hairline = theme.hairline();
        let radius = theme.radius();
        cx.build(
            row((
                icon(icons::FILE).size(12.0).tone(Tone::Accent),
                label(name).size(12.0),
                label(size).size(11.0).tone(Tone::Faint),
            ))
            .gap(5.0)
            .align(AlignItems::Center)
            .padding(UiRect::axes(px(6.0), px(1.0)))
            .radius(radius)
            .fill(fill)
            .border(1.0)
            .border_color(hairline),
        )
    })
}

fn chip(
    key: impl Into<String>,
    name: &'static str,
    size: &'static str,
) -> Atom {
    Atom::new::<Monokai>(
        key,
        format!("[{name}, {size}]"),
        move || chip_view(name, size),
    )
}

fn text(text: &str) -> Piece {
    Piece::Text(text.into())
}

pub fn chips() -> impl View<Bevy, Monokai> {
    column((
        rich(vec![
            Span::new("Compare "),
            Span::atom(chip("a", "monokai_pro.typ", "1 KB")),
            Span::new(" with "),
            Span::atom(chip("b", "sunset.typ", "3 KB")),
            Span::new(" before you go on."),
        ])
        .atoms(),
        row((
            rich_field()
                .placeholder("Type here, or insert a chip")
                .max_lines(4)
                .content([
                    text("Look at "),
                    Piece::Atom(chip("c", "example.typ", "2 KB")),
                    text(
                        " example!\nEnter starts a new line, and a \
                         long one wraps at the width of the field.",
                    ),
                ])
                .on_change(|world, _| {
                    let plain = world
                        .query_filtered::<&RichField, With<Field>>()
                        .single(world)
                        .map(|field| field.plain().text);
                    if let Ok(plain) = plain {
                        world.resource_mut::<Typed>().0 = plain;
                    }
                })
                .width(px(420.0))
                .tagged(Field),
            button(label("Insert chip"))
                .padding(UiRect::axes(px(12.0), px(4.0)))
                .on_activate(insert),
        ))
        .gap(8.0)
        .align(AlignItems::Start),
        label(resource::<Typed, _>(|typed| typed.0.clone()))
            .size(11.0)
            .tone(Tone::Faint),
    ))
    .gap(8.0)
}

fn insert(world: &mut World) {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    let count = COUNT.fetch_add(1, Ordering::Relaxed);
    let atom = chip(format!("chip{count}"), "notes.typ", "4 KB");
    for mut field in world
        .query_filtered::<&mut RichField, With<Field>>()
        .iter_mut(world)
    {
        field.insert_atom(atom.clone());
    }
}
