//! `keyed` and `each` animating in and out against real bevy_ui
//! layout, headless: the space a container takes follows the
//! sequence, and never drops below what its children need.

use core::time::Duration;

use bevy::app::App;
use bevy::asset::AssetApp;
use bevy::camera::visibility::Visibility;
use bevy::camera::{
    Camera, Camera2d, ComputedCameraValues, RenderTargetInfo,
};
use bevy::color::{Alpha, Color};
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::math::UVec2;
use bevy::picking::Pickable;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{
    BackgroundColor, ComputedNode, Node, ScrollPosition,
    UiGlobalTransform, UiPlugin, px,
};
use fynix::{Motion, ScopedExt};

use super::children;
use crate::leave::{Collapsing, Held};
use crate::scroll::{ScrollbarStyle, ScrollbarVisibility};
use crate::tests::{Plain, app_with};
use crate::views::{FrameProps, column, frame, row, scroll};
use crate::{
    AnyView, Bevy, ReducedMotion, StateExt, View, ViewExt, each,
    hidden, keyed, mount, resource,
};

/// An app that lays its nodes out in a 1000 by 1000 window, 10ms to
/// an update.
fn laid_out() -> App {
    let mut app = app_with(Plain::default());
    app.insert_resource(TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(10),
    ));
    app.add_plugins((
        bevy::asset::AssetPlugin::default(),
        bevy::transform::TransformPlugin,
        bevy::input::InputPlugin,
        bevy::picking::PickingPlugin,
        bevy::picking::InteractionPlugin,
    ))
    .init_asset::<bevy::image::Image>()
    .init_resource::<bevy::text::TextPipeline>()
    .init_resource::<bevy::text::FontCx>()
    .init_resource::<bevy::text::ScaleCx>()
    .add_plugins(UiPlugin)
    // The text and image systems of the plugin have nothing to read
    // here, and skip their run.
    .set_error_handler(bevy::ecs::error::ignore);
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(1000, 1000),
                    scale_factor: 1.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    ));
    app
}

#[derive(Resource, Clone, Copy, PartialEq)]
enum Pick {
    Tall,
    Short,
}

#[derive(Resource)]
struct Ids(Vec<u32>);

fn size(app: &App, node: Entity) -> (f32, f32) {
    app.world().get::<ComputedNode>(node).map_or(
        (0.0, 0.0),
        |computed| {
            let size =
                computed.size() * computed.inverse_scale_factor();
            (size.x, size.y)
        },
    )
}

fn height(app: &App, node: Entity) -> f32 {
    size(app, node).1
}

fn width(app: &App, node: Entity) -> f32 {
    size(app, node).0
}

fn alpha(app: &App, node: Entity) -> f32 {
    app.world()
        .get::<BackgroundColor>(node)
        .expect("a fill")
        .0
        .alpha()
}

/// A column of `lines` frames 20px tall that expands and fades in
/// and out.
fn screen(lines: usize) -> AnyView<Bevy, Plain> {
    let lines = (0..lines)
        .map(|_| frame().height(px(20.0)).boxed())
        .collect::<Vec<_>>();
    column(lines)
        .gap(2.0)
        .appear::<Plain>(hidden)
        .transition(Motion::Expand)
        .boxed()
}

/// A header, a keyed container, and a footer, in a column with a gap
/// of 6. Returns the root and the container.
fn page(app: &mut App) -> (Entity, Entity) {
    let root = mount::<Plain>(
        app.world_mut(),
        column((
            frame().height(px(30.0)),
            keyed(
                resource::<Pick, _>(|pick| *pick),
                |pick| match pick {
                    Pick::Tall => screen(3),
                    Pick::Short => screen(1),
                },
            )
            .within(column(())),
            frame().height(px(10.0)),
        ))
        .gap(6.0),
    );
    (root, children(app, root)[1])
}

/// What a page takes without its container: the header, the footer,
/// and the gaps around the container.
const PAGE: f32 = 30.0 + 10.0 + 12.0;

/// A frame of an item in a list: 20px tall and 40px wide, filled,
/// expanding and fading in and out.
fn item(_: &u32) -> AnyView<Bevy, Plain> {
    frame()
        .width(px(40.0))
        .height(px(20.0))
        .fill(Color::WHITE)
        .appear::<Plain>(hidden)
        .transition(Motion::Expand)
        .boxed()
}

/// A header, an `each` in `container`, and a footer, in a column with
/// a gap of 6. Returns the root and the container.
fn list(
    app: &mut App,
    container: impl View<Bevy, Plain> + 'static,
) -> (Entity, Entity) {
    let root = mount::<Plain>(
        app.world_mut(),
        column((
            frame().height(px(30.0)),
            each(
                resource::<Ids, _>(|ids| ids.0.clone()),
                |id| *id,
                item,
            )
            .within(container),
            frame().height(px(10.0)),
        ))
        .gap(6.0),
    );
    (root, children(app, root)[1])
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// The nodes in the flow under `container`, which a layout has
/// placed.
fn in_flow(app: &App, container: Entity) -> Vec<Entity> {
    children(app, container)
        .into_iter()
        .filter(|&node| !app.world().entity(node).contains::<Held>())
        .collect()
}

/// How far down `node` ends from the top of `container`, as laid
/// out.
fn bottom_of(app: &App, container: Entity, node: Entity) -> f32 {
    let top = |node: Entity| {
        let at = app
            .world()
            .get::<UiGlobalTransform>(node)
            .expect("a placed node")
            .translation
            .y;
        at - height(app, node) / 2.0
    };
    top(node) + height(app, node) - top(container)
}

/// Asserts that no in-flow child of `container` ends below it: a
/// container takes at least the space its children do.
fn assert_fits(app: &App, container: Entity, frame: usize) {
    let own = height(app, container);
    for node in in_flow(app, container) {
        let bottom = bottom_of(app, container, node);
        assert!(
            bottom <= own + 1.0,
            "frame {frame}: a child ends at {bottom}, its container \
             at {own}",
        );
    }
}

#[test]
fn layout_runs_headless() {
    let mut app = laid_out();
    app.insert_resource(Pick::Tall);
    let (root, container) = page(&mut app);
    settle(&mut app, 2);

    assert_eq!(height(&app, container), 3.0 * 20.0 + 2.0 * 2.0);
    assert_eq!(height(&app, root), PAGE + 64.0);
}

/// Swaps the page's view, and records the container's height and the
/// page's at every update after.
fn swap(
    app: &mut App,
    (root, container): (Entity, Entity),
    pick: Pick,
    frames: usize,
) -> Vec<(f32, f32)> {
    *app.world_mut().resource_mut::<Pick>() = pick;
    let mut seen = Vec::new();
    for frame in 0..frames {
        app.update();
        assert_fits(app, container, frame);
        seen.push((height(app, container), height(app, root)));
    }
    seen
}

#[test]
fn a_swap_to_a_shorter_view_collapses_then_expands() {
    let mut app = laid_out();
    app.insert_resource(Pick::Tall);
    let page = page(&mut app);
    settle(&mut app, 40);

    let seen = swap(&mut app, page, Pick::Short, 70);

    let heights = seen.iter().map(|seen| seen.0).collect::<Vec<_>>();
    let lowest = heights.iter().copied().fold(f32::MAX, f32::min);
    assert_eq!(lowest, 0.0, "the old space is all gone");
    let bottom = heights
        .iter()
        .position(|&height| height == 0.0)
        .expect("a bottom");
    let top = heights.iter().rposition(|&h| h > 0.0).unwrap();
    assert!(
        heights[..bottom].windows(2).all(|pair| pair[1] <= pair[0]),
        "collapsing only shrinks: {heights:?}"
    );
    assert!(
        heights[top - 8..=top]
            .windows(2)
            .all(|pair| pair[1] >= pair[0]),
        "expanding only grows: {heights:?}"
    );
    assert_eq!(heights[0], 64.0, "the old view fades at full size");
    assert_eq!(*heights.last().unwrap(), 20.0);
    assert_eq!(seen.last().unwrap().1, PAGE + 20.0);
    // The page is never below its floor, nor above its start.
    for (_, page) in &seen {
        assert!((PAGE..=PAGE + 64.0).contains(page), "{page}");
    }
}

#[test]
fn a_swap_to_a_taller_view_collapses_then_expands() {
    let mut app = laid_out();
    app.insert_resource(Pick::Short);
    let page = page(&mut app);
    settle(&mut app, 40);

    let seen = swap(&mut app, page, Pick::Tall, 70);

    let heights = seen.iter().map(|seen| seen.0).collect::<Vec<_>>();
    assert_eq!(heights[0], 20.0);
    assert_eq!(heights.iter().copied().fold(f32::MAX, f32::min), 0.0);
    assert_eq!(*heights.last().unwrap(), 64.0);
    assert_eq!(seen.last().unwrap().1, PAGE + 64.0);
}

#[test]
fn the_new_view_does_not_take_its_space_while_the_old_one_fades() {
    let mut app = laid_out();
    app.insert_resource(Pick::Tall);
    let (_, container) = page(&mut app);
    settle(&mut app, 40);

    *app.world_mut().resource_mut::<Pick>() = Pick::Short;
    // The fade of the old view takes 10 updates.
    for _ in 0..8 {
        app.update();
        assert_eq!(height(&app, container), 64.0, "the old one's");
    }
    let new = children(&app, container)[1];
    assert!(app.world().get::<Held>(new).is_some());
    assert_eq!(
        app.world().get::<Visibility>(new),
        Some(&Visibility::Hidden)
    );
    assert_eq!(
        app.world().get::<Pickable>(new),
        Some(&Pickable::IGNORE)
    );
}

#[test]
fn a_new_view_never_takes_its_full_size_before_it_expands() {
    let mut app = laid_out();
    app.insert_resource(Pick::Tall);
    let (root, container) = page(&mut app);
    settle(&mut app, 40);
    // Nothing to wait for: an empty container takes the new view.
    *app.world_mut().resource_mut::<Pick>() = Pick::Short;
    settle(&mut app, 40);
    assert_eq!(height(&app, container), 20.0);
    let before = height(&app, root);

    *app.world_mut().resource_mut::<Pick>() = Pick::Tall;
    let mut seen = Vec::new();
    for frame in 0..70 {
        app.update();
        assert_fits(&app, container, frame);
        seen.push(height(&app, root));
    }
    // Neither the update that built it nor the one that measured it
    // laid it out at its natural size.
    assert_eq!(seen[0], before);
    assert_eq!(seen[1], before);
    assert!(seen.iter().all(|&height| height <= PAGE + 64.0));
}

#[test]
fn a_view_without_an_entering_rule_is_held_just_the_same() {
    let mut app = laid_out();
    app.insert_resource(Pick::Tall);
    let root = mount::<Plain>(
        app.world_mut(),
        column((
            frame().height(px(30.0)),
            keyed(resource::<Pick, _>(|pick| *pick), |pick| {
                let lines = if *pick == Pick::Tall { 3 } else { 1 };
                column(
                    (0..lines)
                        .map(|_| frame().height(px(20.0)).boxed())
                        .collect::<Vec<_>>(),
                )
                .transition(Motion::Expand)
                .boxed()
            })
            .within(column(())),
        ))
        .gap(6.0),
    );
    let container = children(&app, root)[1];
    settle(&mut app, 4);
    let tall = height(&app, container);

    *app.world_mut().resource_mut::<Pick>() = Pick::Short;
    settle(&mut app, 2);
    let new = children(&app, container)[1];
    assert!(app.world().get::<Held>(new).is_some());
    assert_eq!(height(&app, container), tall, "still the old view");
    settle(&mut app, 60);
    assert!(app.world().get::<Held>(new).is_none());
    assert_eq!(height(&app, container), 20.0);
}

#[test]
fn a_removed_item_never_leaves_less_space_than_the_end() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1, 2, 3]));
    let (root, container) = list(&mut app, column(()));
    settle(&mut app, 40);
    let start = height(&app, root);
    assert_eq!(start, PAGE + 3.0 * 20.0 + 2.0 * 6.0);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 3];
    let end = PAGE + 2.0 * 20.0 + 6.0;
    let mut seen = Vec::new();
    for frame in 0..40 {
        app.update();
        assert_fits(&app, container, frame);
        seen.push(height(&app, root));
    }

    assert!(
        seen.iter().all(|&height| height >= end),
        "dipped below {end}: {seen:?}"
    );
    assert!(
        seen.windows(2).all(|pair| pair[1] <= pair[0]),
        "only shrinks: {seen:?}"
    );
    assert_eq!(seen[5], start, "fading takes no space");
    assert_eq!(*seen.last().unwrap(), end);
    assert_eq!(children(&app, container).len(), 2);
}

#[test]
fn removing_the_first_and_the_last_item_never_dips_either() {
    for gone in [vec![2, 3], vec![1, 2]] {
        let mut app = laid_out();
        app.insert_resource(Ids(vec![1, 2, 3]));
        let (root, container) = list(&mut app, column(()));
        settle(&mut app, 40);

        app.world_mut().resource_mut::<Ids>().0 = gone;
        let end = PAGE + 2.0 * 20.0 + 6.0;
        for frame in 0..40 {
            app.update();
            assert_fits(&app, container, frame);
            assert!(height(&app, root) >= end);
        }
        assert_eq!(height(&app, root), end);
    }
}

#[test]
fn an_inserted_item_expands_its_space_before_it_fades_in() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1, 2]));
    let (root, container) = list(&mut app, column(()));
    settle(&mut app, 40);
    let start = height(&app, root);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 2, 3];
    let end = start + 6.0 + 20.0;
    let mut seen = Vec::new();
    let mut fade = Vec::new();
    for frame in 0..40 {
        app.update();
        assert_fits(&app, container, frame);
        seen.push(height(&app, root));
        let node = children(&app, container)[2];
        fade.push((
            app.world().get::<Held>(node).is_some(),
            alpha(&app, node),
        ));
    }

    assert_eq!(seen[0], start, "held out of the layout");
    assert_eq!(seen[1], start, "measured, takes nothing yet");
    assert!(
        seen.windows(2).all(|pair| pair[1] >= pair[0]),
        "only grows: {seen:?}"
    );
    assert_eq!(*seen.last().unwrap(), end);
    let grown =
        seen.iter().position(|&height| height == end).unwrap();
    // It is invisible for as long as its space is growing.
    for (frame, &(held, alpha)) in fade.iter().enumerate() {
        if frame < grown {
            assert_eq!(alpha, 0.0, "frame {frame} is still entering");
        }
        if frame > grown + 12 {
            assert!(!held);
            assert_eq!(alpha, 1.0);
        }
    }
    assert!(fade[grown + 4].1 > 0.0, "then it fades in");
}

#[test]
fn items_expand_along_a_rows_main_axis() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1, 2]));
    let (_, container) = list(&mut app, row(()).gap(4.0));
    settle(&mut app, 40);
    assert_eq!(width(&app, container), 2.0 * 40.0 + 4.0);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 2, 3];
    let mut seen = Vec::new();
    for _ in 0..40 {
        app.update();
        seen.push(width(&app, container));
    }
    assert!(
        seen.windows(2).all(|pair| pair[1] >= pair[0]),
        "{seen:?}"
    );
    assert_eq!(*seen.last().unwrap(), 3.0 * 40.0 + 2.0 * 4.0);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 3];
    let mut seen = Vec::new();
    for _ in 0..40 {
        app.update();
        seen.push(width(&app, container));
    }
    assert!(
        seen.windows(2).all(|pair| pair[1] <= pair[0]),
        "{seen:?}"
    );
    assert_eq!(*seen.last().unwrap(), 2.0 * 40.0 + 4.0);
}

#[test]
fn a_node_that_cannot_shrink_collapses_without_taking_its_container_down()
 {
    // A negative margin on an item that does not shrink makes bevy_ui
    // lay its container out at nothing, which is why a collapse sets
    // the item's own shrink back.
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1, 2]));
    let root = mount::<Plain>(
        app.world_mut(),
        column((each(
            resource::<Ids, _>(|ids| ids.0.clone()),
            |id| *id,
            |_| frame().height(px(40.0)).shrink(0.0).boxed(),
        )
        .within(column(())),))
        .gap(6.0),
    );
    settle(&mut app, 4);
    let container = children(&app, root)[0];
    let first = children(&app, container)[0];
    assert_eq!(height(&app, container), 86.0);

    for progress in [0.0, 0.25, 0.5, 0.9, 1.0] {
        crate::leave::collapse(app.world_mut(), first, progress);
        app.update();
        let expected =
            40.0 * (1.0 - progress) + 6.0 * (1.0 - progress) + 40.0;
        assert!(
            (height(&app, container) - expected).abs() <= 1.0,
            "at {progress}: {} not {expected}",
            height(&app, container)
        );
        assert_fits(&app, container, 0);
    }
    assert!(app.world().get::<Collapsing>(first).is_some());
}

#[test]
fn reduced_motion_swaps_at_once() {
    let mut app = laid_out();
    app.insert_resource(Pick::Tall)
        .insert_resource(ReducedMotion(true));
    let (root, container) = page(&mut app);
    settle(&mut app, 4);

    *app.world_mut().resource_mut::<Pick>() = Pick::Short;
    app.update();
    let new = children(&app, container)[0];
    assert_eq!(
        children(&app, container).len(),
        1,
        "no fade, no collapse"
    );
    assert!(app.world().get::<Held>(new).is_none());
    assert!(app.world().get::<Collapsing>(new).is_none());
    assert_eq!(
        app.world().get::<Visibility>(new),
        Some(&Visibility::Inherited)
    );
    app.update();
    assert_eq!(height(&app, container), 20.0, "no expand");
    assert_eq!(height(&app, root), PAGE + 20.0);
}

#[test]
fn reduced_motion_shows_an_inserted_item_whole_at_once() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1]))
        .insert_resource(ReducedMotion(true));
    let (root, container) = list(&mut app, column(()));
    settle(&mut app, 4);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 2];
    settle(&mut app, 3);

    let node = children(&app, container)[1];
    assert_eq!(height(&app, root), PAGE + 20.0 + 6.0 + 20.0);
    assert!(app.world().get::<Held>(node).is_none());
    assert_eq!(alpha(&app, node), 1.0, "no fade in");
}

#[test]
fn reduced_motion_removes_an_item_at_once() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1, 2]))
        .insert_resource(ReducedMotion(true));
    let (root, container) = list(&mut app, column(()));
    settle(&mut app, 4);

    app.world_mut().resource_mut::<Ids>().0 = vec![1];
    settle(&mut app, 2);

    assert_eq!(children(&app, container).len(), 1);
    assert_eq!(height(&app, root), PAGE + 20.0);
}

#[test]
fn a_released_node_is_laid_out_and_picked_as_built() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1]));
    let (_, container) = list(&mut app, column(()));
    settle(&mut app, 4);
    let styled = |app: &App, node| {
        let ui = app.world().get::<Node>(node).unwrap();
        (
            ui.position_type,
            ui.width,
            ui.height,
            ui.flex_shrink,
            ui.overflow,
        )
    };
    let first = children(&app, container)[0];
    let before = styled(&app, first);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 2];
    settle(&mut app, 3);
    let second = children(&app, container)[1];
    assert_eq!(
        app.world().get::<Pickable>(second),
        Some(&Pickable::IGNORE),
        "not interactive while it comes in"
    );
    settle(&mut app, 40);

    assert_eq!(styled(&app, second), before);
    assert_ne!(
        app.world().get::<Pickable>(second),
        Some(&Pickable::IGNORE)
    );
    assert_eq!(
        app.world().get::<Visibility>(second),
        Some(&Visibility::Inherited)
    );
}

#[derive(Resource)]
struct Wide(f32);

#[test]
fn a_size_bound_while_held_is_kept_for_the_release() {
    let mut app = laid_out();
    app.insert_resource(Ids(vec![1]))
        .insert_resource(Wide(40.0));
    let root = mount::<Plain>(
        app.world_mut(),
        each(
            resource::<Ids, _>(|ids| ids.0.clone()),
            |id| *id,
            |_| {
                frame()
                    .width(resource::<Wide, _>(|wide| px(wide.0)))
                    .height(px(20.0))
                    .appear::<Plain>(hidden)
                    .transition(Motion::Expand)
                    .boxed()
            },
        )
        .within(column(())),
    );
    settle(&mut app, 4);

    app.world_mut().resource_mut::<Ids>().0 = vec![1, 2];
    app.update();
    let second = children(&app, root)[1];
    assert!(app.world().get::<Held>(second).is_some());
    app.world_mut().resource_mut::<Wide>().0 = 80.0;
    settle(&mut app, 40);

    assert!(app.world().get::<Held>(second).is_none());
    assert_eq!(
        app.world().get::<Node>(second).unwrap().width,
        px(80.0)
    );
}

/// The bars of `area` that show, each as its size and how far its
/// middle is from the area's.
fn bars(app: &App, area: Entity) -> Vec<((f32, f32), (f32, f32))> {
    let middle = |node: Entity| {
        app.world()
            .get::<UiGlobalTransform>(node)
            .expect("placed")
            .to_scale_angle_translation()
            .2
    };
    children(app, area)
        .into_iter()
        .filter(|&node| !app.world().entity(node).contains::<Node>())
        .filter(|&node| {
            app.world().get::<Visibility>(node)
                != Some(&Visibility::Hidden)
        })
        .map(|bar| {
            let off = middle(bar) - middle(area);
            (size(app, bar), (off.x, off.y))
        })
        .collect()
}

#[test]
fn a_scroll_areas_bar_is_its_share_long_and_ends_where_it_does() {
    let mut app = laid_out();
    app.world_mut().resource_mut::<ScrollbarStyle>().visibility =
        ScrollbarVisibility::WhenNeeded;
    // 100 tall, with 200 to show: a track of 96 between the insets.
    let area = mount::<Plain>(
        app.world_mut(),
        scroll((frame().height(px(200.0)).shrink(0.0),))
            .width(px(200.0))
            .height(px(100.0))
            .gap(0.0),
    );
    settle(&mut app, 3);
    assert_eq!(
        bars(&app, area),
        [((6.0, 48.0), (95.0, -24.0))],
        "at the top, along the right edge, and none across"
    );

    app.world_mut().get_mut::<ScrollPosition>(area).unwrap().y =
        100.0;
    settle(&mut app, 3);
    assert_eq!(bars(&app, area), [((6.0, 48.0), (95.0, 24.0))]);

    // Beside the content, in room the area keeps for it.
    app.world_mut().resource_mut::<ScrollbarStyle>().floating = false;
    settle(&mut app, 3);
    let content = children(&app, area)
        .into_iter()
        .find(|&node| app.world().entity(node).contains::<Node>())
        .unwrap();
    assert_eq!(width(&app, content), 190.0);

    app.world_mut().resource_mut::<ScrollbarStyle>().visibility =
        ScrollbarVisibility::Hidden;
    settle(&mut app, 3);
    assert_eq!(bars(&app, area), []);
    assert_eq!(width(&app, content), 200.0);
}
