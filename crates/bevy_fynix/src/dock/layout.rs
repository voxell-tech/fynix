//! The tree's splits and areas as views.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::picking::events::{Drag, Pointer};
use bevy::ui::{
    ComputedNode, Display, FlexDirection, Overflow,
    UiGlobalTransform, UiScale, percent, px,
};

use super::tree::{
    DockAreaStyle, DockNode, DockTree, NodeId, Shape, SplitAxis,
    TabId,
};
use super::{DockTokens, logical, logical_rect, tabs};
use crate::prop::resource;
use crate::views::{
    Axis, BehaviorExt, FrameProps, column, divider, frame, row,
};
use crate::{AnyView, Bevy, Cx, ViewExt, ViewSeq, each};

/// How thick the line between two panes is.
const HANDLE: f32 = 6.0;

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
    .thickness(HANDLE)
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

/// The root of an area around `children`.
fn shell<T: DockTokens, C: ViewSeq<Bevy, T> + 'static>(
    leaf: NodeId,
    children: C,
) -> AnyView<Bevy, T> {
    column(children)
        .gap(0.0)
        .width(percent(100.0))
        .height(percent(100.0))
        .min_width(px(0.0))
        .min_height(px(0.0))
        .overflow(Overflow::clip())
        .tagged(DockArea { leaf })
        .boxed()
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
        if let Some(window) = window {
            cx.under(pane, |cx| cx.build(window));
        }
        pane
    })
}

/// The fraction of a split a handle at `cursor` makes, for panes
/// spanning `start` to `end` with the handle between them. Neither
/// pane gets less than [`MIN_PANE`] when the span allows it.
fn fraction_at(cursor: f32, start: f32, end: f32) -> Option<f32> {
    let free = end - start - HANDLE;
    if free <= 0.0 {
        return None;
    }
    let least = (MIN_PANE / free).min(0.5);
    let fraction = (cursor - start - HANDLE / 2.0) / free;
    Some(fraction.clamp(least, 1.0 - least))
}

/// Moves the split of the handle dragged to follow the pointer.
pub(super) fn drag_handle(
    mut drag: On<Pointer<Drag>>,
    handles: Query<(&SplitHandle, &ChildOf)>,
    siblings: Query<&Children>,
    rects: Query<(&ComputedNode, &UiGlobalTransform)>,
    scale: Option<Res<UiScale>>,
    mut tree: ResMut<DockTree>,
) {
    let handle = drag.event_target();
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
    let (cursor, start, end) = if split.horizontal {
        (cursor.x, before.min.x, after.max.x)
    } else {
        (cursor.y, before.min.y, after.max.y)
    };
    let Some(fraction) = fraction_at(cursor, start, end) else {
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

    #[test]
    fn the_handle_lands_under_the_cursor() {
        // Panes span 0..206 around a 6px handle: 100px each side.
        let fraction = fraction_at(103.0, 0.0, 206.0).unwrap();
        assert_eq!(fraction, 0.5);
        let fraction = fraction_at(53.0, 0.0, 206.0).unwrap();
        assert_eq!(fraction, 0.25);
    }

    #[test]
    fn a_pane_keeps_its_least_size() {
        let low = fraction_at(-500.0, 0.0, 206.0).unwrap();
        let high = fraction_at(900.0, 0.0, 206.0).unwrap();
        assert_eq!(low, MIN_PANE / 200.0);
        assert_eq!(high, 1.0 - MIN_PANE / 200.0);
    }

    #[test]
    fn a_span_too_small_for_the_handle_is_left_alone() {
        assert_eq!(fraction_at(3.0, 0.0, HANDLE), None);
    }

    #[test]
    fn a_tiny_span_splits_evenly_at_most() {
        let fraction = fraction_at(0.0, 0.0, 60.0).unwrap();
        assert_eq!(fraction, 0.5);
    }
}
