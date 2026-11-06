//! A list that builds only the rows near its viewport.

mod heights;
mod sideways;

use core::ops::{Range, RangeInclusive};
use std::sync::Arc;

use bevy::app::{App, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryState;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, Query};
use bevy::ecs::world::World;
use bevy::math::{BVec2, Vec2};
use bevy::ui::{
    ComputedNode, Display, FlexDirection, IgnoreScroll, Node,
    Overflow, OverflowAxis, OverflowClipMargin, PositionType,
    ScrollPosition, Val, percent, px,
};
use heights::Heights;
use sideways::{Cells, RevealX, Widths};

use crate::prop::{Prop, Signal, each};
use crate::scroll::{self, ScrollGoal};
use crate::tokens::SpacingTokens;
use crate::views::frame::{Frame, FrameProps};
use crate::{AnyView, Bevy, Cx, FynixSystems, Styled, View};

/// How far above and below the view rows are built, in logical px,
/// unless [`VirtualListView::overscan`] says otherwise.
const OVERSCAN: f32 = 1000.0;

/// How much of a row, in logical px, has to show for it to count as
/// showing, so that a row only rounding shows does not.
const SHOWN: f64 = 0.5;

/// The most row slots a list builds: a budget of nodes laid out and
/// drawn every frame, which rows a few pixels high would otherwise
/// run through with thousands of slots in the overscan. 199 holds a
/// 500 px view and its default overscan down to rows of 13 px, and
/// shorter rows lose overscan first.
const MAX_SLOTS: usize = 199;

/// How far the median measured height may stray from the estimate
/// before it replaces it.
const ESTIMATE_TOLERANCE: RangeInclusive<f32> = 0.8..=1.25;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (read_sources, place_rows)
            .chain()
            .in_set(FynixSystems::Prepare)
            .after(scroll::ease),
    );
}

/// A column of `count` rows, of which it builds only the ones near
/// what shows. See [`virtual_list`].
pub struct VirtualListView<R> {
    pub frame: Frame,
    count: Prop<usize>,
    estimate: f32,
    sideways: Prop<bool>,
    follow: Prop<bool>,
    overscan: f32,
    row: R,
}

/// A scroll area of `count` rows, each `estimate` pixels high until
/// it has been laid out, built by `row`. Once the median height of
/// the rows laid out in a frame is below 0.8 or above 1.25 times
/// `estimate`, the median takes its place, so a few rows much taller
/// than the rest leave it as it is.
///
/// Unless the view [follows the
/// bottom](VirtualListView::follow_bottom), rows added keep its first
/// visible row in place, so a list that grows from no rows shows its
/// first.
///
/// A row that scrolls far out of sight is not despawned but shows
/// another row instead, so `row` builds the view of a [`RowSlot`]
/// and binds what it shows to [`RowSlot::index`]. Anything else the
/// row holds, such as hover or focus, stays with the slot.
///
/// A row is clipped to the height it is known to have, so on the
/// frame a row first shows, before it is laid out, it shows only as
/// much of itself as its estimate. Each row's top and bottom sit on
/// whole physical pixels, so rows meet without overlapping at any
/// scale factor.
///
/// The area takes the free space along its parent's direction and
/// can shrink below its rows. Its basis is 0, so in a column a
/// `height` does not size it: a parent of that height does. A row
/// takes the area's width. Give the area no padding or border.
///
/// # Limits
///
/// - It builds at most 199 rows, cutting the
///   [overscan](VirtualListView::overscan) on each side first. A view
///   more than 199 rows high shows a blank band below them.
/// - Positions are `f32` px, as in Bevy's layout. Past 2^23 px, about
///   400,000 rows of 20 px, a [`scroll_to`](VirtualList::scroll_to)
///   of the last row can land a pixel short, and past 2^24 px rows
///   can show seams of a few pixels.
pub fn virtual_list<R>(
    count: impl Into<Prop<usize>>,
    estimate: f32,
    row: R,
) -> VirtualListView<R> {
    assert!(
        estimate.is_finite() && estimate > 0.0,
        "a row estimate of {estimate} px"
    );
    VirtualListView {
        frame: Frame::unset()
            .direction(FlexDirection::Column)
            .overflow(Overflow {
                x: OverflowAxis::Clip,
                y: OverflowAxis::Scroll,
            })
            .clip_margin(OverflowClipMargin::content_box())
            .min_width(px(0.0))
            .min_height(px(0.0))
            .grow(1.0),
        count: count.into(),
        estimate,
        sideways: Prop::Value(false),
        follow: Prop::Value(false),
        overscan: OVERSCAN,
        row,
    }
}

impl<R> VirtualListView<R> {
    /// Keeps the view at the bottom while `on` holds and the view is
    /// at the bottom, as rows are added or grow, including while
    /// every row fits the view. A move that leaves the view above the
    /// bottom stops it: a scroll up by the user, or a
    /// [`scroll_to`](VirtualList::scroll_to),
    /// [`scroll_into_view`](VirtualList::scroll_into_view) or
    /// [`scroll_range_into_view`](VirtualList::scroll_range_into_view)
    /// that lands above the bottom. Rows added or measured never stop
    /// it. Any frame that leaves the view at the bottom starts it
    /// again: a move to the bottom, a `scroll_to` that lands there,
    /// rows removed from under the view, or `on` set while there.
    pub fn follow_bottom(
        mut self,
        on: impl Into<Prop<bool>>,
    ) -> Self {
        self.follow = on.into();
        self
    }

    /// Builds the rows within `px` logical pixels above and below the
    /// view too, 1,000 when unset, so that a row is laid out before
    /// it shows unless one frame scrolls further than that. Each
    /// frame lays out and draws every row built, so fewer cost less.
    pub fn overscan(mut self, px: f32) -> Self {
        assert!(
            px.is_finite() && px >= 0.0,
            "an overscan of {px} px"
        );
        self.overscan = px;
        self
    }
}

impl<R> FrameProps for VirtualListView<R> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T, R> View<Bevy, T> for VirtualListView<R>
where
    T: SpacingTokens + Send + Sync + 'static,
    R: Fn(RowSlot) -> AnyView<Bevy, T> + Send + Sync + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let list = cx.build(self.frame);
        let spacer = cx.under(list, Cx::spawn);
        let count = self.count.get(cx.world).unwrap_or(0);
        let follows = self.follow.get(cx.world).unwrap_or(false);
        let state = VirtualList::new(
            self.estimate,
            self.overscan,
            count,
            follows,
            spacer,
        );
        let height = state.height();
        let mut area = cx.world.entity_mut(list);
        area.insert((
            ScrollGoal::default(),
            state,
            Sources {
                count: self.count,
                follow: self.follow,
            },
        ));
        area.get_mut::<Node>().expect("a frame node").flex_basis =
            px(0.0);
        cx.effect(list, self.sideways, |world, list, &on| {
            let mut area = world.entity_mut(list);
            if let Some(mut state) = area.get_mut::<VirtualList>()
                && state.widths.is_some() != on
            {
                state.widths = on.then(Widths::default);
                state.sideways_switched = true;
            }
            if let Some(mut node) = area.get_mut::<Node>() {
                node.overflow.x = if on {
                    OverflowAxis::Scroll
                } else {
                    OverflowAxis::Clip
                };
            }
        });
        *cx.world.get_mut::<Node>(spacer).expect("a node") = Node {
            position_type: PositionType::Absolute,
            top: px(0.0),
            left: px(0.0),
            width: percent(100.0),
            height,
            ..Node::default()
        };
        let row = Arc::new(self.row);
        let slots = watch_list(list, |state| {
            state.map_or(0, |state| state.slots.len())
        })
        .map(|len| (0..len).collect::<Vec<_>>());
        cx.under(list, |cx| {
            cx.build(
                each(
                    slots,
                    |&slot| slot,
                    move |&slot| slot_view(list, slot, row.clone()),
                )
                .within(track()),
            )
        });
        list
    }
}

/// The slot of a [`virtual_list`] that one row view is built in.
/// Which row it shows changes as the list scrolls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowSlot {
    list: Entity,
    slot: usize,
}

impl RowSlot {
    /// The index of the row the slot shows. It is re-read only when
    /// the slot moves to another row, so a slot that keeps its row
    /// reads and writes nothing. It also changes when the slot is
    /// given another row at the same index, as a splice can do.
    pub fn index(self) -> Signal<usize> {
        watch_list(self.list, move |state| {
            let placed = state?.slots.get(self.slot)?;
            Some((placed.index?, placed.filled_at))
        })
        .map(|shows| shows.map_or(0, |(index, _)| index))
    }
}

/// Where a row sits in its list's content, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSpan {
    /// From the top of the first row.
    pub top: f32,
    /// The row's height, or its estimate until it is laid out.
    pub height: f32,
}

/// A virtual list's rows: their heights, and which slot shows which.
/// It sits on the list's scroll area.
#[derive(Component)]
pub struct VirtualList {
    estimate: f32,
    overscan: f64,
    count: usize,
    heights: Heights,
    spacer: Entity,
    slots: Vec<Placed>,
    /// How many fills there have been, stamped on each slot filled.
    fills: u64,
    spawned: Range<usize>,
    first_visible: usize,
    anchor: Anchor,
    follow: bool,
    /// Where the view was left when the rows were last placed, so a
    /// scroll since can be told from the list's own moves.
    placed_at: f32,
    /// How far splices above the view moved its rows since they were
    /// last placed, which the next placing scrolls by to keep them
    /// still.
    spliced_above: f64,
    /// The area's width when its rows were measured.
    across: f32,
    physical_per_logical: f64,
    /// How far each row overflows sideways, while the list scrolls
    /// sideways.
    widths: Option<Widths>,
    /// The width of the rows' sideways cells, as last laid out.
    cell_width: Option<f32>,
    reveal_x: Option<RevealX>,
    /// Whether sideways scrolling was turned on or off since the
    /// rows were last placed.
    sideways_switched: bool,
}

/// The props the list follows: read each frame into [`VirtualList`].
#[derive(Component)]
struct Sources {
    count: Prop<usize>,
    follow: Prop<bool>,
}

/// What the view holds on to as the rows are placed each frame.
#[derive(Clone, Copy)]
enum Anchor {
    /// The first visible row keeps its place on screen, as rows above
    /// it are measured, added or removed.
    FirstRow,
    /// The view stays at the end of the rows, as the list follows the
    /// bottom.
    End,
    /// A scroll is on its way to `target`, placed again each frame
    /// until it lands. `placed` is where the frame before left the
    /// view, `None` on the frame the scroll was asked for.
    Landing { target: Target, placed: Option<f64> },
}

/// Where a scroll on its way is headed.
#[derive(Clone, Copy)]
enum Target {
    /// The row to show first.
    First(usize),
    /// The span `from_top` of the row `index`, in px from its top, to
    /// show by moving the least from `from`.
    Reveal {
        index: usize,
        from_top: (f32, f32),
        from: Option<RevealFrom>,
    },
}

/// Where a reveal moves the least from, fixed on the frame it was
/// asked for so rows measured on the way do not move it: the row the
/// view was at and how far into it, or the end while the list was
/// following the bottom.
#[derive(Clone, Copy)]
enum RevealFrom {
    Row(usize, f64),
    End,
}

/// A row slot: its node once built, the row it shows and the fill
/// that gave it that row, and where it sits in the content.
#[derive(Default)]
struct Placed {
    node: Option<Entity>,
    /// `None` from a splice that removed its row until the next fill
    /// gives it another, later in the same update.
    index: Option<usize>,
    /// A slot given a row again at the index it had, as a splice can
    /// do, reads as moved by its stamp.
    filled_at: u64,
    top: f32,
    height: f32,
}

/// Where a frame leaves the view: jumped there, as a scroll on its
/// way lands, or shifted with any wheel ease in flight. `dropped` are
/// the nodes of the slots the frame let go of.
struct Scroll {
    to: f32,
    jump: bool,
    dropped: Vec<Entity>,
}

impl VirtualList {
    fn new(
        estimate: f32,
        overscan: f32,
        count: usize,
        follow: bool,
        spacer: Entity,
    ) -> Self {
        let mut list = Self {
            estimate,
            overscan: f64::from(overscan),
            count,
            heights: Heights::default(),
            spacer,
            slots: Vec::new(),
            fills: 0,
            spawned: 0..0,
            first_visible: 0,
            anchor: Anchor::FirstRow,
            follow,
            placed_at: 0.0,
            spliced_above: 0.0,
            across: 0.0,
            physical_per_logical: 1.0,
            widths: None,
            cell_width: None,
            reveal_x: None,
            sideways_switched: false,
        };
        list.place(&mut [], 0.0, 0.0, 0.0);
        list
    }

    /// Scrolls so that the row `index` is the first to show, from the
    /// next update, or the view ends at the list's end where the list
    /// cannot scroll that far. Each frame places it again, against
    /// that frame's rows and heights, until it lands: on a frame
    /// after the first that shows only measured rows or leaves the
    /// view where the frame before did. Rows added until then are
    /// rows it lands among, not growth to follow. A scroll of the
    /// view before then stops it.
    pub fn scroll_to(&mut self, index: usize) {
        self.land(Target::First(index));
    }

    /// Scrolls as little as shows all of the row `index`, at the next
    /// update. A row higher than the view scrolls as little as fills
    /// the view with it, so a view already inside it does not move.
    pub fn scroll_into_view(&mut self, index: usize) {
        self.scroll_range_into_view(index, 0.0..f32::INFINITY);
    }

    /// Scrolls as little as shows `range` of the row `index`, in
    /// pixels from the row's top, at the next update, the same way
    /// as [`scroll_into_view`](Self::scroll_into_view) shows a whole
    /// row. `range` is cut to the row's height. Both land the way
    /// [`scroll_to`](Self::scroll_to) does.
    pub fn scroll_range_into_view(
        &mut self,
        index: usize,
        range: Range<f32>,
    ) {
        self.land(Target::Reveal {
            index,
            from_top: (range.start, range.end),
            from: None,
        });
    }

    fn land(&mut self, mut target: Target) {
        if let (Anchor::End, Target::Reveal { from, .. }) =
            (self.anchor, &mut target)
        {
            *from = Some(RevealFrom::End);
        }
        self.anchor = Anchor::Landing {
            target,
            placed: None,
        };
    }

    /// Replaces the rows `at..at + removed` with `inserted` rows of
    /// unknown height, and changes [`count`](Self::count) to match.
    /// Every other row keeps its height and the view built for it,
    /// under its new index. Call it in `PreUpdate`, before the list
    /// places its rows in `Update`, in the frame the list's `count`
    /// changes to the same number. A `count` that changes without a
    /// splice adds or removes rows at the end.
    ///
    /// A splice wholly above the first visible row keeps the rows on
    /// screen where they are. One that removes the first visible row
    /// keeps the view's pixel. A [`scroll_to`](Self::scroll_to) or a
    /// scroll into view still on its way lands on the row it named,
    /// or on `at` if that row is removed.
    /// [`first_visible`](Self::first_visible) moves at once, and
    /// [`spawned`](Self::spawned) from the next update.
    ///
    /// # Panics
    ///
    /// If `at + removed` is past the list's count.
    pub fn splice(
        &mut self,
        at: usize,
        removed: usize,
        inserted: usize,
    ) {
        let end = at + removed;
        assert!(
            end <= self.count,
            "a splice of rows {at}..{end} in a list of {} rows",
            self.count
        );
        let moved = |index: usize| {
            if index >= end {
                (index - removed).saturating_add(inserted)
            } else {
                index.min(at)
            }
        };
        let span = |heights: &Heights, rows: usize| {
            heights.offset(at + rows) - heights.offset(at)
        };
        let above = self.count > 0 && end <= self.first_visible;
        let before = if above {
            span(&self.heights, removed)
        } else {
            0.0
        };
        self.heights.splice(at, removed, inserted, self.estimate);
        if let Some(widths) = &mut self.widths {
            widths.splice(at, removed, inserted);
        }
        self.count = self.count - removed + inserted;
        for placed in &mut self.slots {
            placed.index = placed
                .index
                .filter(|index| !(at..end).contains(index))
                .map(moved);
        }
        if let Anchor::Landing { target, .. } = &mut self.anchor {
            match target {
                Target::First(index) => *index = moved(*index),
                Target::Reveal { index, from, .. } => {
                    *index = moved(*index);
                    if let Some(RevealFrom::Row(row, into)) = from {
                        if (at..end).contains(row) {
                            *into = 0.0;
                        }
                        *row = moved(*row);
                    }
                }
            }
        }
        if above {
            self.spliced_above +=
                span(&self.heights, inserted) - before;
            self.first_visible = moved(self.first_visible);
        } else if at <= self.first_visible {
            self.first_visible = at.min(self.count.saturating_sub(1));
        }
    }

    /// The row at `y` in the list's content, or the nearest row when
    /// `y` is above the first or below the last. `None` while there
    /// are no rows. A point's content `y` is how far below the top of
    /// the area's content box it is, plus the area's
    /// [`ScrollPosition`].
    pub fn row_at(&self, y: f32) -> Option<usize> {
        (self.heights.len() > 0)
            .then(|| self.heights.index_at(f64::from(y.max(0.0))))
    }

    /// Where the row `index` sits in the list's content, `None` past
    /// the last row.
    pub fn row_span(&self, index: usize) -> Option<RowSpan> {
        self.heights.row(index).map(|(height, _)| RowSpan {
            top: self.heights.offset(index) as f32,
            height,
        })
    }

    /// The first row more than half a pixel of which shows.
    pub fn first_visible(&self) -> usize {
        self.first_visible
    }

    /// The rows built, `None` while there are none.
    pub fn spawned(&self) -> Option<RangeInclusive<usize>> {
        (!self.spawned.is_empty())
            .then(|| self.spawned.start..=self.spawned.end - 1)
    }

    /// How many rows the list has.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Whether the view follows the bottom as rows are added, as
    /// [`follow_bottom`](VirtualListView::follow_bottom) describes.
    pub fn following(&self) -> bool {
        self.follow && matches!(self.anchor, Anchor::End)
    }

    /// Rows out of sight cannot be measured again, so a change in
    /// the area's width leaves each with the height it had only as an
    /// estimate, until it shows or the median moves it.
    fn measured_across(&mut self, across: f32) {
        if self.across != across {
            self.across = across;
            self.heights.forget();
            if let Some(widths) = &mut self.widths {
                widths.forget();
            }
        }
    }

    fn height(&self) -> Val {
        px(self.heights.total() as f32)
    }

    fn shown_at(&self, y: f64) -> usize {
        self.heights.index_at(y + SHOWN)
    }

    /// Takes the heights `measured` this frame, finds where the view
    /// goes, and fills the slots around it. `view` is the area's
    /// height, `scroll` where it is, and `end` how far it can scroll.
    fn place(
        &mut self,
        mut measured: &mut [(usize, f32)],
        view: f32,
        scroll: f32,
        end: f32,
    ) -> Scroll {
        let shift = core::mem::take(&mut self.spliced_above);
        let scroll = scroll + shift as f32;
        self.placed_at += shift as f32;
        if let Anchor::Landing {
            placed: Some(placed),
            ..
        } = &mut self.anchor
        {
            *placed += shift;
        }
        let called = matches!(
            self.anchor,
            Anchor::Landing { placed: None, .. }
        );
        if scroll != self.placed_at && !called {
            self.anchor =
                if self.follow && view > 0.0 && scroll >= end {
                    Anchor::End
                } else {
                    Anchor::FirstRow
                };
        }
        let view = f64::from(view);
        let scroll = f64::from(scroll);
        let mut anchor = self.shown_at(scroll);
        let mut within = scroll - self.heights.offset(anchor);
        if let Anchor::Landing {
            target: Target::Reveal { from, .. },
            ..
        } = &mut self.anchor
        {
            from.get_or_insert(RevealFrom::Row(anchor, within));
        }
        if core::mem::take(&mut self.sideways_switched) {
            measured = &mut [];
            self.heights.forget();
            self.heights.reestimate(self.estimate);
        }
        for &(index, height) in &*measured {
            self.heights.set(index, height);
        }
        if let Some(typical) = lower_median(measured)
            && !ESTIMATE_TOLERANCE
                .contains(&(typical / self.estimate))
        {
            self.estimate = typical;
            self.heights.reestimate(typical);
        }
        if let Some(widths) = &mut self.widths {
            widths.resize(self.count);
        }
        if self.heights.len() != self.count {
            self.heights.resize(self.count, self.estimate);
            if anchor >= self.count {
                anchor = self.count.saturating_sub(1);
                within = 0.0;
            }
        }
        let bottom = (self.heights.total() - view).max(0.0);
        let at =
            (self.heights.offset(anchor) + within).clamp(0.0, bottom);
        let to = match self.anchor {
            Anchor::End if self.follow => bottom,
            Anchor::FirstRow | Anchor::End => at,
            Anchor::Landing { target, .. } => {
                self.landing(target, at, bottom, view)
            }
        }
        .clamp(0.0, bottom);
        let at_end = self.follow && to as f32 >= bottom as f32;
        let landing = match self.anchor {
            Anchor::Landing { target, placed } => {
                let landed = placed.is_some_and(|placed| {
                    placed == to || self.shows_measured(to, view)
                });
                (!landed).then_some(Anchor::Landing {
                    target,
                    placed: Some(to),
                })
            }
            _ => None,
        };
        let jump = matches!(self.anchor, Anchor::Landing { .. });
        self.anchor = landing.unwrap_or(if at_end {
            Anchor::End
        } else {
            Anchor::FirstRow
        });
        let dropped = self.fill(to, view);
        Scroll {
            to: to as f32,
            jump,
            dropped,
        }
    }

    fn shows_measured(&self, scroll: f64, view: f64) -> bool {
        let first = self.shown_at(scroll);
        let last = self.heights.index_at(scroll + view - SHOWN);
        (first..=last.max(first)).all(|index| {
            self.heights.row(index).is_none_or(|(_, known)| known)
        })
    }

    /// Where `target` puts the view this frame, given where it is
    /// (`at`) and how far it can scroll (`bottom`).
    fn landing(
        &self,
        target: Target,
        at: f64,
        bottom: f64,
        view: f64,
    ) -> f64 {
        let last = self.count.saturating_sub(1);
        match target {
            Target::First(index) => {
                self.heights.offset(index.min(last))
            }
            Target::Reveal { .. } if self.count == 0 => 0.0,
            Target::Reveal {
                index,
                from_top,
                from,
            } => {
                let index = index.min(last);
                let at = match from {
                    None => at,
                    Some(RevealFrom::End) => bottom,
                    Some(RevealFrom::Row(row, into)) => {
                        (self.heights.offset(row.min(self.count))
                            + into)
                            .clamp(0.0, bottom)
                    }
                };
                let top = self.heights.offset(index);
                let height = self
                    .heights
                    .row(index)
                    .map_or(0.0, |(height, _)| f64::from(height));
                let from = f64::from(from_top.0).clamp(0.0, height);
                let to = f64::from(from_top.1).clamp(from, height);
                reveal(at, (top + from, top + to), view)
            }
        }
    }

    /// Gives the slots the rows within the overscan of the view at
    /// `scroll`, keeping each slot that already shows one of them,
    /// and puts each at its row's top and height, on whole physical
    /// pixels. Returns the nodes of the slots it lets go of.
    fn fill(&mut self, scroll: f64, view: f64) -> Vec<Entity> {
        let count = self.heights.len();
        let top = (scroll - self.overscan).max(0.0);
        let mut first = self.heights.index_at(top);
        let mut end =
            (self.heights.index_at(top + view + 2.0 * self.overscan)
                + 1)
            .min(count);
        if end.saturating_sub(first) > MAX_SLOTS {
            let shown = self.heights.index_at(scroll);
            let below = self.heights.index_at(scroll + view) + 1;
            let spare = MAX_SLOTS.saturating_sub(below - shown);
            first = first.max(shown.saturating_sub(spare / 2));
            end = end.min(first + MAX_SLOTS);
            first = first.max(end.saturating_sub(MAX_SLOTS));
        }
        let need = end.saturating_sub(first);
        let pool = if self.slots.len() > 2 * need {
            need
        } else {
            self.slots.len().max(need)
        }
        .min(count)
        .min(MAX_SLOTS);
        let start = first.min(count - pool);
        let dropped = self
            .slots
            .drain(pool.min(self.slots.len())..)
            .filter_map(|placed| placed.node)
            .collect();
        let mut shown = vec![false; pool];
        let mut slot_of = vec![0; pool];
        let mut free = Vec::new();
        for (slot, placed) in self.slots.iter().enumerate() {
            match placed
                .index
                .and_then(|index| index.checked_sub(start))
            {
                Some(at) if at < pool && !shown[at] => {
                    shown[at] = true;
                    slot_of[at] = slot;
                }
                _ => free.push(slot),
            }
        }
        free.extend(self.slots.len()..pool);
        self.slots.resize_with(pool, Placed::default);
        let missing = (0..pool).filter(|&at| !shown[at]);
        self.fills += 1;
        for (slot, at) in free.into_iter().zip(missing) {
            self.slots[slot].index = Some(start + at);
            self.slots[slot].filled_at = self.fills;
            slot_of[at] = slot;
        }
        self.spawned = start..start + pool;
        let whole = |y: f64| {
            (y * self.physical_per_logical).round()
                / self.physical_per_logical
        };
        let tops = self
            .heights
            .offsets(start)
            .take(pool + 1)
            .map(whole)
            .collect::<Vec<_>>();
        for (at, &slot) in slot_of.iter().enumerate() {
            self.slots[slot].top = tops[at] as f32;
            self.slots[slot].height =
                (tops[at + 1] - tops[at]) as f32;
        }
        self.first_visible = self.shown_at(scroll);
        dropped
    }
}

fn read_sources(
    world: &mut World,
    lists: &mut QueryState<(Entity, &Sources)>,
    mut read: Local<Vec<(Entity, usize, bool)>>,
) {
    read.clear();
    read.extend(lists.iter(world).map(|(list, sources)| {
        (
            list,
            sources.count.get(world).unwrap_or(0),
            sources.follow.get(world).unwrap_or(false),
        )
    }));
    for &(list, count, follow) in read.iter() {
        if let Some(mut state) = world.get_mut::<VirtualList>(list)
            && (state.count != count || state.follow != follow)
        {
            state.count = count;
            state.follow = follow;
        }
    }
}

fn place_rows(
    mut lists: Query<(
        Entity,
        &mut VirtualList,
        &ComputedNode,
        &mut ScrollPosition,
        &mut ScrollGoal,
    )>,
    mut nodes: Query<(&ComputedNode, &mut Node)>,
    mut cells: Cells,
    mut commands: Commands,
) {
    for (entity, mut list, area, mut position, mut goal) in &mut lists
    {
        let scale = area.inverse_scale_factor();
        if scale > 0.0 {
            list.physical_per_logical = f64::from(scale.recip());
        }
        let view = area.content_box().height() * scale;
        let (wide, room) = if list.widths.is_some() {
            sideways::overflows(
                entity,
                &list.slots,
                &cells,
                &nodes,
                scale,
            )
        } else {
            (Vec::new(), None)
        };
        list.cell_width = room.or(list.cell_width);
        let mut measured = list
            .slots
            .iter()
            .filter_map(|placed| {
                let (node, _) = nodes.get(placed.node?).ok()?;
                let height = node.content_size().y
                    * node.inverse_scale_factor();
                let index = placed.index?;
                (height > 0.0).then_some((index, height))
            })
            .collect::<Vec<_>>();
        let across = area.content_box().width() * scale;
        list.measured_across(across);
        let end = scroll::overflow(area).y;
        let scroll = list.place(&mut measured, view, position.y, end);
        // A slot let go of is despawned here and not left to `each`,
        // which runs after the mounted effects: a row must not run
        // against the row its slot no longer has.
        for node in scroll.dropped {
            commands.entity(node).try_despawn();
        }
        if scroll.jump {
            *goal = ScrollGoal::default();
            position.y = scroll.to;
        } else if scroll.to != position.y {
            let delta = Vec2::Y * (scroll.to - position.y);
            goal.shift(&mut position, delta);
        }
        list.placed_at = position.y;
        for placed in &list.slots {
            let (top, height) = (px(placed.top), px(placed.height));
            if let Some(slot) = placed.node
                && let Ok((_, mut slot)) = nodes.get_mut(slot)
                && (slot.top != top || slot.height != height)
            {
                slot.top = top;
                slot.height = height;
            }
        }
        let width = sideways::place_x(
            entity,
            &mut list,
            &mut position,
            across,
            wide,
            &mut cells,
            &mut nodes,
        );
        let height = list.height();
        if let Ok((_, mut spacer)) = nodes.get_mut(list.spacer)
            && (spacer.height != height || spacer.width != width)
        {
            spacer.height = height;
            spacer.width = width;
        }
    }
}

/// Where a view with `room` to show at `at` moves the least to show
/// `span`: nowhere while the span is in view, and the least way into
/// a span longer than the room.
fn reveal(at: f64, span: (f64, f64), room: f64) -> f64 {
    let (lo, hi) = (span.0, span.1 - room);
    at.clamp(lo.min(hi), lo.max(hi))
}

/// The node the slots are built under: as wide as the area and
/// scrolled with it, but left where it is by a sideways scroll, which
/// moves only the rows' sideways cells.
fn track<T: 'static>() -> AnyView<Bevy, T> {
    AnyView::new(|cx: &mut Cx<'_, Bevy, T>| {
        let node = cx.spawn();
        *cx.world.get_mut::<Node>(node).expect("a node") = Node {
            position_type: PositionType::Absolute,
            top: px(0.0),
            left: px(0.0),
            right: px(0.0),
            ..Node::default()
        };
        cx.world
            .entity_mut(node)
            .insert(IgnoreScroll(BVec2 { x: true, y: false }));
        node
    })
}

fn slot_view<T, R>(
    list: Entity,
    slot: usize,
    row: Arc<R>,
) -> AnyView<Bevy, T>
where
    T: 'static,
    R: Fn(RowSlot) -> AnyView<Bevy, T> + Send + Sync + 'static,
{
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let node = cx.spawn();
        let mut state =
            cx.world.get_mut::<VirtualList>(list).expect("the list");
        let placed = &mut state.slots[slot];
        placed.node = Some(node);
        let (top, height) = (placed.top, placed.height);
        // In a block its row keeps its height whatever the slot's,
        // and that is what the slot measures.
        *cx.world.get_mut::<Node>(node).expect("a node") = Node {
            display: Display::Block,
            position_type: PositionType::Absolute,
            top: px(top),
            left: px(0.0),
            right: px(0.0),
            height: px(height),
            overflow: Overflow::clip(),
            ..Node::default()
        };
        cx.under(node, |cx| cx.build(row(RowSlot { list, slot })));
        node
    })
}

/// A signal of `read` over the list on `list`, re-read when what
/// `read` returns changes.
fn watch_list<T>(
    list: Entity,
    read: impl Fn(Option<&VirtualList>) -> T + Send + Sync + 'static,
) -> Signal<T>
where
    T: PartialEq + Send + Sync + 'static,
{
    let read = Arc::new(move |world: &World| {
        read(world.get::<VirtualList>(list))
    });
    let check = read.clone();
    let mut seen = None;
    Signal::new(
        move |world| read(world),
        move |world| {
            let now = check(world);
            let changed = seen.as_ref() != Some(&now);
            seen = Some(now);
            changed
        },
    )
}

fn lower_median(heights: &mut [(usize, f32)]) -> Option<f32> {
    let middle = heights.len().checked_sub(1)? / 2;
    let (_, &mut (_, height), _) = heights
        .select_nth_unstable_by(middle, |a, b| a.1.total_cmp(&b.1));
    Some(height)
}
