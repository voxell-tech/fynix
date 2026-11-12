//! The shape's fields, and the scene's on a second page.

use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::world::World;
use bevy::ui::{UiRect, percent, px};
use bevy_fynix::icons::CHEVRON;
use bevy_fynix::prop::Prop;
use bevy_fynix::tokens::{MotionTokens, Tone};
use bevy_fynix::views::{
    AnimatedField, BehaviorExt, FrameProps, HasAction, button,
    checkbox, column, dropdown, field_row, label, number_field, row,
    scroll, segmented, slider, text_field,
};
use bevy_fynix::{
    AnyView, Bevy, ReducedMotion, ScopedExt, StateExt, View, ViewExt,
    hidden, keyed, resource,
};

use crate::common::Monokai;
use crate::timeline::EASINGS;
use crate::{Easing, FollowLog, Page, Shape};

pub fn inspector() -> AnyView<Bevy, Monokai> {
    scroll((
        segmented(
            ["Shape", "Scene"],
            resource::<Page, _>(|page| page.0),
            |world, at| world.resource_mut::<Page>().0 = at,
        ),
        // Each page is built when it is shown. The last one fades
        // out and closes its space, then the next opens and fades
        // in.
        keyed(
            resource::<Page, _>(|page| page.0),
            |page| match page {
                0 => shape_page().boxed(),
                _ => scene_page().boxed(),
            },
        )
        .presence(Monokai::expand)
        .within(column(())),
    ))
    .width(percent(100.0))
    .height(percent(100.0))
    .padding(UiRect::all(px(12.0)))
    .gap(12.0)
    .boxed()
}

/// A page that eases in and out as it is switched.
fn page(rows: impl View<Bevy, Monokai>) -> impl View<Bevy, Monokai> {
    column((rows,))
        .gap(6.0)
        .appear::<Monokai>(hidden)
        .transition(Monokai::expand)
}

/// A number field for one of the shape's fields.
fn number(
    read: fn(&Shape) -> f32,
    write: fn(&mut Shape, f32),
    most: f32,
) -> impl View<Bevy, Monokai> {
    number_field(resource::<Shape, _>(read), move |world, value| {
        write(&mut world.resource_mut::<Shape>(), value)
    })
    .range(0.0, most)
    .width(px(90.0))
}

/// The shape's fields. The ones the clip moves turn accent while they
/// hold keyframes, which the button below toggles.
fn shape_page() -> impl View<Bevy, Monokai> {
    page(column((
        field_row(
            label("name"),
            text_field(
                resource::<Shape, _>(|shape| shape.name.clone()),
                |world, name| {
                    world.resource_mut::<Shape>().name = name
                },
            )
            .width(px(140.0)),
        ),
        field_row(
            label("x").animatable::<Monokai>("x"),
            number(|s| s.x, |s, x| s.x = x, 600.0),
        ),
        field_row(label("y"), number(|s| s.y, |s, y| s.y = y, 400.0)),
        field_row(
            label("size"),
            number(|s| s.size, |s, size| s.size = size, 240.0),
        ),
        field_row(
            label("travel").animatable::<Monokai>("travel"),
            number(
                |s| s.travel,
                |s, travel| s.travel = travel,
                600.0,
            ),
        ),
        field_row(
            label("radius"),
            slider(resource::<Shape, _>(|shape| shape.radius))
                .range(0.0, 60.0)
                .on_change(|world, radius| {
                    world.resource_mut::<Shape>().radius = radius;
                })
                .width(px(140.0)),
        ),
        field_row(
            label("opacity"),
            slider(resource::<Shape, _>(|shape| shape.opacity))
                .on_change(|world, opacity| {
                    world.resource_mut::<Shape>().opacity = opacity;
                })
                .width(px(140.0)),
        ),
        field_row(
            label("visible"),
            toggle(
                resource::<Shape, _>(|shape| shape.visible),
                |world, on| {
                    world.resource_mut::<Shape>().visible = on
                },
            ),
        ),
        field_row(
            label("easing"),
            dropdown(
                EASINGS,
                resource::<Easing, _>(|easing| easing.0),
                CHEVRON,
                |world, at| world.resource_mut::<Easing>().0 = at,
            )
            .width(px(140.0)),
        ),
        // A row of its own, so the column does not stretch it.
        row((button(label("Toggle keyframes"))
            .padding(UiRect::axes(px(10.0), px(4.0)))
            .on_activate(toggle_keyframes),)),
    )))
}

fn scene_page() -> impl View<Bevy, Monokai> {
    page(column((
        field_row(
            label("reduced motion"),
            toggle(
                resource::<ReducedMotion, _>(|reduced| reduced.0),
                |world, on| {
                    world.resource_mut::<ReducedMotion>().0 = on
                },
            ),
        ),
        field_row(
            label("follow the log"),
            toggle(
                resource::<FollowLog, _>(|follow| follow.0),
                |world, on| world.resource_mut::<FollowLog>().0 = on,
            ),
        ),
        field_row(
            label("a long field name, kept on one line"),
            label("plain").tone(Tone::Dim),
        ),
    )))
}

/// A checkbox that writes with `write`, in a row of its own so the
/// field row does not stretch it.
fn toggle(
    checked: impl Into<Prop<bool>>,
    write: fn(&mut World, bool),
) -> impl View<Bevy, Monokai> {
    row((checkbox(checked).on_change(write),))
}

fn toggle_keyframes(world: &mut World) {
    let fields = world
        .query_filtered::<Entity, With<AnimatedField>>()
        .iter(world)
        .collect::<Vec<_>>();
    for field in fields {
        let mut field = world.entity_mut(field);
        if field.contains::<HasAction>() {
            field.remove::<HasAction>();
        } else {
            field.insert(HasAction);
        }
    }
}
