//! The popup the "+" button of a bar opens, listing the windows that
//! can be added.

use bevy::asset::Handle;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::image::Image;
use bevy::math::{Rect, Vec2};
use bevy::ui::{AlignItems, ComputedNode, UiGlobalTransform, px};
use bevy::ui_widgets::Activate;
use bevy::ui_widgets::popover::{
    PopoverAlign, PopoverPlacement, PopoverSide,
};

use super::tabs::AddButton;
use super::tree::{DockTree, NodeId};
use super::{DockRegistry, DockRoot, DockTokens, logical_rect};
use crate::tokens::Tone;
use crate::views::{
    BehaviorExt, FrameProps, icon, label, menu_item, popup, row,
};
use crate::{AnyView, Bevy, Cx, ViewExt};

/// The open popup: the leaf it adds to, and the rect of the "+"
/// button that opened it, in the dock's own coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenPopup {
    pub leaf: NodeId,
    pub anchor: Rect,
}

/// The open popup, if any.
#[derive(Resource, Default, Debug)]
pub struct AddPopup {
    pub open: Option<OpenPopup>,
}

/// Opens the popup under the "+" button that was activated.
pub(super) fn open(
    activate: On<Activate>,
    buttons: Query<(&AddButton, &ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    roots: Query<(&ComputedNode, &UiGlobalTransform), With<DockRoot>>,
    mut popup: ResMut<AddPopup>,
) {
    let button = activate.event_target();
    let Ok((add, computed, transform)) = buttons.get(button) else {
        return;
    };
    let rect = logical_rect(computed, transform);
    let origin = parents
        .iter_ancestors(button)
        .find_map(|ancestor| roots.get(ancestor).ok())
        .map_or(Vec2::ZERO, |(computed, transform)| {
            logical_rect(computed, transform).min
        });
    popup.open = Some(OpenPopup {
        leaf: add.leaf,
        anchor: Rect::from_corners(
            rect.min - origin,
            rect.max - origin,
        ),
    });
}

/// The popup, or an empty node while none is open.
pub(super) fn build<T: DockTokens>(
    open: &Option<OpenPopup>,
) -> AnyView<Bevy, T> {
    match open {
        Some(open) => menu(open.clone()),
        None => row(()).boxed(),
    }
}

/// A window that can be added: its id, name and icon.
type Choice = (String, String, Option<Handle<Image>>);

/// The popup at `open`, right aligned under its button where it fits.
/// It lists the registered windows that are not in the tree yet.
fn menu<T: DockTokens>(open: OpenPopup) -> AnyView<Bevy, T> {
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let (width, gap) =
            (cx.theme().menu_width(), cx.theme().menu_padding());
        let tree = cx.world.resource::<DockTree>();
        let choices = cx
            .world
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
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let rows = if choices.is_empty() {
            vec![label("Nothing left to add").tone(Tone::Dim).boxed()]
        } else {
            choices
                .into_iter()
                .map(|choice| choice_row(open.leaf, choice))
                .collect()
        };
        let placements = [
            (PopoverSide::Bottom, PopoverAlign::End),
            (PopoverSide::Bottom, PopoverAlign::Start),
            (PopoverSide::Top, PopoverAlign::End),
            (PopoverSide::Top, PopoverAlign::Start),
        ]
        .into_iter()
        .map(|(side, align)| PopoverPlacement { side, align, gap })
        .collect();
        cx.build(
            popup(open.anchor, rows)
                .placements(placements)
                .on_dismiss(|world, _| {
                    world.resource_mut::<AddPopup>().open = None;
                })
                .min_width(px(width)),
        )
    })
}

/// The row adding `choice` to `leaf`.
fn choice_row<T: DockTokens>(
    leaf: NodeId,
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
            world.resource_mut::<AddPopup>().open = None;
        })
        .boxed()
}
