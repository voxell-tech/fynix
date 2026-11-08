//! The scene's assets and layers.

use bevy::ecs::world::World;
use bevy::ui::{AlignItems, JustifyContent, UiRect, percent, px};
use bevy_fynix::icons::PLUS;
use bevy_fynix::tokens::{Motion, Tone};
use bevy_fynix::views::{
    Axis, BehaviorExt, ContextMenuExt, FrameProps, Icon, Label,
    TooltipExt, column, divider, foldable, icon, label, menu_item,
    row, scroll, submenu, tinted_icon,
};
use bevy_fynix::{
    AnyView, Bevy, Hovered, ScopedExt, StateExt, View, ViewExt, each,
    hidden, resource,
};

use crate::common::Monokai;
use crate::{Build, Layers, commands, icons};

const ASSETS: [&str; 4] = [
    "meshes/cube.glb",
    "meshes/sphere.glb",
    "materials/brick",
    "scenes/intro.mox",
];

pub fn scene() -> AnyView<Bevy, Monokai> {
    scroll((
        foldable(
            label("Assets"),
            column(ASSETS.into_iter().map(asset).collect::<Vec<_>>())
                .gap(2.0)
                .padding(UiRect::left(px(20.0))),
        )
        .open(true),
        divider(Axis::Horizontal),
        row((
            label("Layers").tone(Tone::Dim),
            tinted_icon(PLUS)
                .on_activate(commands::run(commands::NEW_LAYER))
                .tooltip(|| label("New layer")),
        ))
        .justify(JustifyContent::SpaceBetween)
        .align(AlignItems::Center),
        // A layer that stays keeps its node through a reorder. A new
        // one opens its space and fades in, and a removed one fades
        // out and closes it.
        each(
            resource::<Layers, _>(|layers| layers.ids.clone()),
            |id| *id,
            |id| layer(*id).appear::<Monokai>(hidden).boxed(),
        )
        .within(column(()).gap(2.0)),
    ))
    .width(percent(100.0))
    .height(percent(100.0))
    .padding(UiRect::all(px(12.0)))
    .gap(10.0)
    .boxed()
}

/// A row that lights its label and icon under the pointer, over the
/// theme's curve.
fn lit(view: impl View<Bevy, Monokai>) -> impl View<Bevy, Monokai> {
    view.when::<Hovered, _>(|cx: &mut Build| {
        cx.set::<Label>(|label, _| label.tone(Tone::Accent));
        cx.set::<Icon>(|icon, _| icon.tone(Tone::Accent));
    })
    .transition(Motion::Interact)
}

fn asset(path: &'static str) -> AnyView<Bevy, Monokai> {
    lit(label(path).tone(Tone::Dim))
        .context_menu(move || {
            (menu_item(label("Add as a layer"))
                .on_activate(commands::run(commands::NEW_LAYER)),)
        })
        .boxed()
}

fn layer(id: u32) -> impl View<Bevy, Monokai> {
    lit(row((
        icon(icons::LAYER).size(12.0).tone(Tone::Dim),
        label(format!("Layer {id}")),
    ))
    .gap(8.0)
    .align(AlignItems::Center)
    .padding(UiRect::vertical(px(2.0))))
    .context_menu(move || {
        (
            menu_item(label("Duplicate"))
                .on_activate(move |world| duplicate(world, id)),
            submenu(label("Move"), move || {
                (
                    menu_item(label("To the top"))
                        .on_activate(move |world| to(world, id, 0)),
                    menu_item(label("To the bottom")).on_activate(
                        move |world| to(world, id, usize::MAX),
                    ),
                )
            }),
            menu_item(label("Delete"))
                .on_activate(move |world| delete(world, id)),
        )
    })
}

fn duplicate(world: &mut World, id: u32) {
    let mut layers = world.resource_mut::<Layers>();
    let copy = layers.next;
    layers.next += 1;
    let at = layers.ids.iter().position(|&each| each == id);
    layers.ids.insert(at.map_or(0, |at| at + 1), copy);
}

/// Moves the layer `id` to `at`, or to the bottom past the end.
fn to(world: &mut World, id: u32, at: usize) {
    let mut layers = world.resource_mut::<Layers>();
    layers.ids.retain(|&each| each != id);
    let at = at.min(layers.ids.len());
    layers.ids.insert(at, id);
}

fn delete(world: &mut World, id: u32) {
    world
        .resource_mut::<Layers>()
        .ids
        .retain(|&each| each != id);
}
