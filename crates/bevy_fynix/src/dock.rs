//! Docking: split panes with tabbed areas, shown from a
//! [`DockTree`].
//!
//! The layout is built again only when the tree's [`Shape`] changes.
//! Everything that changes more often is a bound prop: the share of a
//! split, the active tab, and the tabs of one area.

mod drag;
mod layout;
mod popup;
mod registry;
mod tabs;
mod tree;

use core::marker::PhantomData;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::system::Res;
use bevy::math::{Rect, Vec2};
use bevy::ui::{
    ComputedNode, UiGlobalTransform, UiRect, UiScale, percent, px,
};
pub use drag::{DockDrag, DropTarget};
pub use layout::DockArea;
pub use registry::{DockRegistry, DockWindowKind};
pub use tabs::{ActiveTab, DockIcons, DockTab};
pub use tree::{
    DockAreaStyle, DockLeaf, DockNode, DockSplit, DockTabEntry,
    DockTree, Edge, NodeId, Shape, SplitAxis, TabId,
};

use crate::cursor::OverrideCursor;
use crate::prop::{keyed, resource};
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
};
use crate::views::{BehaviorExt, FrameProps, column};
use crate::{AnyView, Bevy, Cx, ViewExt};

/// The token traits the dock's views read.
pub trait DockTokens:
    TextTokens
    + SurfaceTokens
    + SpacingTokens
    + MotionTokens
    + Send
    + Sync
    + 'static
{
}

impl<T> DockTokens for T where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static
{
}

/// The resources and observers the dock needs, for views built with
/// the theme `T`. [`DockTree`] and [`DockRegistry<T>`] are filled by
/// the app.
pub struct DockPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for DockPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: DockTokens> Plugin for DockPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_resource::<DockTree>()
            .init_resource::<DockRegistry<T>>()
            .init_resource::<DockDrag>()
            .init_resource::<OverrideCursor>()
            .add_observer(layout::grab_handle)
            .add_observer(layout::release_handle)
            .add_observer(layout::drag_handle)
            .add_observer(drag::start)
            .add_observer(drag::moved::<T>)
            .add_observer(drag::end)
            .add_systems(Update, (drag::cancel, tabs::show_close));
    }
}

/// On the root node of a [`dock`].
#[derive(Component, Clone, Copy, Debug)]
pub struct DockRoot;

/// The dock shown from the [`DockTree`] resource: split panes, each
/// leaf a tab bar over the content of its active tab.
pub fn dock<T: DockTokens>() -> AnyView<Bevy, T> {
    let layout = AnyView::new(|cx: &mut Cx<'_, Bevy, T>| {
        // The rest of the gap, with each area's own inset making a
        // whole one at the edge.
        let inset = cx.theme().dock_gap() - layout::inset(cx.theme());
        cx.build(
            keyed(
                resource::<DockTree, _>(DockTree::shape),
                |shape| layout::build(shape),
            )
            .within(
                column(())
                    .gap(0.0)
                    .padding(UiRect::all(px(inset)))
                    .width(percent(100.0))
                    .height(percent(100.0)),
            ),
        )
    });
    column((layout,))
        .gap(0.0)
        .width(percent(100.0))
        .height(percent(100.0))
        .tagged(DockRoot)
        .boxed()
}

/// A pointer position in logical pixels, from the window's.
fn logical(position: Vec2, scale: Option<Res<UiScale>>) -> Vec2 {
    position / scale.map_or(1.0, |scale| scale.0)
}

/// The rect of a node in logical pixels.
fn logical_rect(
    computed: &ComputedNode,
    transform: &UiGlobalTransform,
) -> Rect {
    let inverse = computed.inverse_scale_factor();
    let size = computed.size() * inverse;
    let (_, _, center) = transform.to_scale_angle_translation();
    Rect::from_center_size(center.trunc() * inverse, size)
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::asset::Handle;
    use bevy::color::Color;
    use bevy::ecs::entity::Entity;
    use bevy::ecs::hierarchy::ChildOf;
    use bevy::ecs::query::With;
    use bevy::ui::widget::{ImageNode, Text};
    use bevy::ui::{
        BackgroundColor, Display, FlexDirection, Node, Val, px,
    };
    use bevy::ui_widgets::{
        Activate, MenuAction, MenuEvent, MenuItem,
    };

    use super::tabs::AddButton;
    use super::*;
    use crate::tests::{self, Plain, kids};
    use crate::tokens::{SurfaceTokens, TextTokens, Tone};
    use crate::views::{Open, label};
    use crate::{Dragging, mount};

    fn window(
        name: &'static str,
        text: &'static str,
    ) -> DockWindowKind<Plain> {
        DockWindowKind::new(name, move || label(text).boxed())
    }

    /// A tree of the areas `left` and `right` side by side, and a
    /// registry of the windows in them and one more, `extra`.
    fn app() -> (App, NodeId, NodeId) {
        app_with(None)
    }

    /// [`app`], with the app's `icons` inserted before the dock is
    /// built.
    fn app_with(icons: Option<DockIcons>) -> (App, NodeId, NodeId) {
        let mut app = tests::app();
        app.add_plugins(DockPlugin::<Plain>::default());
        if let Some(icons) = icons {
            app.insert_resource(icons);
        }
        app.world_mut()
            .resource_mut::<DockRegistry<Plain>>()
            .register("one", window("One", "one body"))
            .register("two", window("Two", "two body"))
            .register("three", window("Three", "three body"))
            .register("extra", window("Extra", "extra body"));
        let mut tree = DockTree::new();
        let left = tree.set_root_leaf(
            DockLeaf::new("left", DockAreaStyle::TabBar)
                .with_windows(vec!["one".into(), "two".into()]),
        );
        let (right, _) =
            tree.split(left, Edge::Right, "three".into()).unwrap();
        app.insert_resource(tree);
        mount::<Plain>(app.world_mut(), dock::<Plain>());
        // The first update only starts the clock.
        app.update();
        (app, left, right)
    }

    fn subtree(app: &App, node: Entity) -> Vec<Entity> {
        let mut all = vec![node];
        let mut next = 0;
        while next < all.len() {
            all.extend(kids(app, all[next]));
            next += 1;
        }
        all
    }

    /// The text under `node`, sorted.
    fn texts(app: &App, node: Entity) -> Vec<String> {
        let mut texts = subtree(app, node)
            .into_iter()
            .filter_map(|node| app.world().get::<Text>(node))
            .map(|text| text.0.clone())
            .collect::<Vec<_>>();
        texts.sort();
        texts
    }

    fn find<C: bevy::ecs::component::Component + Copy>(
        app: &mut App,
    ) -> Vec<(Entity, C)> {
        app.world_mut()
            .query::<(Entity, &C)>()
            .iter(app.world())
            .map(|(entity, component)| (entity, *component))
            .collect()
    }

    fn width(app: &App, node: Entity) -> Val {
        app.world().get::<Node>(node).unwrap().width
    }

    fn percent_of(app: &App, node: Entity) -> f32 {
        match width(app, node) {
            Val::Percent(share) => share,
            other => panic!("not a percentage: {other:?}"),
        }
    }

    fn display(app: &App, node: Entity) -> Display {
        app.world().get::<Node>(node).unwrap().display
    }

    fn area_of(app: &mut App, leaf: NodeId) -> Entity {
        find::<layout::DockArea>(app)
            .into_iter()
            .find(|(_, area)| area.leaf == leaf)
            .map(|(entity, _)| entity)
            .expect("an area")
    }

    /// The panes of the content container of `area`, in order.
    fn contents(app: &App, area: Entity) -> Vec<Entity> {
        let [_, container] = kids(app, area)[..] else {
            panic!("a bar and the contents");
        };
        kids(app, container)
    }

    /// Updates until what animates out is gone.
    fn settle(app: &mut App) {
        for _ in 0..6 {
            app.update();
        }
    }

    #[test]
    fn a_split_of_two_areas_builds_its_nodes() {
        let (mut app, left, right) = app();
        let root = find::<DockRoot>(&mut app)[0].0;

        let [layout] = kids(&app, root)[..] else {
            panic!("the layout");
        };
        let [split] = kids(&app, layout)[..] else {
            panic!("one split");
        };
        let ui = app.world().get::<Node>(split).unwrap();
        assert_eq!(ui.flex_direction, FlexDirection::Row);
        let [first, handle, second] = kids(&app, split)[..] else {
            panic!("two panes and a handle");
        };
        assert_eq!(width(&app, first), Val::Percent(50.0));
        assert_eq!(width(&app, second), Val::Percent(50.0));
        assert_eq!(
            width(&app, handle),
            px(Plain::default().divider())
        );
        assert!(
            app.world().get::<layout::SplitHandle>(handle).is_some()
        );

        let left_area = area_of(&mut app, left);
        let right_area = area_of(&mut app, right);
        assert_eq!(kids(&app, first), [left_area]);
        assert_eq!(kids(&app, second), [right_area]);
        assert_eq!(contents(&app, left_area).len(), 2);
        assert_eq!(contents(&app, right_area).len(), 1);
        assert_eq!(
            texts(&app, left_area),
            ["+", "One", "Two", "one body", "two body", "x", "x"]
        );
        assert_eq!(
            texts(&app, right_area),
            ["+", "Three", "three body", "x"]
        );
    }

    #[test]
    fn a_headless_area_has_no_bar() {
        let (mut app, left, _) = app();
        app.world_mut()
            .resource_mut::<DockTree>()
            .get_mut(left)
            .and_then(DockNode::as_leaf_mut)
            .unwrap()
            .style = DockAreaStyle::Headless;
        app.update();

        let area = area_of(&mut app, left);
        assert_eq!(kids(&app, area).len(), 1, "only the contents");
        assert!(texts(&app, area).iter().all(|text| text != "+"));
    }

    #[test]
    fn a_new_fraction_resizes_the_panes_and_builds_nothing() {
        let (mut app, _, _) = app();
        let root = find::<DockRoot>(&mut app)[0].0;
        let before = subtree(&app, root);
        let split = app.world().resource::<DockTree>().root.unwrap();

        app.world_mut()
            .resource_mut::<DockTree>()
            .set_fraction(split, 0.3);
        app.update();

        assert_eq!(subtree(&app, root), before, "same entities");
        let [first, _, second] =
            kids(&app, kids(&app, kids(&app, root)[0])[0])[..]
        else {
            panic!("two panes and a handle");
        };
        assert!((percent_of(&app, first) - 30.0).abs() < 1e-4);
        assert!((percent_of(&app, second) - 70.0).abs() < 1e-4);

        // Once more, as a drag does every frame.
        app.world_mut()
            .resource_mut::<DockTree>()
            .set_fraction(split, 0.6);
        app.update();
        assert_eq!(subtree(&app, root), before);
        assert!((percent_of(&app, first) - 60.0).abs() < 1e-4);
    }

    #[test]
    fn a_vertical_split_sizes_the_heights() {
        let (mut app, left, _) = app();
        app.world_mut().resource_mut::<DockTree>().split(
            left,
            Edge::Bottom,
            "extra".into(),
        );
        app.update();

        let root = find::<DockRoot>(&mut app)[0].0;
        let inner =
            app.world().resource::<DockTree>().parent_of(left);
        let (inner, _) = (inner.unwrap(), root);
        let handle = find::<layout::SplitHandle>(&mut app)
            .into_iter()
            .find(|(_, handle)| handle.split == inner)
            .map(|(entity, _)| entity)
            .unwrap();
        let [first, _, second] = kids(
            &app,
            app.world().get::<ChildOf>(handle).unwrap().parent(),
        )[..] else {
            panic!("two panes and a handle");
        };
        let ui = |node| app.world().get::<Node>(node).unwrap();
        assert_eq!(ui(first).height, Val::Percent(50.0));
        assert_eq!(ui(second).height, Val::Percent(50.0));
        assert_eq!(ui(first).width, Val::Percent(100.0));
    }

    #[test]
    fn activating_a_tab_shows_its_content_and_hides_the_other() {
        let (mut app, left, _) = app();
        let area = area_of(&mut app, left);
        let [one, two] = contents(&app, area)[..] else {
            panic!("two contents");
        };
        assert_eq!(display(&app, one), Display::Flex);
        assert_eq!(display(&app, two), Display::None);

        let tab_two = find::<tabs::DockTab>(&mut app)
            .into_iter()
            .find(|(_, tab)| {
                app.world()
                    .resource::<DockTree>()
                    .leaf(left)
                    .unwrap()
                    .windows[1]
                    .id
                    == tab.tab
            })
            .map(|(entity, _)| entity)
            .unwrap();
        app.world_mut().trigger(Activate { entity: tab_two });
        app.update();

        assert_eq!(display(&app, one), Display::None);
        assert_eq!(display(&app, two), Display::Flex);
    }

    #[test]
    fn adding_and_closing_a_tab_rebuilds_only_that_area() {
        let (mut app, left, right) = app();
        let root = find::<DockRoot>(&mut app)[0].0;
        let left_area = area_of(&mut app, left);
        let right_area = area_of(&mut app, right);
        let right_before = subtree(&app, right_area);
        let left_before = subtree(&app, left_area);
        let whole = subtree(&app, root);

        let added = app
            .world_mut()
            .resource_mut::<DockTree>()
            .add_tab(left, "extra")
            .unwrap();
        settle(&mut app);

        assert_eq!(subtree(&app, right_area), right_before);
        let left_now = subtree(&app, left_area);
        assert!(
            left_before.iter().all(|node| left_now.contains(node))
        );
        assert!(left_now.len() > left_before.len());
        assert_eq!(contents(&app, left_area).len(), 3);
        assert!(
            texts(&app, left_area).contains(&"extra body".into())
        );
        let after = subtree(&app, root);
        assert!(whole.iter().all(|node| after.contains(node)));

        app.world_mut().resource_mut::<DockTree>().remove_tab(added);
        settle(&mut app);

        assert_eq!(subtree(&app, right_area), right_before);
        assert_eq!(subtree(&app, left_area), left_before);
        assert_eq!(contents(&app, left_area).len(), 2);
    }

    #[test]
    fn the_close_button_removes_its_tab_and_keeps_the_others() {
        let (mut app, left, _) = app();
        let left_area = area_of(&mut app, left);
        let [one, _] = contents(&app, left_area)[..] else {
            panic!("two contents");
        };
        let tab = app
            .world()
            .resource::<DockTree>()
            .leaf(left)
            .unwrap()
            .windows[1]
            .id;
        let tab_node = find::<DockTab>(&mut app)
            .into_iter()
            .find(|(_, dock_tab)| dock_tab.tab == tab)
            .map(|(entity, _)| entity)
            .unwrap();
        let cross = subtree(&app, tab_node)
            .into_iter()
            .find(|&node| {
                app.world()
                    .get::<Text>(node)
                    .is_some_and(|text| text.0 == "x")
            })
            .expect("a cross");
        let close =
            app.world().get::<ChildOf>(cross).unwrap().parent();

        app.world_mut().trigger(Activate { entity: close });
        settle(&mut app);

        let remaining = app
            .world()
            .resource::<DockTree>()
            .leaf(left)
            .unwrap()
            .windows
            .len();
        assert_eq!(remaining, 1);
        assert_eq!(
            contents(&app, left_area),
            [one],
            "its content stays"
        );
        assert_eq!(
            texts(&app, left_area),
            ["+", "One", "one body", "x"]
        );
    }

    #[test]
    fn a_close_button_shows_only_while_its_tab_is_hovered() {
        let (mut app, _, _) = app();
        let tabs = find::<DockTab>(&mut app);
        let visibility = |app: &App, tab: Entity| {
            let close = subtree(app, tab)
                .into_iter()
                .find(|&node| {
                    app.world()
                        .get::<tabs::CloseButton>(node)
                        .is_some()
                })
                .expect("a close button");
            *app.world().get::<Visibility>(close).unwrap()
        };
        let [(one, _), (two, _), ..] = tabs[..] else {
            panic!("two tabs");
        };
        settle(&mut app);
        assert_eq!(visibility(&app, one), Visibility::Hidden);

        tests::hover(&mut app, one, true);
        settle(&mut app);
        assert_eq!(visibility(&app, one), Visibility::Inherited);
        assert_eq!(visibility(&app, two), Visibility::Hidden);

        tests::hover(&mut app, one, false);
        settle(&mut app);
        assert_eq!(visibility(&app, one), Visibility::Hidden);
    }

    #[test]
    fn the_active_tab_is_marked_and_filled() {
        let (mut app, left, _) = app();
        let tab_ids = app
            .world()
            .resource::<DockTree>()
            .leaf(left)
            .unwrap()
            .windows
            .iter()
            .map(|tab| tab.id)
            .collect::<Vec<_>>();
        let node_of = |app: &mut App, id: TabId| {
            find::<DockTab>(app)
                .into_iter()
                .find(|(_, tab)| tab.tab == id)
                .map(|(entity, _)| entity)
                .unwrap()
        };
        let (one, two) = (
            node_of(&mut app, tab_ids[0]),
            node_of(&mut app, tab_ids[1]),
        );
        let marked = |app: &App, node: Entity| {
            app.world().get::<ActiveTab>(node).is_some()
        };
        let fill = |app: &App, node: Entity| {
            app.world()
                .get::<bevy::ui::BackgroundColor>(node)
                .unwrap()
                .0
        };
        assert!(marked(&app, one) && !marked(&app, two));
        // Lit from the first write, not from the next update.
        assert_eq!(fill(&app, one), Color::srgb(0.2, 0.2, 0.2));
        settle(&mut app);
        assert_eq!(fill(&app, one), Color::srgb(0.2, 0.2, 0.2));
        assert_eq!(fill(&app, two), Color::NONE);

        app.world_mut()
            .resource_mut::<DockTree>()
            .set_active(left, tab_ids[1]);
        settle(&mut app);

        assert!(!marked(&app, one) && marked(&app, two));
        assert_eq!(fill(&app, one), Color::NONE);
        assert_eq!(fill(&app, two), Color::srgb(0.2, 0.2, 0.2));
    }

    #[test]
    fn closing_the_last_tab_of_an_area_builds_the_layout_again() {
        let (mut app, left, right) = app();
        let root = find::<DockRoot>(&mut app)[0].0;
        let layout = kids(&app, root)[0];
        let before = kids(&app, layout);
        let tab = app
            .world()
            .resource::<DockTree>()
            .leaf(right)
            .unwrap()
            .windows[0]
            .id;

        app.world_mut().resource_mut::<DockTree>().remove_tab(tab);
        settle(&mut app);

        let after = kids(&app, layout);
        assert_eq!(after.len(), 1);
        assert_ne!(after, before, "a new layout without the split");
        assert!(
            find::<layout::DockArea>(&mut app)
                .iter()
                .all(|(_, area)| area.leaf == left)
        );
    }

    /// Opens the list of the "+" button in the area of `leaf`,
    /// returning the root the button and its list hang under.
    fn open_add(app: &mut App, leaf: NodeId) -> Entity {
        let button = find::<AddButton>(app)
            .into_iter()
            .map(|(entity, _)| entity)
            .find(|&button| {
                let mut at = button;
                loop {
                    let area =
                        app.world().get::<layout::DockArea>(at);
                    if let Some(area) = area {
                        return area.leaf == leaf;
                    }
                    match app.world().get::<ChildOf>(at) {
                        Some(parent) => at = parent.parent(),
                        None => return false,
                    }
                }
            })
            .unwrap();
        app.world_mut().trigger(MenuEvent {
            source: button,
            action: MenuAction::Toggle,
        });
        settle(app);
        app.world().get::<ChildOf>(button).unwrap().parent()
    }

    #[test]
    fn the_add_button_opens_a_list_of_what_is_not_open() {
        let (mut app, left, _) = app();
        let root = open_add(&mut app, left);

        assert!(app.world().get::<Open>(root).is_some());
        let rows = subtree(&app, root)
            .into_iter()
            .filter(|node| {
                app.world().get::<MenuItem>(*node).is_some()
            })
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            texts(&app, rows[0]),
            ["Extra"],
            "only what is closed"
        );
    }

    #[test]
    fn picking_a_row_adds_the_window_and_shuts_the_list() {
        let (mut app, left, _) = app();
        let root = open_add(&mut app, left);
        let row = subtree(&app, root)
            .into_iter()
            .find(|node| app.world().get::<MenuItem>(*node).is_some())
            .expect("a row");

        app.world_mut().trigger(Activate { entity: row });
        settle(&mut app);

        let tree = app.world().resource::<DockTree>();
        assert!(tree.leaf(left).unwrap().has_window("extra"));
        assert_eq!(
            tree.active(left),
            tree.leaf(left).unwrap().windows.last().map(|tab| tab.id)
        );
        assert!(app.world().get::<Open>(root).is_none());
    }

    use bevy::camera::NormalizedRenderTarget;
    use bevy::camera::visibility::Visibility;
    use bevy::input::ButtonInput;
    use bevy::input::keyboard::KeyCode;
    use bevy::math::{Rect, Vec2};
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Drag, DragEnd, DragStart, Pointer};
    use bevy::picking::pointer::{
        Location, PointerButton, PointerId,
    };
    use bevy::reflect::Reflect;
    use bevy::ui::{ComputedNode, UiGlobalTransform};

    fn pointer<E: core::fmt::Debug + Clone + Reflect>(
        node: Entity,
        at: Vec2,
        event: E,
    ) -> Pointer<E> {
        let target = NormalizedRenderTarget::None {
            width: 0,
            height: 0,
        };
        Pointer::new(
            PointerId::Mouse,
            Location {
                target,
                position: at,
            },
            event,
            node,
        )
    }

    fn press(app: &mut App, node: Entity, at: Vec2) {
        let hit = HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
        let event = DragStart {
            button: PointerButton::Primary,
            hit,
        };
        app.world_mut().trigger(pointer(node, at, event));
    }

    fn drag_to(app: &mut App, node: Entity, at: Vec2) {
        let event = Drag {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
            delta: Vec2::ZERO,
        };
        app.world_mut().trigger(pointer(node, at, event));
        app.update();
    }

    fn release(app: &mut App, node: Entity, at: Vec2) {
        let event = DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
        };
        app.world_mut().trigger(pointer(node, at, event));
        app.update();
    }

    fn lay(app: &mut App, node: Entity, rect: Rect) {
        app.world_mut().entity_mut(node).insert((
            ComputedNode {
                size: rect.size(),
                ..ComputedNode::default()
            },
            UiGlobalTransform::from_translation(rect.center()),
        ));
    }

    /// Lays the left area over `(0, 0)..(200, 200)` with its tabs
    /// 60px wide in a 20px bar, and the right one beside it.
    fn lay_out(app: &mut App, left: NodeId, right: NodeId) {
        for (leaf, x) in [(left, 0.0), (right, 200.0)] {
            let area = area_of(app, leaf);
            lay(app, area, Rect::new(x, 0.0, x + 200.0, 200.0));
            let bar = kids(app, area)[0];
            lay(app, bar, Rect::new(x, 0.0, x + 200.0, 20.0));
            let row = kids(app, bar)[0];
            for (index, tab) in kids(app, row).into_iter().enumerate()
            {
                let x = x + 62.0 * index as f32;
                lay(app, tab, Rect::new(x, 0.0, x + 60.0, 20.0));
            }
        }
    }

    fn tab_node(app: &mut App, leaf: NodeId, index: usize) -> Entity {
        let id = app
            .world()
            .resource::<DockTree>()
            .leaf(leaf)
            .unwrap()
            .windows[index]
            .id;
        find::<DockTab>(app)
            .into_iter()
            .find(|(_, tab)| tab.tab == id)
            .map(|(entity, _)| entity)
            .unwrap()
    }

    fn windows(app: &App, leaf: NodeId) -> Vec<String> {
        app.world()
            .resource::<DockTree>()
            .leaf(leaf)
            .unwrap()
            .windows
            .iter()
            .map(|tab| tab.window_id.clone())
            .collect()
    }

    fn dragging(app: &App) -> bool {
        matches!(
            app.world().resource::<DockDrag>(),
            DockDrag::Dragging { .. }
        )
    }

    #[test]
    fn dragging_the_handle_moves_the_split_and_builds_nothing() {
        let (mut app, _, _) = app();
        let root = find::<DockRoot>(&mut app)[0].0;
        let before = subtree(&app, root);
        let (handle, split) = find::<layout::SplitHandle>(&mut app)
            .into_iter()
            .map(|(entity, handle)| (entity, handle.split))
            .next()
            .unwrap();
        let [first, _, second] = kids(
            &app,
            app.world().get::<ChildOf>(handle).unwrap().parent(),
        )[..] else {
            panic!("two panes and a handle");
        };
        lay(&mut app, first, Rect::new(0.0, 0.0, 98.0, 100.0));
        lay(&mut app, second, Rect::new(102.0, 0.0, 200.0, 100.0));

        // A quarter of the 196px the panes share, past the handle's
        // own half, dragged by the wider strip that grabs it.
        let strip = kids(&app, handle)[0];
        drag_to(&mut app, strip, Vec2::new(51.0, 50.0));

        let fraction = app
            .world()
            .resource::<DockTree>()
            .get(split)
            .and_then(DockNode::as_split)
            .unwrap()
            .fraction;
        assert!((fraction - 0.25).abs() < 1e-4, "{fraction}");
        assert_eq!(subtree(&app, root), before, "same entities");
        assert!((percent_of(&app, first) - 25.0).abs() < 1e-3);
    }

    fn forced(app: &App) -> Option<bevy::window::SystemCursorIcon> {
        app.world().resource::<OverrideCursor>().0
    }

    #[test]
    fn a_handle_drag_holds_its_resize_cursor_until_it_ends() {
        let (mut app, _, _) = app();
        let handle = find::<layout::SplitHandle>(&mut app)[0].0;
        let resize =
            *app.world().get::<crate::EntityCursor>(handle).unwrap();

        press(&mut app, handle, Vec2::ZERO);
        assert_eq!(forced(&app), Some(resize.0));

        release(&mut app, handle, Vec2::ZERO);
        assert_eq!(forced(&app), None);
    }

    fn fill_of(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
    }

    #[test]
    fn a_handle_is_clear_until_hovered_or_dragged() {
        let (mut app, _, _) = app();
        let handle = find::<layout::SplitHandle>(&mut app)[0].0;
        let accent = Plain::default().accent();
        let settle = |app: &mut App| {
            for _ in 0..6 {
                app.update();
            }
        };
        settle(&mut app);
        assert_eq!(fill_of(&app, handle), Color::NONE);
        assert_eq!(
            width(&app, handle),
            px(Plain::default().divider()),
            "still to grab"
        );

        tests::hover(&mut app, handle, true);
        settle(&mut app);
        assert_eq!(fill_of(&app, handle), accent);
        tests::hover(&mut app, handle, false);
        settle(&mut app);
        assert_eq!(fill_of(&app, handle), Color::NONE);

        press(&mut app, handle, Vec2::ZERO);
        app.update();
        assert!(app.world().get::<Dragging>(handle).is_some());
        settle(&mut app);
        assert_eq!(fill_of(&app, handle), accent, "while dragged");

        release(&mut app, handle, Vec2::ZERO);
        assert!(app.world().get::<Dragging>(handle).is_none());
        settle(&mut app);
        assert_eq!(fill_of(&app, handle), Color::NONE);
    }

    #[test]
    fn the_buttons_draw_the_apps_icons_and_only_tint() {
        let (mut app, _, _) = app_with(Some(DockIcons {
            close: Some(Handle::default()),
            add: Some(Handle::default()),
        }));
        let marks = app
            .world_mut()
            .query_filtered::<Entity, With<ImageNode>>()
            .iter(app.world())
            .collect::<Vec<_>>();
        assert_eq!(marks.len(), 5, "three crosses and two pluses");
        for _ in 0..6 {
            app.update();
        }
        let all = find::<DockRoot>(&mut app)[0].0;
        let text = texts(&app, all);
        assert!(text.iter().all(|text| text != "x" && text != "+"));

        let tint = |app: &App, mark: Entity| {
            app.world().get::<ImageNode>(mark).unwrap().color
        };
        let dim = Plain::default().tone(Tone::Dim);
        let body = Plain::default().tone(Tone::Body);
        let hot = [Tone::Critical, Tone::Accent];
        let mut dims = 0;
        for mark in marks {
            let button =
                app.world().get::<ChildOf>(mark).unwrap().parent();
            // The cross of an active tab is lit with the rest of it,
            // and a plus rests in the body tone.
            let rest = tint(&app, mark);
            assert!(rest == dim || rest == body);
            dims += usize::from(rest == dim);
            tests::hover(&mut app, button, true);
            for _ in 0..6 {
                app.update();
            }
            assert_eq!(fill_of(&app, button), Color::NONE);
            let lit = tint(&app, mark);
            assert!(
                hot.iter()
                    .any(|&tone| lit == Plain::default().tone(tone))
            );
            tests::hover(&mut app, button, false);
            for _ in 0..6 {
                app.update();
            }
            assert_eq!(tint(&app, mark), rest);
        }
        assert_eq!(dims, 1, "the inactive cross");
    }

    #[test]
    fn a_tab_drag_holds_the_grabbing_cursor_until_it_ends() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        assert_eq!(forced(&app), None, "not yet a drag");
        drag_to(&mut app, one, Vec2::new(230.0, 10.0));
        assert_eq!(
            forced(&app),
            Some(bevy::window::SystemCursorIcon::Grabbing)
        );

        release(&mut app, one, Vec2::new(230.0, 10.0));
        assert_eq!(forced(&app), None);
    }

    #[test]
    fn cancelling_a_tab_drag_lets_go_of_the_cursor() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        app.init_resource::<ButtonInput<KeyCode>>();
        let one = tab_node(&mut app, left, 0);
        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(230.0, 10.0));

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert_eq!(forced(&app), None);
    }

    #[test]
    fn a_small_move_is_not_a_drag() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(12.0, 11.0));

        assert!(!dragging(&app));
        release(&mut app, one, Vec2::new(12.0, 11.0));
        assert!(matches!(
            app.world().resource::<DockDrag>(),
            DockDrag::Idle
        ));
        assert_eq!(windows(&app, left), ["one", "two"]);
    }

    #[test]
    fn dragging_a_tab_shows_a_ghost_and_a_hint_and_hides_the_tab() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(230.0, 10.0));
        drag_to(&mut app, one, Vec2::new(231.0, 10.0));

        let DockDrag::Dragging {
            ghost,
            hint,
            target,
            ..
        } = *app.world().resource::<DockDrag>()
        else {
            panic!("dragging");
        };
        assert_eq!(
            target,
            Some(DropTarget::Tabs {
                leaf: right,
                index: 1
            })
        );
        let ghost_ui = app.world().get::<Node>(ghost).unwrap();
        assert_eq!(ghost_ui.left, px(231.0 - 40.0));
        assert_eq!(texts(&app, ghost), ["One"]);
        let hint_ui = app.world().get::<Node>(hint.unwrap()).unwrap();
        assert_eq!(hint_ui.left, px(230.0));
        assert_eq!(hint_ui.width, px(30.0));
        assert_eq!(
            app.world().get::<Visibility>(one),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn dropping_on_a_bar_moves_the_tab_to_that_slot() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(210.0, 10.0));
        drag_to(&mut app, one, Vec2::new(215.0, 10.0));
        release(&mut app, one, Vec2::new(215.0, 10.0));
        settle(&mut app);

        assert_eq!(windows(&app, left), ["two"]);
        assert_eq!(windows(&app, right), ["one", "three"]);
        let right_area = area_of(&mut app, right);
        assert_eq!(contents(&app, right_area).len(), 2);
        assert!(matches!(
            app.world().resource::<DockDrag>(),
            DockDrag::Idle
        ));
        assert!(find::<DockArea>(&mut app).len() == 2, "no new area");
    }

    #[test]
    fn dropping_within_a_bar_reorders_the_tabs() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(115.0, 10.0));
        drag_to(&mut app, one, Vec2::new(118.0, 10.0));
        release(&mut app, one, Vec2::new(118.0, 10.0));
        settle(&mut app);

        assert_eq!(windows(&app, left), ["two", "one"]);
    }

    #[test]
    fn dropping_on_an_edge_splits_the_area_with_the_tab() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let two = tab_node(&mut app, left, 1);
        let before = app.world().resource::<DockTree>().shape();
        let id = app
            .world()
            .resource::<DockTree>()
            .leaf(left)
            .unwrap()
            .windows[1]
            .id;

        press(&mut app, two, Vec2::new(70.0, 10.0));
        drag_to(&mut app, two, Vec2::new(390.0, 100.0));
        drag_to(&mut app, two, Vec2::new(392.0, 100.0));
        release(&mut app, two, Vec2::new(392.0, 100.0));
        settle(&mut app);

        let tree = app.world().resource::<DockTree>();
        assert_ne!(tree.shape(), before);
        let new_leaf = tree.find_leaf_for_tab(id).unwrap();
        assert_ne!(new_leaf, left);
        assert_ne!(new_leaf, right);
        assert_eq!(tree.leaves().count(), 3);
        assert_eq!(windows(&app, left), ["one"]);
        assert_eq!(find::<DockArea>(&mut app).len(), 3);
    }

    #[test]
    fn dropping_in_the_middle_of_its_own_area_does_nothing() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(100.0, 100.0));
        drag_to(&mut app, one, Vec2::new(101.0, 100.0));

        let DockDrag::Dragging { hint, target, .. } =
            *app.world().resource::<DockDrag>()
        else {
            panic!("dragging");
        };
        assert_eq!((hint, target), (None, None));
        release(&mut app, one, Vec2::new(101.0, 100.0));
        assert_eq!(windows(&app, left), ["one", "two"]);
    }

    #[test]
    fn escape_ends_the_drag_without_dropping() {
        let (mut app, left, right) = app();
        lay_out(&mut app, left, right);
        app.init_resource::<ButtonInput<KeyCode>>();
        let one = tab_node(&mut app, left, 0);

        press(&mut app, one, Vec2::new(10.0, 10.0));
        drag_to(&mut app, one, Vec2::new(230.0, 10.0));
        drag_to(&mut app, one, Vec2::new(231.0, 10.0));
        let DockDrag::Dragging { ghost, hint, .. } =
            *app.world().resource::<DockDrag>()
        else {
            panic!("dragging");
        };
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        // Once more, for the commands the cancel queued.
        app.update();

        assert!(matches!(
            app.world().resource::<DockDrag>(),
            DockDrag::Idle
        ));
        assert!(app.world().get_entity(ghost).is_err());
        assert!(app.world().get_entity(hint.unwrap()).is_err());
        assert_eq!(
            app.world().get::<Visibility>(one),
            Some(&Visibility::Inherited)
        );
        release(&mut app, one, Vec2::new(231.0, 10.0));
        assert_eq!(windows(&app, left), ["one", "two"], "no drop");
    }
}
