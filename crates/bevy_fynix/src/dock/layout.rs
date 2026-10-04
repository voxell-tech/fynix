//! The tree's splits and areas as views.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::input_focus::tab_navigation::TabGroup;
use bevy::picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy::ui::{
    ComputedNode, Display, FlexDirection, Overflow,
    UiGlobalTransform, UiRect, UiScale, percent, px,
};

use super::tree::{
    DockAreaStyle, DockNode, DockTree, NodeId, Shape, SplitAxis,
    TabId,
};
use super::{DockTokens, logical, logical_rect, tabs};
use crate::cursor::{EntityCursor, OverrideCursor};
use crate::prop::resource;
use crate::views::{
    Axis, BehaviorExt, FrameProps, column, divider, frame, revealed,
    row,
};
use crate::{
    AnyView, Bevy, Cx, Dragging, ScopedExt, ViewExt, ViewSeq, each,
};

/// The least a pane keeps when its split is dragged, in logical
/// pixels.
const MIN_PANE: f32 = 48.0;

/// On the handle of a split, which the drag moves.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct SplitHandle {
    pub split: NodeId,
    pub horizontal: bool,
}

/// On the root node of an area.
#[derive(Component, Clone, Copy, Debug)]
pub struct DockArea {
    pub leaf: NodeId,
}

/// The layout of `shape`, or an empty node for none.
pub(super) fn build<T: DockTokens>(
    shape: &Option<Shape>,
) -> AnyView<Bevy, T> {
    match shape {
        Some(shape) => node(shape.clone()),
        None => frame().boxed(),
    }
}

fn node<T: DockTokens>(shape: Shape) -> AnyView<Bevy, T> {
    match shape {
        Shape::Leaf(leaf, style) => area(leaf, style),
        Shape::Split(split, axis, a, b) => {
            self::split(split, axis, *a, *b)
        }
    }
}

fn split<T: DockTokens>(
    id: NodeId,
    axis: SplitAxis,
    a: Shape,
    b: Shape,
) -> AnyView<Bevy, T> {
    let horizontal = axis == SplitAxis::Horizontal;
    let handle = divider(if horizontal {
        Axis::Vertical
    } else {
        Axis::Horizontal
    })
    .rules(revealed)
    .tagged(SplitHandle {
        split: id,
        horizontal,
    });
    let children = (
        pane(id, a, true, horizontal),
        handle,
        pane(id, b, false, horizontal),
    );
    let stack = if horizontal {
        row(children)
    } else {
        column(children)
    };
    stack
        .gap(0.0)
        .width(percent(100.0))
        .height(percent(100.0))
        .min_width(px(0.0))
        .min_height(px(0.0))
        .boxed()
}

/// One side of a split. Its size follows the split's fraction, so a
/// drag writes this and builds nothing.
fn pane<T: DockTokens>(
    split: NodeId,
    child: Shape,
    first: bool,
    horizontal: bool,
) -> AnyView<Bevy, T> {
    let share = resource::<DockTree, _>(move |tree| {
        let fraction = tree
            .get(split)
            .and_then(DockNode::as_split)
            .map_or(0.5, |split| split.fraction);
        percent(100.0 * if first { fraction } else { 1.0 - fraction })
    });
    let pane = column((node(child),))
        .gap(0.0)
        .overflow(Overflow::clip())
        .min_width(px(0.0))
        .min_height(px(0.0));
    if horizontal {
        pane.width(share).height(percent(100.0)).boxed()
    } else {
        pane.width(percent(100.0)).height(share).boxed()
    }
}

/// A leaf: its tab bar over the content of every tab.
fn area<T: DockTokens>(
    leaf: NodeId,
    style: DockAreaStyle,
) -> AnyView<Bevy, T> {
    let contents = each(
        resource::<DockTree, _>(move |tree| {
            tree.leaf(leaf)
                .map(|leaf| leaf.windows.clone())
                .unwrap_or_default()
        }),
        |tab| tab.id,
        move |tab| content(leaf, tab.id, tab.window_id.clone()),
    )
    .within(
        column(())
            .gap(0.0)
            .grow(1.0)
            .width(percent(100.0))
            .min_width(px(0.0))
            .min_height(px(0.0)),
    );
    match style {
        DockAreaStyle::TabBar => {
            shell(leaf, (tabs::bar(leaf), contents))
        }
        DockAreaStyle::Headless => shell(leaf, (contents,)),
    }
}

/// How far an area is set in from its pane. Two areas side by side
/// are the dock's gap apart, counting the split's handle between
/// them.
pub(super) fn inset<T: DockTokens>(theme: &T) -> f32 {
    ((theme.dock_gap() - theme.divider()) / 2.0).max(0.0)
}

/// The root of an area around `children`: a bordered card set in by
/// [`inset`].
fn shell<T: DockTokens, C: ViewSeq<Bevy, T> + 'static>(
    leaf: NodeId,
    children: C,
) -> AnyView<Bevy, T> {
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let theme = cx.theme();
        let inset = inset(theme);
        let radius = theme.dock_radius();
        let border = theme.dock_border();
        let edge = theme.dock_edge();
        cx.build(
            column(children)
                .gap(0.0)
                .grow(1.0)
                .margin(UiRect::all(px(inset)))
                .min_width(px(0.0))
                .min_height(px(0.0))
                .overflow(Overflow::clip())
                .radius(radius)
                .border(border)
                .border_color(edge)
                .tagged(DockArea { leaf }),
        )
    })
}

/// The pane of one tab's window. Switching tabs flips its display and
/// keeps what the window built.
fn content<T: DockTokens>(
    leaf: NodeId,
    tab: TabId,
    window: String,
) -> AnyView<Bevy, T> {
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let window = cx
            .world
            .get_resource::<super::DockRegistry<T>>()
            .and_then(|registry| registry.get(&window))
            .map(|kind| kind.build());
        let pane = cx.build(
            frame()
                .direction(FlexDirection::Column)
                .width(percent(100.0))
                .grow(1.0)
                .min_width(px(0.0))
                .min_height(px(0.0))
                .overflow(Overflow::clip())
                .display(resource::<DockTree, _>(move |tree| {
                    if tree.active(leaf) == Some(tab) {
                        Display::Flex
                    } else {
                        Display::None
                    }
                })),
        );
        // Tab stays among the fields of the window it is pressed in.
        cx.world.entity_mut(pane).insert(TabGroup::default());
        if let Some(window) = window {
            cx.under(pane, |cx| cx.build(window));
        }
        pane
    })
}

/// The fraction of a split a handle at `cursor` makes, for panes
/// spanning `start` to `end` with a `handle` thick line between them.
/// Neither pane gets less than [`MIN_PANE`] when the span allows it.
fn fraction_at(
    cursor: f32,
    start: f32,
    end: f32,
    handle: f32,
) -> Option<f32> {
    let free = end - start - handle;
    if free <= 0.0 {
        return None;
    }
    let least = (MIN_PANE / free).min(0.5);
    let fraction = (cursor - start - handle / 2.0) / free;
    Some(fraction.clamp(least, 1.0 - least))
}

/// The handle `node` is, or the handle whose grab strip it is.
fn handle_of(
    node: Entity,
    is_handle: impl Fn(Entity) -> bool,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    [Some(node), parents.get(node).ok().map(ChildOf::parent)]
        .into_iter()
        .flatten()
        .find(|&node| is_handle(node))
}

/// Holds the handle's resize cursor and marks it [`Dragging`] while
/// it is dragged.
pub(super) fn grab_handle(
    start: On<Pointer<DragStart>>,
    handles: Query<&EntityCursor, With<SplitHandle>>,
    parents: Query<&ChildOf>,
    mut forced: ResMut<OverrideCursor>,
    mut commands: Commands,
) {
    let found = handle_of(
        start.event_target(),
        |node| handles.contains(node),
        &parents,
    );
    if let Some((handle, cursor)) = found
        .and_then(|handle| Some((handle, handles.get(handle).ok()?)))
    {
        forced.0 = Some(cursor.0);
        commands.entity(handle).insert(Dragging);
    }
}

/// Lets go of the cursor held and the [`Dragging`] mark of a handle
/// that was dragged.
pub(super) fn release_handle(
    end: On<Pointer<DragEnd>>,
    handles: Query<(), With<SplitHandle>>,
    parents: Query<&ChildOf>,
    mut forced: ResMut<OverrideCursor>,
    mut commands: Commands,
) {
    if let Some(handle) = handle_of(
        end.event_target(),
        |node| handles.contains(node),
        &parents,
    ) {
        forced.0 = None;
        commands.entity(handle).remove::<Dragging>();
    }
}

/// Moves the split of the handle dragged to follow the pointer.
pub(super) fn drag_handle(
    mut drag: On<Pointer<Drag>>,
    handles: Query<(&SplitHandle, &ChildOf)>,
    parents: Query<&ChildOf>,
    siblings: Query<&Children>,
    rects: Query<(&ComputedNode, &UiGlobalTransform)>,
    scale: Option<Res<UiScale>>,
    mut tree: ResMut<DockTree>,
) {
    let Some(handle) = handle_of(
        drag.event_target(),
        |node| handles.contains(node),
        &parents,
    ) else {
        return;
    };
    let Ok((split, parent)) = handles.get(handle) else {
        return;
    };
    drag.propagate(false);
    let Ok(children) = siblings.get(parent.parent()) else {
        return;
    };
    let Some(at) = children.iter().position(|&kid| kid == handle)
    else {
        return;
    };
    let (Some(&before), Some(&after)) =
        (children.get(at.wrapping_sub(1)), children.get(at + 1))
    else {
        return;
    };
    let rect_of = |node: Entity| {
        rects
            .get(node)
            .map(|(computed, transform)| {
                logical_rect(computed, transform)
            })
            .ok()
    };
    let (Some(before), Some(after)) =
        (rect_of(before), rect_of(after))
    else {
        return;
    };
    let cursor = logical(drag.pointer_location.position, scale);
    let (cursor, start, end, handle) = if split.horizontal {
        (
            cursor.x,
            before.min.x,
            after.max.x,
            after.min.x - before.max.x,
        )
    } else {
        (
            cursor.y,
            before.min.y,
            after.max.y,
            after.min.y - before.max.y,
        )
    };
    let Some(fraction) = fraction_at(cursor, start, end, handle)
    else {
        return;
    };
    let current = tree
        .get(split.split)
        .and_then(DockNode::as_split)
        .map(|split| split.fraction);
    if current != Some(fraction.clamp(0.05, 0.95)) {
        tree.set_fraction(split.split, fraction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handle's thickness.
    const HANDLE: f32 = 2.0;

    #[test]
    fn the_handle_lands_under_the_cursor() {
        // 100px of pane each side of the handle.
        let end = 200.0 + HANDLE;
        let fraction =
            fraction_at(100.0 + HANDLE / 2.0, 0.0, end, HANDLE)
                .unwrap();
        assert_eq!(fraction, 0.5);
        let fraction =
            fraction_at(50.0 + HANDLE / 2.0, 0.0, end, HANDLE)
                .unwrap();
        assert_eq!(fraction, 0.25);
    }

    #[test]
    fn a_pane_keeps_its_least_size() {
        let end = 200.0 + HANDLE;
        let low = fraction_at(-500.0, 0.0, end, HANDLE).unwrap();
        let high = fraction_at(900.0, 0.0, end, HANDLE).unwrap();
        assert_eq!(low, MIN_PANE / 200.0);
        assert_eq!(high, 1.0 - MIN_PANE / 200.0);
    }

    #[test]
    fn a_span_too_small_for_the_handle_is_left_alone() {
        assert_eq!(fraction_at(3.0, 0.0, HANDLE, HANDLE), None);
    }

    #[test]
    fn a_tiny_span_splits_evenly_at_most() {
        let fraction = fraction_at(0.0, 0.0, 60.0, HANDLE).unwrap();
        assert_eq!(fraction, 0.5);
    }
}
