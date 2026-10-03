//! The list the "+" button of a bar opens: the windows that can be
//! added.

use bevy::asset::Handle;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::ui::AlignItems;
use bevy::ui_widgets::popover::{
    PopoverAlign, PopoverPlacement, PopoverSide,
};

use super::tree::{DockTree, NodeId};
use super::{DockRegistry, DockTokens};
use crate::tokens::Tone;
use crate::views::{
    BehaviorExt, FrameProps, icon, label, menu_item, row, shut,
};
use crate::{AnyView, Bevy, ViewExt};

/// A window that can be added: its id, name and icon.
type Choice = (String, String, Option<Handle<Image>>);

/// Below the button, right aligned where it fits, `gap` away.
pub(super) fn placements(gap: f32) -> Vec<PopoverPlacement> {
    [
        (PopoverSide::Bottom, PopoverAlign::End),
        (PopoverSide::Bottom, PopoverAlign::Start),
        (PopoverSide::Top, PopoverAlign::End),
        (PopoverSide::Top, PopoverAlign::Start),
    ]
    .into_iter()
    .map(|(side, align)| PopoverPlacement { side, align, gap })
    .collect()
}

/// A row per registered window not in the tree yet, each adding its
/// window to `leaf` and shutting the list of the button at `root`.
pub(super) fn rows<T: DockTokens>(
    world: &World,
    leaf: NodeId,
    root: Entity,
) -> Vec<AnyView<Bevy, T>> {
    let tree = world.resource::<DockTree>();
    let choices = world
        .get_resource::<DockRegistry<T>>()
        .map(|registry| {
            registry
                .iter()
                .filter(|(id, _)| {
                    tree.find_leaf_with_window(id).is_none()
                })
                .map(|(id, kind)| {
                    (
                        id.to_string(),
                        kind.name.clone(),
                        kind.icon.clone(),
                    )
                })
                .collect::<Vec<Choice>>()
        })
        .unwrap_or_default();

    if choices.is_empty() {
        // A row, so the list has something to hold the focus that
        // keeps it open.
        return vec![
            menu_item(label("Nothing left to add").tone(Tone::Dim))
                .on_activate(move |world| shut(world, root))
                .boxed(),
        ];
    }
    choices
        .into_iter()
        .map(|choice| choice_row(leaf, root, choice))
        .collect()
}

/// The row adding `choice` to `leaf`.
fn choice_row<T: DockTokens>(
    leaf: NodeId,
    root: Entity,
    (id, name, image): Choice,
) -> AnyView<Bevy, T> {
    let mut parts = Vec::new();
    if let Some(image) = image {
        parts.push(icon(image).boxed());
    }
    parts.push(label(name).boxed());
    menu_item(row(parts).align(AlignItems::Center))
        .on_activate(move |world| {
            world
                .resource_mut::<DockTree>()
                .add_tab(leaf, id.clone());
            shut(world, root);
        })
        .boxed()
}
