//! The virtual list against real bevy_ui layout, headless: which rows
//! it builds, where they show, and what a change keeps still.

use core::ops::Range;
use core::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;

use bevy::app::{App, Last};
use bevy::ecs::change_detection::Mut;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Changed, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::ecs::world::World;
use bevy::math::Vec2;
use bevy::ui::{
    ComputedNode, Node, ScrollPosition, UiGlobalTransform, Val, px,
};

use super::layout::{height, laid_out, laid_out_at, settle, top_of};
use crate::tests::{Plain, wheel};
use crate::views::{
    FrameProps, RowSlot, RowSpan, VirtualList, column, frame,
    row as hrow, virtual_list,
};
use crate::{
    AnyView, Bevy, Prop, ViewExt, derived, every_frame, mount,
    resource,
};

const ESTIMATE: f32 = 20.0;
const VIEW: f32 = 500.0;
const OVERSCAN: f32 = 1000.0;
/// Frames enough for any scroll to land and the rows to be measured.
const SETTLED: usize = 6;

#[derive(Resource)]
struct Count(usize);

#[derive(Resource)]
struct Across(f32);

#[derive(Resource)]
struct Grown(usize, f32);

#[derive(Resource)]
struct Follow(bool);

/// The row a slot's view shows, written by the view.
#[derive(Component)]
struct Shows(usize);

type Height = fn(&World, usize) -> f32;

fn uniform(_: &World, _: usize) -> f32 {
    20.0
}

/// Rows whose edges fall off the pixel grid at fractional scales.
fn off_grid(_: &World, _: usize) -> f32 {
    19.0
}

fn tall_hundredths(_: &World, index: usize) -> f32 {
    if index.is_multiple_of(100) {
        60.0
    } else {
        20.0
    }
}

fn regions(_: &World, index: usize) -> f32 {
    match index {
        ..3_000 => 20.0,
        3_000..7_000 => 10.0,
        _ => 200.0,
    }
}

fn mixed(_: &World, index: usize) -> f32 {
    let x = (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 59;
    12.0 + x as f32 * 28.0 / 31.0
}

fn short_tail(_: &World, index: usize) -> f32 {
    if index >= 9_900 { 5.0 } else { 40.0 }
}

fn tall_tail(_: &World, index: usize) -> f32 {
    if index >= 9_960 { 60.0 } else { 17.0 }
}

fn grown(world: &World, index: usize) -> f32 {
    match world.get_resource::<Grown>() {
        Some(&Grown(at, height)) if at == index => height,
        _ => 20.0,
    }
}

/// A row of the height `height` gives its index, re-read every
/// frame, that records which row it shows.
fn row(height: Height) -> impl Fn(RowSlot) -> AnyView<Bevy, Plain> {
    move |slot| {
        AnyView::new(move |cx| {
            let index = slot.index();
            let node = cx.build(
                frame().height(
                    derived(move |world| {
                        px(height(world, index.get(world)))
                    })
                    .when(every_frame()),
                ),
            );
            cx.effect(
                node,
                slot.index().into(),
                |world, node, &index| {
                    world.entity_mut(node).insert(Shows(index));
                },
            );
            node
        })
    }
}

fn list(app: &mut App, count: usize, height: Height) -> Entity {
    list_in(app, count, row(height), px(VIEW))
}

fn list_in(
    app: &mut App,
    count: usize,
    row: impl Fn(RowSlot) -> AnyView<Bevy, Plain> + Send + Sync + 'static,
    view: impl Into<Prop<Val>>,
) -> Entity {
    let area = mounted(app, count, ESTIMATE, OVERSCAN, row, view);
    settle(app, SETTLED);
    area
}

/// A list of `count` rows in a column 300 px wide and `view` high,
/// bound to [`Count`] and [`Follow`], before its first update.
fn mounted(
    app: &mut App,
    count: usize,
    estimate: f32,
    overscan: f32,
    row: impl Fn(RowSlot) -> AnyView<Bevy, Plain> + Send + Sync + 'static,
    view: impl Into<Prop<Val>>,
) -> Entity {
    app.insert_resource(Count(count));
    if !app.world().contains_resource::<Follow>() {
        app.insert_resource(Follow(false));
    }
    mount::<Plain>(
        app.world_mut(),
        column((virtual_list(
            resource::<Count, _>(|count| count.0),
            estimate,
            row,
        )
        .follow_bottom(resource::<Follow, _>(|follow| follow.0))
        .overscan(overscan),))
        .width(px(300.0))
        .height(view),
    );
    the_list(app)
}

fn the_list(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<VirtualList>>()
        .single(app.world())
        .expect("one list")
}

fn state(app: &App, area: Entity) -> &VirtualList {
    app.world().get::<VirtualList>(area).expect("a list")
}

fn list_mut(app: &mut App, area: Entity) -> Mut<'_, VirtualList> {
    app.world_mut()
        .get_mut::<VirtualList>(area)
        .expect("a list")
}

fn scroll_to(app: &mut App, area: Entity, index: usize) {
    list_mut(app, area).scroll_to(index);
}

fn reveal(app: &mut App, area: Entity, index: usize) {
    list_mut(app, area).scroll_into_view(index);
    settle(app, SETTLED);
}

fn reveal_range(
    app: &mut App,
    area: Entity,
    index: usize,
    range: Range<f32>,
) {
    list_mut(app, area).scroll_range_into_view(index, range);
    settle(app, SETTLED);
}

fn scroll(app: &App, area: Entity) -> f32 {
    app.world().get::<ScrollPosition>(area).expect("a scroll").y
}

/// How far the area scrolls each way.
fn overflow(app: &App, area: Entity) -> Vec2 {
    crate::scroll::overflow(
        app.world().get::<ComputedNode>(area).unwrap(),
    )
}

fn scroll_by(app: &mut App, area: Entity, by: f32) {
    app.world_mut().get_mut::<ScrollPosition>(area).unwrap().y += by;
}

fn count(app: &mut App, count: usize) {
    app.world_mut().resource_mut::<Count>().0 = count;
}

/// Every row view built, with the row it shows and its slot, by
/// index.
fn built(app: &mut App) -> Vec<(usize, Entity, Entity)> {
    let mut built = app
        .world_mut()
        .query::<(Entity, &Shows, &ChildOf)>()
        .iter(app.world())
        .map(|(node, shows, slot)| (shows.0, node, slot.parent()))
        .collect::<Vec<_>>();
    built.sort();
    built
}

/// The spans of the rows drawn, from the top of the area, as the
/// slots clip them, in order.
fn drawn(app: &mut App, area: Entity) -> Vec<(f32, f32)> {
    let mut spans = built(app)
        .into_iter()
        .map(|(_, row, slot)| {
            let start = top_of(app, area, row);
            let end = start + height(app, row);
            let from = top_of(app, area, slot);
            (start.max(from), end.min(from + height(app, slot)))
        })
        .filter(|(start, end)| end > start)
        .collect::<Vec<_>>();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    spans
}

fn assert_apart(app: &mut App, area: Entity, when: &str) {
    let spans = drawn(app, area);
    for pair in spans.windows(2) {
        assert!(
            pair[1].0 >= pair[0].1 - 0.01,
            "{when}: a row drawn at {:?} and one at {:?}",
            pair[0],
            pair[1]
        );
    }
}

/// The widest gap in the view no row is drawn over.
fn uncovered(app: &mut App, area: Entity) -> f32 {
    let mut at = 0.0f32;
    let mut worst = 0.0f32;
    for (start, end) in drawn(app, area) {
        if end <= at {
            continue;
        }
        worst = worst.max(start.min(VIEW) - at);
        at = end;
        if at >= VIEW {
            return worst;
        }
    }
    worst.max(VIEW - at)
}

/// The rows built, each with its node and its top from the top of
/// the area, by index.
fn rows(app: &mut App, area: Entity) -> Vec<(usize, Entity, f32)> {
    built(app)
        .into_iter()
        .map(|(index, node, _)| {
            (index, node, top_of(app, area, node))
        })
        .collect()
}

fn shown_at(
    app: &mut App,
    area: Entity,
    index: usize,
) -> Option<f32> {
    rows(app, area)
        .into_iter()
        .find(|&(shown, ..)| shown == index)
        .map(|(.., top)| top)
}

/// The rows built run unbroken from above `from` to below `to`.
fn assert_cover(app: &mut App, area: Entity, from: f32, to: f32) {
    let rows = rows(app, area);
    let indices = rows.iter().map(|row| row.0).collect::<Vec<_>>();
    let unbroken =
        indices.windows(2).all(|pair| pair[1] == pair[0] + 1);
    assert!(unbroken, "rows {indices:?}");
    let (_, first, start) = rows[0];
    let (_, last, end) = rows[rows.len() - 1];
    let end = end + height(app, last);
    assert!(
        start <= from,
        "the first row, {first}, starts at {start}"
    );
    assert!(end >= to, "the last row, {last}, ends at {end}");
}

fn near(got: Option<f32>, want: f32) -> bool {
    got.is_some_and(|got| (got - want).abs() <= 1.0)
}

/// Whether a `scroll_to(index)` has landed: the row at the top of
/// the view, or the last row at the bottom where the list cannot
/// scroll that far.
fn landed(
    app: &mut App,
    area: Entity,
    index: usize,
    when: &str,
    failed: &mut Vec<String>,
) {
    let list = state(app, area);
    let last = list.count() - 1;
    let target = list.row_span(index.min(last)).unwrap();
    let end = list.row_span(last).unwrap();
    let bottom = (end.top + end.height - VIEW).max(0.0);
    let at_bottom = target.top >= bottom - 0.5;
    let (shown, want) = if at_bottom {
        (last, (VIEW - end.height).min(end.top))
    } else {
        (index.min(last), 0.0)
    };
    let got = shown_at(app, area, shown);
    if !near(got, want) {
        failed.push(format!(
            "{when}: row {shown} at {got:?}, not {want}"
        ));
    }
}

/// A `scroll_to(index)` asked for now lands by the second frame and
/// stays landed.
fn lands(
    app: &mut App,
    area: Entity,
    index: usize,
    case: &str,
    failed: &mut Vec<String>,
) {
    scroll_to(app, area, index);
    settle(app, 2);
    landed(
        app,
        area,
        index,
        &format!("{case}, by the second frame"),
        failed,
    );
    settle(app, SETTLED);
    landed(app, area, index, &format!("{case}, settled"), failed);
}

#[test]
fn it_builds_only_the_rows_within_the_overscan() {
    for (overscan, index) in
        [(OVERSCAN, 0), (OVERSCAN, 5_000), (100.0, 5_000)]
    {
        let mut app = laid_out();
        let area = mounted(
            &mut app,
            10_000,
            ESTIMATE,
            overscan,
            row(tall_hundredths),
            px(VIEW),
        );
        scroll_to(&mut app, area, index);
        settle(&mut app, SETTLED);

        let when = format!("overscan {overscan}, row {index}");
        assert_eq!(
            state(&app, area).first_visible(),
            index,
            "{when}"
        );
        assert_eq!(
            shown_at(&mut app, area, index),
            Some(0.0),
            "{when}"
        );
        let above = if index == 0 { 0.0 } else { overscan };
        assert_cover(
            &mut app,
            area,
            -above,
            VIEW + 2.0 * overscan - above,
        );
        let most = ((VIEW + 2.0 * overscan) / ESTIMATE) as usize + 1;
        let built = rows(&mut app, area).len();
        assert!(built <= most, "{when}: {built} rows");
    }
}

#[test]
fn a_scroll_shows_new_rows_in_the_nodes_it_already_has() {
    let mut app = laid_out();
    let area = list(&mut app, 10_000, uniform);
    let nodes = |app: &mut App| {
        rows(app, area)
            .into_iter()
            .map(|(index, node, _)| (node, index))
            .collect::<HashMap<_, _>>()
    };
    let before = nodes(&mut app);
    let spawned = app.world().entities().count_spawned();

    scroll_by(&mut app, area, 1_500.0);
    settle(&mut app, SETTLED);

    let after = nodes(&mut app);
    let mut kept = after.keys().collect::<Vec<_>>();
    kept.sort();
    let mut had = before.keys().collect::<Vec<_>>();
    had.sort();
    assert_eq!(kept, had, "the same row nodes");
    assert!(after.iter().any(|(node, index)| before[node] != *index));
    assert_eq!(app.world().entities().count_spawned(), spawned);
    assert_eq!(shown_at(&mut app, area, 75), Some(0.0));
}

#[test]
fn a_scroll_reads_only_the_rows_whose_slot_moves() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let mut app = laid_out();
    let counted = |slot: RowSlot| {
        frame()
            .height(slot.index().map(|_| {
                READS.fetch_add(1, Ordering::Relaxed);
                px(20.0)
            }))
            .boxed()
    };
    let area = list_in(&mut app, 10_000, counted, px(VIEW));
    scroll_to(&mut app, area, 5_000);
    settle(&mut app, SETTLED);

    READS.store(0, Ordering::Relaxed);
    scroll_by(&mut app, area, 200.0);
    settle(&mut app, SETTLED);

    assert_eq!(
        READS.load(Ordering::Relaxed),
        10,
        "one read a moved row"
    );
}

#[test]
fn the_first_row_holds_still_when_a_row_above_grows() {
    let mut app = laid_out();
    let area = list(&mut app, 10_000, grown);
    scroll_to(&mut app, area, 5_000);
    settle(&mut app, SETTLED);
    let before = scroll(&app, area);

    app.insert_resource(Grown(4_990, 100.0));
    for frame in 0..4 {
        app.update();
        assert_eq!(
            state(&app, area).first_visible(),
            5_000,
            "frame {frame}"
        );
    }
    assert_eq!(shown_at(&mut app, area, 5_000), Some(0.0));
    assert_eq!(scroll(&app, area), before + 80.0);

    let (still, first_still) = wheeled_as_a_row_above_grows(false);
    let (moved, first_moved) = wheeled_as_a_row_above_grows(true);
    assert_eq!(
        first_moved, first_still,
        "an ease in flight keeps its row"
    );
    assert_eq!(
        moved,
        still + 80.0,
        "and lands as far on as the growth"
    );
}

/// Where a wheel flung three notches lands, and the row it leaves at
/// the top, when a row above grows by 80 px while it is easing, or
/// does not.
fn wheeled_as_a_row_above_grows(grows: bool) -> (f32, usize) {
    let mut app = laid_out();
    let area = list(&mut app, 10_000, grown);
    scroll_to(&mut app, area, 5_000);
    settle(&mut app, SETTLED);
    wheel(&mut app, area, -3.0);
    settle(&mut app, 2);
    if grows {
        app.insert_resource(Grown(4_990, 100.0));
    }
    settle(&mut app, 120);
    (scroll(&app, area), state(&app, area).first_visible())
}

#[test]
fn a_wrong_estimate_never_overlaps_rows_or_leaves_a_gap() {
    for (height, estimate) in [
        (uniform as Height, ESTIMATE / 10.0),
        (uniform, ESTIMATE * 10.0),
        (regions, ESTIMATE),
    ] {
        let mut app = laid_out();
        let area = mounted(
            &mut app,
            10_000,
            estimate,
            OVERSCAN,
            row(height),
            px(VIEW),
        );
        let check = |app: &mut App, when: &str| {
            let built = rows(app, area).len();
            assert!(built < 200, "{when}: {built} rows built");
            assert_apart(app, area, when);
        };
        for frame in 1..=30 {
            app.update();
            check(&mut app, &format!("first paint, frame {frame}"));
        }
        type Jump = fn(&mut App, Entity);
        let jumps: [(&str, Jump); 2] = [
            ("scroll_to", |app, area| scroll_to(app, area, 5_000)),
            ("a jump", |app, area| scroll_by(app, area, 20_000.0)),
        ];
        for (name, jump) in jumps {
            jump(&mut app, area);
            for frame in 1..=8 {
                app.update();
                let when = format!(
                    "{} px rows under {estimate}, {name}, frame \
                     {frame}",
                    height(app.world(), 0)
                );
                check(&mut app, &when);
                if frame >= 2 {
                    let gap = uncovered(&mut app, area);
                    assert_eq!(
                        gap, 0.0,
                        "{when}: {gap} px uncovered"
                    );
                }
            }
        }
    }
}

#[test]
fn one_huge_row_barely_moves_the_estimate() {
    let mut app = laid_out();
    let area = list(&mut app, 10_000, |_, index| {
        if index == 3 { 2_000.0 } else { 20.0 }
    });

    let last = state(&app, area).row_span(9_999).unwrap();
    let real = 9_999.0 * 20.0 + 1_980.0;
    assert!(
        (last.top - real).abs() / real < 0.03,
        "row 9999 at {} of {real}",
        last.top
    );
}

#[test]
fn a_width_change_reestimates_rows_out_of_sight() {
    let mut app = laid_out();
    app.insert_resource(Across(300.0));
    app.insert_resource(Count(10_000));
    let wraps = |world: &World, _: usize| {
        if world.resource::<Across>().0 < 300.0 {
            40.0
        } else {
            20.0
        }
    };
    mount::<Plain>(
        app.world_mut(),
        column((virtual_list(
            resource::<Count, _>(|count| count.0),
            ESTIMATE,
            row(wraps),
        ),))
        .width(resource::<Across, _>(|across| px(across.0)))
        .height(px(VIEW)),
    );
    let area = the_list(&mut app);
    scroll_to(&mut app, area, 5_000);
    settle(&mut app, SETTLED);

    app.insert_resource(Across(200.0));
    settle(&mut app, SETTLED);

    assert_eq!(
        state(&app, area).row_span(50),
        Some(RowSpan {
            top: 2_000.0,
            height: 40.0
        })
    );
    assert_eq!(shown_at(&mut app, area, 5_000), Some(0.0));
}

/// Each built row's slot in physical px: its index, top from the top
/// of the area, and height.
fn physical(app: &mut App, area: Entity) -> Vec<(usize, f32, f32)> {
    built(app)
        .into_iter()
        .map(|(index, _, slot)| {
            let computed =
                app.world().get::<ComputedNode>(slot).unwrap();
            let scale = computed.inverse_scale_factor().recip();
            let top = top_of(app, area, slot) * scale;
            (index, top, height(app, slot) * scale)
        })
        .collect()
}

/// The first slot off a whole physical pixel, or seam between
/// neighbours.
fn off_pixels(spans: &[(usize, f32, f32)]) -> Option<String> {
    let whole = |y: f32| (y - y.round()).abs() <= 0.01;
    if let Some(&(index, top, height)) =
        spans.iter().find(|span| !whole(span.1) || !whole(span.2))
    {
        return Some(format!("row {index} at {top}, {height} high"));
    }
    spans
        .windows(2)
        .find(|pair| {
            pair[1].0 == pair[0].0 + 1
                && (pair[1].1 - (pair[0].1 + pair[0].2)).abs() > 0.01
        })
        .map(|pair| format!("a seam under row {}", pair[0].0))
}

#[test]
fn rows_sit_on_whole_physical_pixels_at_every_scale() {
    let mut failed = Vec::new();
    for scale in [1.25, 1.5] {
        let mut app = laid_out_at(scale);
        let area = mounted(
            &mut app,
            10_000,
            16.3,
            OVERSCAN,
            row(regions),
            px(VIEW),
        );
        let mut check = |app: &mut App, when: String| {
            if let Some(off) = off_pixels(&physical(app, area)) {
                failed.push(format!("{scale}x, {when}: {off}"));
            }
        };
        for frame in 1..=3 {
            app.update();
            check(&mut app, format!("first paint, frame {frame}"));
        }
        for (step, index) in
            [5_000, 9_999, 7_100].into_iter().enumerate()
        {
            scroll_to(&mut app, area, index);
            for frame in 1..=3 {
                app.update();
                check(
                    &mut app,
                    format!("scroll_to({index}), frame {frame}"),
                );
            }
            for k in 0..2 {
                scroll_by(
                    &mut app,
                    area,
                    0.3 + step as f32 * 0.07 + k as f32 * 0.11,
                );
                app.update();
                check(&mut app, format!("scroll_to({index}), +{k}"));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn count_changes_keep_the_view_of_a_list_that_does_not_follow() {
    let mut app = laid_out();
    let area = list(&mut app, 10, uniform);

    count(&mut app, 10_000);
    settle(&mut app, SETTLED);
    assert_eq!(scroll(&app, area), 0.0, "grown from a few rows");
    assert_eq!(state(&app, area).first_visible(), 0);

    scroll_to(&mut app, area, 9_999);
    settle(&mut app, SETTLED);
    let first = state(&app, area).first_visible();
    scroll_to(&mut app, area, first);
    settle(&mut app, SETTLED);
    let at = scroll(&app, area);
    count(&mut app, 20_000);
    settle(&mut app, SETTLED);
    assert_eq!(scroll(&app, area), at, "grown at the end");
    assert_eq!(state(&app, area).first_visible(), first);

    count(&mut app, 10);
    settle(&mut app, SETTLED);
    let indices = rows(&mut app, area)
        .into_iter()
        .map(|row| row.0)
        .collect::<Vec<_>>();
    assert_eq!(indices, (0..10).collect::<Vec<_>>(), "shrunk");
    assert_eq!(scroll(&app, area), 0.0);
}

#[test]
fn scroll_to_lands_on_its_row_by_the_second_frame() {
    let mut failed = Vec::new();
    let cases: [(&str, Height, f32, f32, usize); 5] = [
        ("off the pixel grid", off_grid, 17.0, 1.25, 9_973),
        ("12 to 40 px", mixed, 26.0, 1.0, 5_000),
        ("a 5 px tail", short_tail, 40.0, 1.0, 9_940),
        ("a 60 px tail", tall_tail, 17.0, 1.0, 9_990),
        ("20 px, past the end", uniform, 20.0, 1.0, 10_050),
    ];
    for (name, height, estimate, scale, index) in cases {
        let mut app = laid_out_at(scale);
        let area = mounted(
            &mut app,
            10_000,
            estimate,
            OVERSCAN,
            row(height),
            px(VIEW),
        );
        settle(&mut app, SETTLED);
        let case = format!(
            "{name} under {estimate}, {scale}x, scroll_to({index})"
        );
        lands(&mut app, area, index, &case, &mut failed);
    }

    let mut app = laid_out();
    let area = list(&mut app, 1_000, off_grid);
    count(&mut app, 10_000);
    lands(&mut app, area, 9_973, "with the count", &mut failed);

    let mut app = laid_out();
    let area = list(&mut app, 1_000, uniform);
    scroll_to(&mut app, area, 5_000);
    for rows in (2_000..=7_000).step_by(1_000) {
        count(&mut app, rows);
        app.update();
    }
    settle(&mut app, SETTLED);
    landed(&mut app, area, 5_000, "among rows arriving", &mut failed);
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn a_scroll_on_the_frame_after_a_scroll_to_stands() {
    let mut app = laid_out();
    let area = list(&mut app, 10_000, uniform);
    scroll_to(&mut app, area, 5_000);
    app.update();
    let landed = scroll(&app, area);

    scroll_by(&mut app, area, 100.0);
    settle(&mut app, SETTLED);

    assert_eq!(scroll(&app, area), landed + 100.0);
}

#[test]
fn scroll_into_view_moves_the_least() {
    let mut app = laid_out();
    let area = list(&mut app, 10_000, tall_hundredths);

    reveal(&mut app, area, 40);
    assert_eq!(scroll(&app, area), 40.0 * 20.0 + 40.0 + 20.0 - VIEW);
    assert_eq!(shown_at(&mut app, area, 40), Some(VIEW - 20.0));

    reveal(&mut app, area, 300);
    assert_eq!(
        shown_at(&mut app, area, 300),
        Some(VIEW - 60.0),
        "an unmeasured row shows whole"
    );
}

#[test]
fn a_range_of_a_tall_row_scrolls_in_by_the_least() {
    let mut app = laid_out();
    app.insert_resource(Grown(50, 2_000.0));
    let area = list(&mut app, 10_000, grown);
    reveal(&mut app, area, 50);
    let top = state(&app, area).row_span(50).unwrap().top;
    assert_eq!(scroll(&app, area), top);

    reveal_range(&mut app, area, 50, 1_500.0..1_520.0);
    assert_eq!(
        scroll(&app, area),
        top + 1_520.0 - VIEW,
        "from above"
    );

    reveal_range(&mut app, area, 50, 600.0..620.0);
    assert_eq!(scroll(&app, area), top + 600.0, "from below");

    reveal_range(&mut app, area, 50, 700.0..720.0);
    assert_eq!(scroll(&app, area), top + 600.0, "in view");

    reveal_range(&mut app, area, 50, 100.0..900.0);
    assert_eq!(
        scroll(&app, area),
        top + 400.0,
        "taller than the view"
    );
}

#[test]
fn a_following_list_keeps_each_new_row_in_sight() {
    for height in [uniform as Height, tall_hundredths] {
        let mut app = laid_out();
        app.insert_resource(Follow(true));
        let area = list(&mut app, 0, height);
        for rows in (2..=600).step_by(2) {
            count(&mut app, rows);
            app.update();
            let top =
                shown_at(&mut app, area, rows.saturating_sub(3));
            let shown =
                top.is_some_and(|top| (0.0..VIEW).contains(&top));
            assert!(
                rows < 4 || shown,
                "{rows} rows: row {} at {top:?}",
                rows - 3
            );
        }
        settle(&mut app, SETTLED);
        let last = shown_at(&mut app, area, 599).unwrap();
        assert_eq!(last + height(app.world(), 599), VIEW);
    }
}

#[test]
fn following_stops_on_a_move_up_and_resumes_at_the_bottom() {
    let mut app = laid_out();
    app.insert_resource(Follow(true));
    let area = list(&mut app, 1_000, uniform);
    scroll_to(&mut app, area, 999);
    settle(&mut app, SETTLED);
    assert!(state(&app, area).is_following(), "at the bottom");

    scroll_by(&mut app, area, -0.25);
    settle(&mut app, SETTLED);
    let up = scroll(&app, area);
    assert!(!state(&app, area).is_following(), "a quarter pixel up");
    count(&mut app, 1_100);
    settle(&mut app, SETTLED);
    assert_eq!(scroll(&app, area), up, "growth after leaving");

    scroll_by(&mut app, area, 1.0e9);
    settle(&mut app, SETTLED);
    count(&mut app, 1_200);
    settle(&mut app, SETTLED);
    assert_eq!(shown_at(&mut app, area, 1_199), Some(VIEW - 20.0));

    app.insert_resource(Follow(false));
    settle(&mut app, SETTLED);
    app.insert_resource(Follow(true));
    settle(&mut app, SETTLED);
    assert!(
        state(&app, area).is_following(),
        "on again at its bottom"
    );
    count(&mut app, 1_300);
    settle(&mut app, SETTLED);
    assert_eq!(shown_at(&mut app, area, 1_299), Some(VIEW - 20.0));

    app.insert_resource(Follow(false));
    settle(&mut app, SETTLED);
    let at = scroll(&app, area);
    count(&mut app, 1_400);
    settle(&mut app, SETTLED);
    assert_eq!(scroll(&app, area), at, "follow switched off");
    assert!(!state(&app, area).is_following());

    scroll_by(&mut app, area, -100.0);
    app.insert_resource(Follow(true));
    settle(&mut app, SETTLED);
    let at = scroll(&app, area);
    count(&mut app, 1_500);
    settle(&mut app, SETTLED);
    assert_eq!(scroll(&app, area), at, "on again above its bottom");
    assert!(!state(&app, area).is_following());

    scroll_to(&mut app, area, 1_490);
    settle(&mut app, SETTLED);
    assert!(
        state(&app, area).is_following(),
        "a scroll_to to the bottom"
    );
    count(&mut app, 1_600);
    settle(&mut app, SETTLED);
    assert_eq!(shown_at(&mut app, area, 1_599), Some(VIEW - 20.0));

    scroll_to(&mut app, area, 0);
    settle(&mut app, SETTLED);
    assert!(
        !state(&app, area).is_following(),
        "a scroll_to to the top"
    );
    count(&mut app, 1_700);
    settle(&mut app, SETTLED);
    assert_eq!(scroll(&app, area), 0.0);
}

#[test]
fn a_wheel_flung_to_the_end_follows_the_rows_arriving() {
    let mut app = laid_out();
    app.insert_resource(Follow(true));
    let area = list(&mut app, 1_000, uniform);
    scroll_to(&mut app, area, 999);
    settle(&mut app, SETTLED);
    scroll_by(&mut app, area, -100.0);
    settle(&mut app, SETTLED);
    assert!(!state(&app, area).is_following(), "scrolled up");

    wheel(&mut app, area, -3.0);
    let mut frames = 0;
    while !state(&app, area).is_following() {
        frames += 1;
        assert!(
            frames <= 60,
            "following again within 60 frames, {} short of the end",
            overflow(&app, area).y - scroll(&app, area)
        );
        app.world_mut().resource_mut::<Count>().0 += 1;
        app.update();
    }
    for frame in 1..=30 {
        app.world_mut().resource_mut::<Count>().0 += 1;
        app.update();
        assert!(
            state(&app, area).is_following(),
            "frame {frame} after"
        );
    }
    let last = app.world().resource::<Count>().0 - 1;
    assert_eq!(shown_at(&mut app, area, last), Some(VIEW - 20.0));
}

#[derive(Resource, Default)]
struct NodeWrites(usize);

fn count_node_writes(
    nodes: Query<(), Changed<Node>>,
    mut writes: ResMut<NodeWrites>,
) {
    writes.0 += nodes.iter().count();
}

#[test]
fn a_still_list_writes_no_node() {
    let mut app = laid_out();
    list(&mut app, 100, uniform);
    app.init_resource::<NodeWrites>()
        .add_systems(Last, count_node_writes);
    app.update();
    app.world_mut().resource_mut::<NodeWrites>().0 = 0;

    app.update();
    app.update();

    assert_eq!(app.world().resource::<NodeWrites>().0, 0);
}

/// The rows of a spliced list: each one's id and height.
#[derive(Resource)]
struct Model(Vec<(u32, f32)>);

/// The id of the row a slot's view shows, written by the view.
#[derive(Component)]
struct Id(u32);

fn model(count: usize) -> Vec<(u32, f32)> {
    (0..count as u32)
        .map(|id| (id, 16.0 + (id * 37 % 9) as f32))
        .collect()
}

/// A row of the [`Model`]'s height for its index, that records the
/// index and id it shows, re-read every frame.
fn modelled(slot: RowSlot) -> AnyView<Bevy, Plain> {
    AnyView::new(move |cx| {
        let index = slot.index();
        let node = cx.build(
            frame().height(
                derived(move |world| {
                    px(world.resource::<Model>().0[index.get(world)]
                        .1)
                })
                .when(every_frame()),
            ),
        );
        let index = slot.index();
        let shows = derived(move |world| {
            let index = index.get(world);
            (index, world.resource::<Model>().0[index].0)
        })
        .when(every_frame());
        cx.effect(node, shows.into(), |world, node, &(index, id)| {
            world.entity_mut(node).insert((Shows(index), Id(id)));
        });
        node
    })
}

fn modelled_list(app: &mut App, count: usize) -> Entity {
    app.insert_resource(Model(model(count)));
    list_in(app, count, modelled, px(VIEW))
}

const SPLICED_HEIGHT: f32 = 22.0;

/// Splices the model and the list alike, with new ids.
fn splice_rows(
    world: &mut World,
    area: Entity,
    at: usize,
    removed: usize,
    inserted: usize,
) {
    let mut model = world.resource_mut::<Model>();
    let next =
        model.0.iter().map(|&(id, _)| id + 1).max().unwrap_or(0);
    let new =
        (next..next + inserted as u32).map(|id| (id, SPLICED_HEIGHT));
    model.0.splice(at..at + removed, new);
    let count = model.0.len();
    world.resource_mut::<Count>().0 = count;
    world
        .get_mut::<VirtualList>(area)
        .expect("a list")
        .splice(at, removed, inserted);
}

/// The rows built, by id: each one's index, node and top from the
/// top of the area.
fn ids(
    app: &mut App,
    area: Entity,
) -> HashMap<u32, (usize, Entity, f32)> {
    built(app)
        .into_iter()
        .map(|(index, node, _)| {
            let id = app.world().get::<Id>(node).expect("an id").0;
            (id, (index, node, top_of(app, area, node)))
        })
        .collect()
}

fn spans(list: &VirtualList) -> Vec<f32> {
    (0..list.count())
        .map(|index| list.row_span(index).expect("a row").height)
        .collect()
}

/// Every row sits where a list of `heights` would put it.
fn assert_stacked(list: &VirtualList, heights: &[f32], when: &str) {
    assert_eq!(list.count(), heights.len(), "{when}: the count");
    let mut top = 0.0;
    for (index, &height) in heights.iter().enumerate() {
        assert_eq!(
            list.row_span(index),
            Some(RowSpan { top, height }),
            "{when}: row {index}"
        );
        top += height;
    }
}

fn splice_case(
    view: usize,
    at: usize,
    removed: usize,
    inserted: usize,
) {
    const COUNT: usize = 2_100;
    let when = format!("{removed} for {inserted} at {at}");
    let removed = removed.min(COUNT - at);
    let mut app = laid_out();
    let area = modelled_list(&mut app, COUNT);
    scroll_to(&mut app, area, view);
    settle(&mut app, SETTLED);
    let before = ids(&mut app, area);
    let mut heights = spans(state(&app, area));
    heights.splice(
        at..at + removed,
        core::iter::repeat_n(ESTIMATE, inserted),
    );

    splice_rows(app.world_mut(), area, at, removed, inserted);
    assert_stacked(state(&app, area), &heights, &when);
    settle(&mut app, SETTLED);

    let list = state(&app, area);
    assert_stacked(list, &spans(list), &format!("{when}, settled"));
    let after = ids(&mut app, area);
    let model = &app.world().resource::<Model>().0;
    let list = state(&app, area);
    let mut kept = 0;
    for (&id, &(index, node, _)) in &after {
        assert_eq!(model[index].0, id, "{when}: row {index}'s id");
        assert_eq!(
            list.row_span(index).unwrap().height,
            model[index].1,
            "{when}: row {index}'s height"
        );
        if let Some(&(_, was, _)) = before.get(&id) {
            assert_eq!(node, was, "{when}: the view of {id}");
            kept += 1;
        }
    }
    assert!(kept > 0, "{when}: no row kept its view");
}

#[test]
fn rows_after_a_splice_move_and_keep_their_views() {
    for (removed, inserted) in [(0, 3), (3, 0), (2, 3)] {
        splice_case(0, 0, removed, inserted);
        splice_case(1_040, 1_050, removed, inserted);
        splice_case(2_099, 2_099, removed, inserted);
    }
}

/// The rows in the view, by id and top, from the top down.
fn on_screen(app: &mut App, area: Entity) -> Vec<(u32, f32)> {
    let mut shown = ids(app, area)
        .into_iter()
        .filter(|&(_, (_, _, top))| (0.0..VIEW).contains(&top))
        .map(|(id, (_, _, top))| (id, top))
        .collect::<Vec<_>>();
    shown.sort_by(|a, b| a.1.total_cmp(&b.1));
    shown
}

#[test]
fn a_splice_above_the_view_leaves_the_rows_on_screen_still() {
    let cases: [&[(usize, usize, usize)]; 3] = [
        &[(1_000, 0, 20)],
        &[(4_970, 3, 10)],
        &[(1_000, 0, 20), (4_980, 15, 2)],
    ];
    for splices in cases {
        let when = format!("{splices:?}");
        let mut app = laid_out();
        let area = modelled_list(&mut app, 10_000);
        scroll_to(&mut app, area, 5_000);
        settle(&mut app, SETTLED);
        scroll_by(&mut app, area, 7.0);
        settle(&mut app, SETTLED);
        let shown = on_screen(&mut app, area);
        let mut moved = state(&app, area).first_visible();

        for &(at, removed, inserted) in splices {
            splice_rows(app.world_mut(), area, at, removed, inserted);
            moved = moved - removed + inserted;
        }
        assert_eq!(
            state(&app, area).first_visible(),
            moved,
            "{when}"
        );
        app.update();
        assert_eq!(on_screen(&mut app, area), shown, "{when}");
        assert_eq!(
            state(&app, area).first_visible(),
            moved,
            "{when}"
        );
        settle(&mut app, SETTLED);
        assert_eq!(
            on_screen(&mut app, area),
            shown,
            "{when}, settled"
        );
    }

    let mut app = laid_out();
    let area = modelled_list(&mut app, 10_000);
    scroll_to(&mut app, area, 5_000);
    settle(&mut app, SETTLED);
    splice_rows(app.world_mut(), area, 4_990, 20, 0);
    assert_eq!(state(&app, area).first_visible(), 4_990);
    app.update();
    assert_eq!(
        uncovered(&mut app, area),
        0.0,
        "the first row removed"
    );
    settle(&mut app, SETTLED);
    assert_eq!(uncovered(&mut app, area), 0.0, "settled");
    assert_apart(&mut app, area, "settled");
}

/// A row that reads what it shows only when its slot moves.
fn read_on_move(slot: RowSlot) -> AnyView<Bevy, Plain> {
    AnyView::new(move |cx| {
        let node = cx.build(frame().height(px(ESTIMATE)));
        let index = slot.index();
        let mut moved = slot.index();
        let shows = derived(move |world| {
            let index = index.get(world);
            (index, world.resource::<Model>().0[index].0)
        })
        .when(move |world| moved.changed(world));
        cx.effect(node, shows.into(), |world, node, &(index, id)| {
            world.entity_mut(node).insert((Shows(index), Id(id)));
        });
        node
    })
}

#[test]
fn clearing_a_list_runs_no_row_against_a_removed_row() {
    type Row = fn(RowSlot) -> AnyView<Bevy, Plain>;
    for (row, reads) in [
        (read_on_move as Row, "on a move"),
        (modelled, "every frame"),
    ] {
        for spliced in [false, true] {
            let when = format!("read {reads}, spliced {spliced}");
            let mut app = laid_out();
            app.insert_resource(Model(model(30)));
            let area = list_in(&mut app, 30, row, px(VIEW));

            if spliced {
                splice_rows(app.world_mut(), area, 0, 30, 0);
            } else {
                app.world_mut().resource_mut::<Model>().0.clear();
                count(&mut app, 0);
            }
            settle(&mut app, SETTLED);

            assert!(rows(&mut app, area).is_empty(), "{when}");
            assert_eq!(state(&app, area).spawned(), None, "{when}");
        }
    }
}

#[test]
fn a_slot_refilled_at_its_own_index_shows_the_new_row() {
    let mut app = laid_out();
    app.insert_resource(Model(model(3)));
    let area = list_in(&mut app, 3, read_on_move, px(VIEW));

    splice_rows(app.world_mut(), area, 1, 1, 0);
    app.update();

    let mut shown = ids(&mut app, area)
        .into_iter()
        .map(|(id, (index, ..))| (index, id))
        .collect::<Vec<_>>();
    shown.sort();
    assert_eq!(shown, [(0, 0), (1, 2)]);
}

#[derive(Resource)]
struct Sideways(bool);

/// A cell that stays put as the row's sideways content moves.
#[derive(Component)]
struct Pinned;

/// The width of each row's sideways content.
#[derive(Resource)]
struct Wide(Vec<f32>);

/// A list [`Across`] wide, 300 px at first, whose rows are a pinned
/// cell 40 px wide and a sideways cell of content [`Wide`], 100 px
/// but for row 7 at 900 and row 9 at 500, scrolling sideways while
/// [`Sideways`] holds.
fn sideways_list(app: &mut App) -> Entity {
    let mut wide = vec![100.0; 10_000];
    (wide[7], wide[9]) = (900.0, 500.0);
    app.insert_resource(Wide(wide));
    app.insert_resource(Count(10_000));
    app.insert_resource(Across(300.0));
    app.insert_resource(Sideways(true));
    let row = |slot: RowSlot| {
        let pinned = AnyView::new(|cx| {
            let pinned =
                cx.build(frame().width(px(40.0)).height(px(20.0)));
            cx.world.entity_mut(pinned).insert(Pinned);
            pinned
        });
        let index = slot.index();
        let content = frame()
            .width(
                derived(move |world| {
                    px(world.resource::<Wide>().0[index.get(world)])
                })
                .when(every_frame()),
            )
            .height(px(20.0));
        let content = AnyView::new(move |cx| {
            let node = cx.build(content);
            cx.effect(
                node,
                slot.index().into(),
                |world, node, &index| {
                    world.entity_mut(node).insert(Shows(index));
                },
            );
            node
        });
        hrow((pinned, slot.sideways(content))).boxed()
    };
    mount::<Plain>(
        app.world_mut(),
        column((virtual_list(
            resource::<Count, _>(|count| count.0),
            ESTIMATE,
            row,
        )
        .sideways(resource::<Sideways, _>(|on| on.0)),))
        .width(resource::<Across, _>(|across| px(across.0)))
        .height(px(VIEW)),
    );
    settle(app, SETTLED);
    the_list(app)
}

fn left(app: &App, node: Entity) -> f32 {
    let computed = app.world().get::<ComputedNode>(node).unwrap();
    let at = app.world().get::<UiGlobalTransform>(node).unwrap();
    (at.translation.x - computed.size().x / 2.0)
        * computed.inverse_scale_factor()
}

fn right(app: &App, node: Entity) -> f32 {
    let computed = app.world().get::<ComputedNode>(node).unwrap();
    left(app, node)
        + computed.size().x * computed.inverse_scale_factor()
}

/// The content node of the row `index`.
fn shows(app: &mut App, index: usize) -> Entity {
    built(app)
        .into_iter()
        .find(|&(shown, ..)| shown == index)
        .map(|(_, node, _)| node)
        .expect("a built row")
}

/// The sideways cell of a row's content node: the node two above it,
/// past the one that moves.
fn cell_of(app: &App, content: Entity) -> Entity {
    let moved = app.world().get::<ChildOf>(content).unwrap().parent();
    app.world().get::<ChildOf>(moved).unwrap().parent()
}

fn scroll_x(app: &mut App, area: Entity, by: f32) {
    app.world_mut().get_mut::<ScrollPosition>(area).unwrap().x += by;
    settle(app, SETTLED);
}

fn reveal_x(app: &mut App, area: Entity, x: Range<f32>) {
    list_mut(app, area).scroll_x_into_view(x);
    settle(app, SETTLED);
}

fn x(app: &App, area: Entity) -> f32 {
    app.world().get::<ScrollPosition>(area).unwrap().x
}

#[test]
fn sideways_rows_share_one_offset_and_the_widest_sets_its_end() {
    let mut app = laid_out();
    let area = sideways_list(&mut app);
    let pinned = |app: &mut App| {
        app.world_mut()
            .query_filtered::<Entity, With<Pinned>>()
            .iter(app.world())
            .map(|pinned| left(app, pinned))
            .collect::<Vec<_>>()
    };
    let before = pinned(&mut app);
    let (narrow, long) = (shows(&mut app, 3), shows(&mut app, 7));
    let (narrow_at, long_at) = (left(&app, narrow), left(&app, long));
    let cell = cell_of(&app, long);

    scroll_x(&mut app, area, 50.0);
    assert_eq!(left(&app, narrow), narrow_at - 50.0);
    assert_eq!(left(&app, long), long_at - 50.0);
    assert_eq!(pinned(&mut app), before, "the pinned cells stay put");

    scroll_x(&mut app, area, 10_000.0);
    assert_eq!(right(&app, long), right(&app, cell), "the end shows");
    let by = long_at - left(&app, long);
    assert_eq!(left(&app, narrow), narrow_at - by);
    let ends = |app: &App| (x(app, area), overflow(app, area).x);
    assert_eq!(ends(&app), (by, by), "the bar spans the end");

    scroll_to(&mut app, area, 5_000);
    settle(&mut app, SETTLED);
    assert_eq!(
        ends(&app),
        (by, by),
        "a wide row out of sight keeps the range"
    );

    app.world_mut().resource_mut::<Wide>().0.remove(7);
    list_mut(&mut app, area).splice(7, 1, 0);
    count(&mut app, 9_999);
    settle(&mut app, SETTLED);
    let room = right(&app, cell) - left(&app, cell);
    let next = 500.0 - room;
    assert_eq!(ends(&app), (next, next), "the next widest sets it");

    app.insert_resource(Across(400.0));
    settle(&mut app, SETTLED);
    assert_eq!(
        ends(&app),
        (0.0, 0.0),
        "a width change forgets the rows out of sight"
    );
    scroll_to(&mut app, area, 0);
    settle(&mut app, SETTLED);
    assert_eq!(
        ends(&app),
        (0.0, next - 100.0),
        "until they show again"
    );

    count(&mut app, 8);
    settle(&mut app, SETTLED);
    assert_eq!(ends(&app), (0.0, 0.0), "the rows left fit");
}

#[test]
fn turning_sideways_off_and_on_keeps_the_rows() {
    let mut app = laid_out();
    let area = sideways_list(&mut app);
    scroll_to(&mut app, area, 3);
    settle(&mut app, SETTLED);
    scroll_x(&mut app, area, 10_000.0);
    let long = shows(&mut app, 7);
    let cell = cell_of(&app, long);
    let spans = |app: &App| {
        (0..20)
            .map(|index| state(app, area).row_span(index))
            .collect::<Vec<_>>()
    };
    let before = spans(&app);

    app.insert_resource(Sideways(false));
    settle(&mut app, SETTLED);
    assert_eq!(state(&app, area).first_visible(), 3);
    assert_eq!(spans(&app), before);
    assert_eq!(x(&app, area), 0.0);
    let moved = app.world().get::<ChildOf>(long).unwrap().parent();
    assert_eq!(
        left(&app, moved),
        left(&app, cell),
        "back at the start"
    );
    assert_eq!(
        right(&app, moved),
        right(&app, cell),
        "the cell's width"
    );
    scroll_x(&mut app, area, 10_000.0);
    assert_eq!(x(&app, area), 0.0, "no sideways scroll");

    app.insert_resource(Sideways(true));
    settle(&mut app, SETTLED);
    scroll_x(&mut app, area, 10_000.0);
    assert_eq!(right(&app, long), right(&app, cell), "the end shows");
    assert_eq!(state(&app, area).first_visible(), 3);
}

#[test]
fn a_sideways_reveal_moves_the_least() {
    let mut app = laid_out();
    let area = sideways_list(&mut app);
    let content = shows(&mut app, 3);
    let cell = cell_of(&app, content);
    let room = right(&app, cell) - left(&app, cell);

    reveal_x(&mut app, area, 500.0..600.0);
    assert_eq!(x(&app, area), 600.0 - room, "within the cell");

    app.world_mut().resource_mut::<Wide>().0[7] = 100.0;
    reveal_x(&mut app, area, 500.0..600.0);
    assert_eq!(
        (x(&app, area), overflow(&app, area).x),
        (500.0 - room, 500.0 - room),
        "no further than the widest row left"
    );
}
