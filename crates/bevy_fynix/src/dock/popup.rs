//! The popup the "+" button of a bar opens, listing the windows that
//! can be added.

use bevy::asset::Handle;
use bevy::ecs::component::Component;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::image::Image;
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Pointer};
use bevy::ui::{
    AlignItems, ComputedNode, UiGlobalTransform, UiRect, Val, px,
};
use bevy::ui_widgets::Activate;

use super::tabs::AddButton;
use super::tree::{DockTree, NodeId};
use super::{DockRegistry, DockRoot, DockTokens, logical_rect};
use crate::tokens::Tone;
use crate::views::{
    BehaviorExt, FrameProps, column, icon, label, menu_item,
    menu_surface, overlay, row,
};
use crate::{AnyView, Bevy, Cx, ScopedExt, ViewExt};

/// Where the popup sits, in the dock's own coordinates: `right` from
/// the dock's right edge and `top` from its top.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenPopup {
    pub leaf: NodeId,
    pub right: f32,
    pub top: f32,
}

/// The open popup, if any.
#[derive(Resource, Default, Debug)]
pub struct AddPopup {
    pub open: Option<OpenPopup>,
}

/// On the node behind the popup, which closes it when clicked.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct Backdrop;

/// Opens the popup under the "+" button that was activated, right
/// aligned to it.
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
    let dock = parents
        .iter_ancestors(button)
        .find_map(|ancestor| roots.get(ancestor).ok())
        .map_or(rect, |(computed, transform)| {
            logical_rect(computed, transform)
        });
    popup.open = Some(OpenPopup {
        leaf: add.leaf,
        right: (dock.max.x - rect.max.x).max(0.0),
        top: rect.max.y - dock.min.y,
    });
}

/// Closes the popup on a click that lands on its backdrop.
pub(super) fn dismiss(
    click: On<Pointer<Click>>,
    backdrops: Query<(), With<Backdrop>>,
    mut popup: ResMut<AddPopup>,
) {
    if backdrops.contains(click.original_event_target()) {
        popup.open = None;
    }
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

/// The popup at `open`. It lists the registered windows that are not
/// in the tree yet.
fn menu<T: DockTokens>(open: OpenPopup) -> AnyView<Bevy, T> {
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let (width, offset) =
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
        let menu = column(rows)
            .inset(UiRect::new(
                Val::Auto,
                px(open.right),
                px(open.top + offset),
                Val::Auto,
            ))
            .min_width(px(width))
            .rules(|cx: &mut Cx<'_, Bevy, T>| {
                cx.defaults(menu_surface);
            });
        cx.build(
            overlay((menu,))
                .with(Pickable::default())
                .with(Backdrop)
                .z(Some(180)),
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
