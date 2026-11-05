//! One sideways scroll shared by every row of a virtual list.

use core::ops::Range;

use bevy::ecs::change_detection::Mut;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::system::Query;
use bevy::ui::{
    ComputedNode, Node, Overflow, ScrollPosition, UiTransform, Val,
    Val2, percent, px,
};

use super::{Placed, RowSlot, VirtualList, VirtualListView, reveal};
use crate::prop::Prop;
use crate::{AnyView, Bevy, Cx};

impl<R> VirtualListView<R> {
    /// Lets the list scroll sideways while `on` holds, by one offset
    /// for every row and with one bar. Only the [`RowSlot::sideways`]
    /// cells of a row move. The list scrolls as far as the content of
    /// the widest such cell of any row laid out so far overflows its
    /// cell, so a wide row that scrolls out of sight keeps the range.
    /// A change in the area's width forgets the rows out of sight
    /// until they show again.
    ///
    /// Turning it off or on keeps the first visible row in place and
    /// forgets how far rows overflow and the heights they were laid
    /// out at, which wrapping changes.
    pub fn sideways(mut self, on: impl Into<Prop<bool>>) -> Self {
        self.sideways = on.into();
        self
    }
}

impl RowSlot {
    /// A cell of the row that clips `content` to the room the row
    /// leaves it. While the list scrolls
    /// [sideways](VirtualListView::sideways), `content` keeps its own
    /// width and moves with the list's sideways offset, and the rest
    /// of the row stays. Otherwise `content` takes the cell's width,
    /// so text that wraps wraps inside it.
    pub fn sideways<T: 'static>(
        self,
        content: AnyView<Bevy, T>,
    ) -> AnyView<Bevy, T> {
        AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
            let cell = cx.spawn();
            *cx.world.get_mut::<Node>(cell).expect("a node") = Node {
                flex_grow: 1.0,
                flex_basis: px(0.0),
                min_width: px(0.0),
                overflow: Overflow::clip_x(),
                ..Node::default()
            };
            let sideways = cx
                .world
                .get::<VirtualList>(self.list)
                .is_some_and(|list| list.widths.is_some());
            let moved = cx.under(cell, Cx::spawn);
            let mut node = cx.world.entity_mut(moved);
            node.insert(SidewaysContent(self));
            fit(
                &mut node.get_mut::<Node>().expect("a node"),
                sideways,
            );
            cx.under(moved, |cx| cx.build(content));
            cell
        })
    }
}

impl VirtualList {
    /// Scrolls sideways as little as shows `x` of the rows'
    /// [`sideways`](RowSlot::sideways) content, in pixels from its
    /// left, at the next update. A range wider than the cells scrolls
    /// as little as fills them with it. It goes no further than the
    /// widest row laid out so far overflows, and is placed again each
    /// frame until a frame leaves the offset where the frame before
    /// did, so a row that grows wide in the same frame shows. A
    /// sideways scroll before then stops it. It does nothing unless
    /// the list scrolls [sideways](VirtualListView::sideways).
    pub fn scroll_x_into_view(&mut self, x: Range<f32>) {
        if self.widths.is_some() {
            self.reveal_x = Some(RevealX {
                x: (x.start, x.end),
                placed: None,
            });
        }
    }
}

/// The node under a [`RowSlot::sideways`] cell that moves with the
/// list's sideways offset, and the slot it is in.
#[derive(Component)]
pub(super) struct SidewaysContent(RowSlot);

/// Sizes a moved node to its content while the list scrolls sideways
/// and to its cell otherwise. Takes the `Mut`, not `&mut Node`, so a
/// node that already fits is not marked changed: a changed `Node`
/// makes bevy_ui lay it out again.
pub(super) fn fit(node: &mut Mut<Node>, sideways: bool) {
    let (width, shrink) = if sideways {
        (Val::Auto, 0.0)
    } else {
        (percent(100.0), 1.0)
    };
    if node.width != width || node.flex_shrink != shrink {
        node.width = width;
        node.flex_shrink = shrink;
    }
}

/// A sideways scroll into view on its way: the span of the content
/// to show, and where the frame before left the offset, `None` on
/// the frame it was asked for.
#[derive(Clone, Copy)]
pub(super) struct RevealX {
    x: (f32, f32),
    placed: Option<f32>,
}

/// How far each row's sideways content overflows its cell, and the
/// most of any, which is how far the list scrolls sideways.
#[derive(Default)]
pub(super) struct Widths {
    each: Vec<f32>,
    widest: f32,
}

impl Widths {
    pub(super) fn forget(&mut self) {
        self.each.fill(0.0);
        self.widest = 0.0;
    }

    fn set(&mut self, index: usize, by: f32) {
        let Some(at) = self.each.get_mut(index) else {
            return;
        };
        let was = core::mem::replace(at, by);
        if by >= self.widest {
            self.widest = by;
        } else if was == self.widest {
            self.widest = self.most();
        }
    }

    pub(super) fn splice(
        &mut self,
        at: usize,
        removed: usize,
        inserted: usize,
    ) {
        let Some(gone) = self.each.get(at..at + removed) else {
            return;
        };
        let widest = self.widest > 0.0 && gone.contains(&self.widest);
        self.each.splice(
            at..at + removed,
            core::iter::repeat_n(0.0, inserted),
        );
        if widest {
            self.widest = self.most();
        }
    }

    pub(super) fn resize(&mut self, count: usize) {
        let shrinks = count < self.each.len();
        self.each.resize(count, 0.0);
        if shrinks {
            self.widest = self.most();
        }
    }

    fn most(&self) -> f32 {
        self.each.iter().copied().fold(0.0, f32::max)
    }
}

/// The moved node of every sideways cell, with its cell, its laid-out
/// size and the transform that moves it.
pub(super) type Cells<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SidewaysContent,
        &'static ChildOf,
        &'static ComputedNode,
        &'static mut UiTransform,
    ),
>;

/// How far each built row's sideways content overflows its cell, in
/// logical px, and the cells' width.
pub(super) fn overflows(
    list: Entity,
    slots: &[Placed],
    cells: &Cells,
    nodes: &Query<(&ComputedNode, &mut Node)>,
    scale: f32,
) -> (Vec<(usize, f32)>, Option<f32>) {
    let mut wide = vec![None::<f32>; slots.len()];
    let mut width = None;
    for (_, cell, room, content, _) in cells {
        let RowSlot { list: of, slot } = cell.0;
        let Ok((room, _)) = nodes.get(room.parent()) else {
            continue;
        };
        if of != list || room.size().x <= 0.0 {
            continue;
        }
        let room = room.content_box().width();
        width = Some(room * scale);
        let by = (content.size().x - room).max(0.0) * scale;
        if let Some(most) = wide.get_mut(slot) {
            *most = Some(most.map_or(by, |most| most.max(by)));
        }
    }
    let wide = wide
        .into_iter()
        .zip(slots)
        .filter_map(|(by, placed)| Some((placed.index?, by?)))
        .collect();
    (wide, width)
}

/// Places the list's sideways offset: within how far the rows laid
/// out so far overflow, moved the least for a reveal on its way.
/// Moves every sideways cell's content by it and sizes it to fit.
/// Returns the width the spacer takes, so the bar spans the overflow.
pub(super) fn place_x(
    list: Entity,
    state: &mut VirtualList,
    position: &mut ScrollPosition,
    across: f32,
    wide: Vec<(usize, f32)>,
    cells: &mut Cells,
    nodes: &mut Query<(&ComputedNode, &mut Node)>,
) -> Val {
    let mut width = percent(100.0);
    let mut x = 0.0;
    let revealing = state.reveal_x.take();
    let room = state.cell_width.unwrap_or(across);
    let widest = state.widths.as_mut().map(|widths| {
        for (index, by) in wide {
            widths.set(index, by);
        }
        widths.widest
    });
    if let Some(widest) = widest {
        x = position.x.clamp(0.0, widest);
        if let Some(revealing) = revealing
            && revealing
                .placed
                .is_none_or(|placed| placed == position.x)
        {
            let span =
                (f64::from(revealing.x.0), f64::from(revealing.x.1));
            x = (reveal(f64::from(x), span, f64::from(room)) as f32)
                .clamp(0.0, widest);
            if revealing.placed != Some(x) {
                state.reveal_x = Some(RevealX {
                    placed: Some(x),
                    ..revealing
                });
            }
        }
        width = px(across + widest);
    }
    if position.x != x {
        position.x = x;
    }
    let shift = Val2::px(-x, 0.0);
    let sideways = state.widths.is_some();
    for (moved, cell, _, _, mut transform) in cells {
        if cell.0.list != list {
            continue;
        }
        if transform.translation != shift {
            transform.translation = shift;
        }
        if let Ok((_, mut node)) = nodes.get_mut(moved) {
            fit(&mut node, sideways);
        }
    }
    width
}
