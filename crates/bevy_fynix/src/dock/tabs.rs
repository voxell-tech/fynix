//! A leaf's tab bar: a scrolling row of tabs and the "+" button.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ui::{
    AlignItems, FlexDirection, Overflow, UiRect, percent, px,
};
use bevy::window::SystemCursorIcon;

use super::DockTokens;
use super::registry::DockRegistry;
use super::tree::{DockTabEntry, DockTree, NodeId, TabId};
use crate::cursor::EntityCursor;
use crate::prop::resource;
use crate::tokens::Tone;
use crate::views::{
    BehaviorExt, Frame, FrameProps, Icon, Label, button, icon, label,
    row, scroll,
};
use crate::{AnyView, Bevy, Cx, Hovered, StateExt, ViewExt, each};

/// On the root node of a tab.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockTab {
    pub leaf: NodeId,
    pub tab: TabId,
}

/// On the root node of the tab an area shows, for state rules to wait
/// on.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ActiveTab;

/// On the node holding the tabs of a bar, whose children are the
/// tabs.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct TabRow {
    pub leaf: NodeId,
}

/// On the "+" button at the end of a bar.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct AddButton {
    pub leaf: NodeId,
}

/// The bar of `leaf`.
pub(super) fn bar<T: DockTokens>(leaf: NodeId) -> AnyView<Bevy, T> {
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let height = cx.theme().row();
        let fill = cx.theme().panel();
        let tabs = each(
            resource::<DockTree, _>(move |tree| {
                tree.leaf(leaf)
                    .map(|leaf| leaf.windows.clone())
                    .unwrap_or_default()
            }),
            |tab| tab.id,
            move |tab| self::tab(leaf, tab),
        )
        .within(
            scroll(())
                .with(TabRow { leaf })
                .direction(FlexDirection::Row)
                .overflow(Overflow::scroll_x())
                .gap(2.0)
                .height(percent(100.0))
                .align(AlignItems::Center),
        );
        let add = button(label("+"))
            .fill(Color::NONE)
            .padding(UiRect::axes(px(8.0), px(0.0)))
            .height(percent(100.0))
            .shrink(0.0)
            .tagged(AddButton { leaf });
        cx.build(
            row((tabs, add))
                .gap(0.0)
                .width(percent(100.0))
                .height(px(height))
                .shrink(0.0)
                .align(AlignItems::Center)
                .fill(fill),
        )
    })
}

/// The rules of the active tab: a fill, and its text and icons lit.
fn lit<T: DockTokens>(cx: &mut Cx<'_, Bevy, T>) {
    cx.set::<Label>(|label, _| label.tone(Tone::Body));
    cx.set::<Icon>(|icon, _| icon.tone(Tone::Body));
    cx.root(|cx| {
        cx.set::<Frame>(|frame, theme: &T| frame.fill(theme.fill()));
    });
}

/// The rules of a hovered tab.
fn hovered<T: DockTokens>(cx: &mut Cx<'_, Bevy, T>) {
    cx.root(|cx| {
        cx.set::<Frame>(|frame, theme: &T| frame.fill(theme.hover()));
    });
}

/// One tab: its icon, its name and a close button. A click makes it
/// active. It is dim and unfilled until it is.
fn tab<T: DockTokens>(
    leaf: NodeId,
    entry: &DockTabEntry,
) -> AnyView<Bevy, T> {
    let DockTabEntry { window_id, id } = entry.clone();
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let kind = cx
            .world
            .get_resource::<DockRegistry<T>>()
            .and_then(|registry| registry.get(&window_id));
        let name = kind.map_or_else(
            || window_id.clone(),
            |kind| kind.name.clone(),
        );
        let image = kind.and_then(|kind| kind.icon.clone());
        let mut parts = Vec::new();
        if let Some(image) = image {
            parts.push(icon(image).size(12.0).boxed());
        }
        parts.push(label(name).bold(true).wrap(false).boxed());
        parts.push(close(id));

        let tab =
            button(row(parts).gap(6.0).align(AlignItems::Center))
                .fill(Color::NONE)
                .padding(UiRect::axes(px(8.0), px(3.0)))
                .height(percent(100.0))
                .shrink(0.0)
                .tagged(DockTab { leaf, tab: id })
                .tagged(EntityCursor(SystemCursorIcon::Grab))
                .on_activate(move |world| {
                    world
                        .resource_mut::<DockTree>()
                        .set_active(leaf, id);
                })
                .when::<ActiveTab, _>(lit::<T>)
                .when::<Hovered, _>(hovered::<T>)
                .toned(Tone::Dim);
        // Marked from the start when its area shows it, so it is
        // drawn lit at the first write.
        let shown =
            cx.world.resource::<DockTree>().active(leaf) == Some(id);
        let node = if shown {
            cx.build(tab.seeded(ActiveTab))
        } else {
            cx.build(tab)
        };
        // Marks the tab while its area shows it.
        cx.effect(
            node,
            resource::<DockTree, _>(move |tree| {
                tree.active(leaf) == Some(id)
            })
            .into(),
            |world, node, &active| {
                let Ok(mut entity) = world.get_entity_mut(node)
                else {
                    return;
                };
                if active {
                    if !entity.contains::<ActiveTab>() {
                        entity.insert(ActiveTab);
                    }
                } else {
                    entity.remove::<ActiveTab>();
                }
            },
        );
        node
    })
}

/// The button closing `tab`, dim until the pointer is over it.
fn close<T: DockTokens>(tab: TabId) -> AnyView<Bevy, T> {
    button(label("x"))
        .fill(Color::NONE)
        .padding(UiRect::axes(px(4.0), px(0.0)))
        .on_activate(move |world| {
            world.resource_mut::<DockTree>().remove_tab(tab);
        })
        .when::<Hovered, _>(|cx: &mut Cx<'_, Bevy, T>| {
            cx.set::<Label>(|label, _| label.tone(Tone::Critical));
        })
        .toned(Tone::Dim)
        .boxed()
}
