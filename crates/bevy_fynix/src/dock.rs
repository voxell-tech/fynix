//! Docking: split panes with tabbed areas, shown from a
//! [`DockTree`].
//!
//! The layout is built again only when the tree's [`Shape`] changes.
//! Everything that changes more often is a bound prop: the share of a
//! split, the active tab, and the tabs of one area.

mod layout;
mod popup;
mod registry;
mod tabs;
mod tree;

use core::marker::PhantomData;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::schedule::common_conditions::resource_changed;
use bevy::math::Rect;
use bevy::ui::{ComputedNode, UiGlobalTransform, percent};
pub use layout::DockArea;
pub use popup::{AddPopup, OpenPopup};
pub use registry::{DockRegistry, DockWindowKind};
pub use tabs::{ActiveTab, DockTab};
pub use tree::{
    DockAreaStyle, DockLeaf, DockNode, DockSplit, DockTabEntry,
    DockTree, Edge, NodeId, Shape, SplitAxis, TabId,
};

use crate::prop::{keyed, resource};
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
};
use crate::views::{BehaviorExt, FrameProps, column, overlay};
use crate::{AnyView, Bevy, ViewExt};

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
            .init_resource::<AddPopup>()
            .add_observer(layout::drag_handle)
            .add_observer(popup::open)
            .add_observer(popup::dismiss)
            .add_systems(
                Update,
                tabs::mark_active
                    .run_if(resource_changed::<DockTree>)
                    .before(crate::mounted::update::<T>),
            );
    }
}

/// On the root node of a [`dock`].
#[derive(Component, Clone, Copy, Debug)]
pub struct DockRoot;

/// The dock shown from the [`DockTree`] resource: split panes, each
/// leaf a tab bar over the content of its active tab.
pub fn dock<T: DockTokens>() -> AnyView<Bevy, T> {
    let layout =
        keyed(resource::<DockTree, _>(DockTree::shape), |shape| {
            layout::build(shape)
        })
        .within(
            column(())
                .gap(0.0)
                .width(percent(100.0))
                .height(percent(100.0)),
        );
    let popup = keyed(
        resource::<AddPopup, _>(|popup| popup.open.clone()),
        |open| popup::build(open),
    )
    .within(overlay(()));
    column((layout, popup))
        .gap(0.0)
        .width(percent(100.0))
        .height(percent(100.0))
        .tagged(DockRoot)
        .boxed()
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
    use core::time::Duration;

    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::entity::Entity;
    use bevy::ecs::hierarchy::{ChildOf, Children};
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use bevy::ui::widget::Text;
    use bevy::ui::{Display, FlexDirection, Node, Val, px};
    use bevy::ui_widgets::Activate;

    use super::tabs::AddButton;
    use super::*;
    use crate::tokens::{Curve, Motion, Tone};
    use crate::views::label;
    use crate::{FynixPlugin, Theme, mount};

    struct Plain;

    impl TextTokens for Plain {
        fn tone(&self, _: Tone) -> Color {
            Color::WHITE
        }

        fn body_size(&self) -> f32 {
            14.0
        }

        fn small_size(&self) -> f32 {
            11.0
        }
    }

    impl SurfaceTokens for Plain {
        fn fill(&self) -> Color {
            Color::srgb(0.2, 0.2, 0.2)
        }

        fn hover(&self) -> Color {
            Color::srgb(0.3, 0.3, 0.3)
        }

        fn panel(&self) -> Color {
            Color::BLACK
        }
    }

    impl SpacingTokens for Plain {
        fn gap(&self) -> f32 {
            6.0
        }

        fn row(&self) -> f32 {
            20.0
        }

        fn radius(&self) -> f32 {
            3.0
        }
    }

    impl MotionTokens for Plain {
        fn motion(&self, _: crate::tokens::Motion) -> Curve {
            let _ = Motion::Interact;
            Curve {
                duration: Duration::from_millis(100),
                ease: |t| t,
            }
        }
    }

    fn window(
        name: &'static str,
        text: &'static str,
    ) -> DockWindowKind<Plain> {
        DockWindowKind::new(name, move || label(text).boxed())
    }

    /// A tree of the areas `left` and `right` side by side, and a
    /// registry of the windows in them and one more, `extra`.
    fn app() -> (App, NodeId, NodeId) {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
            DockPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain))
        .insert_resource(
            TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ),
        );
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

    fn kids(app: &App, node: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(node)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
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

        let [layout, popup] = kids(&app, root)[..] else {
            panic!("the layout and the popup");
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
        assert_eq!(width(&app, handle), px(6.0));
        assert!(
            app.world().get::<layout::SplitHandle>(handle).is_some()
        );
        assert_eq!(kids(&app, popup).len(), 1, "an empty popup");

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

    #[test]
    fn the_add_button_opens_a_popup_listing_what_is_not_open() {
        let (mut app, left, _) = app();
        let button = find::<AddButton>(&mut app)
            .into_iter()
            .find(|(_, add)| add.leaf == left)
            .map(|(entity, _)| entity)
            .unwrap();

        app.world_mut().trigger(Activate { entity: button });
        app.update();

        assert_eq!(
            app.world()
                .resource::<AddPopup>()
                .open
                .as_ref()
                .map(|open| open.leaf),
            Some(left)
        );
        let root = find::<DockRoot>(&mut app)[0].0;
        let popup = kids(&app, root)[1];
        assert_eq!(
            texts(&app, popup),
            ["Extra"],
            "only what is closed"
        );
    }

    #[test]
    fn picking_a_row_adds_the_window_and_closes_the_popup() {
        let (mut app, left, _) = app();
        app.world_mut().resource_mut::<AddPopup>().open =
            Some(OpenPopup {
                leaf: left,
                left: 0.0,
                top: 0.0,
            });
        app.update();
        let root = find::<DockRoot>(&mut app)[0].0;
        let popup = kids(&app, root)[1];
        let row = subtree(&app, popup)
            .into_iter()
            .find(|node| {
                app.world()
                    .get::<bevy::ui_widgets::Button>(*node)
                    .is_some()
            })
            .expect("a row");

        app.world_mut().trigger(Activate { entity: row });
        settle(&mut app);

        let tree = app.world().resource::<DockTree>();
        assert!(tree.leaf(left).unwrap().has_window("extra"));
        assert_eq!(
            tree.active(left),
            tree.leaf(left).unwrap().windows.last().map(|tab| tab.id)
        );
        assert!(app.world().resource::<AddPopup>().open.is_none());
        assert_eq!(kids(&app, popup).len(), 1);
        assert!(texts(&app, popup).is_empty(), "the popup is gone");
    }
}
