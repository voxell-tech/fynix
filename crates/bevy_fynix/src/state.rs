//! Interaction states as components on a node, and the state rules
//! that wait on them.

use core::any::TypeId;
use core::marker::PhantomData;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::lifecycle::{Insert, Remove};
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::ecs::world::World;
use bevy::picking::events::{Pointer, Press, Release};
use bevy::picking::hover::Hovered as PickingHovered;
use fynix::{Condition, ScopedExt, When};

use crate::leave::Entering;
use crate::{Bevy, Cx, Styled, View};

/// The pointer is over the node or one of its descendants.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Hovered;

/// A pointer button is down on the node.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Pressed;

/// The copy of Bevy's own hover component onto [`Hovered`]. Bevy
/// counts a hovered descendant as hovering its ancestors, so moving
/// between a node and its children never lets go of it.
fn sync_hover(
    insert: On<Insert, PickingHovered>,
    picked: Query<&PickingHovered>,
    mut commands: Commands,
) {
    let node = insert.event_target();
    let Ok(picked) = picked.get(node) else {
        return;
    };
    if picked.get() {
        commands.entity(node).insert(Hovered);
    } else {
        commands.entity(node).remove::<(Hovered, Pressed)>();
    }
}

fn press(press: On<Pointer<Press>>, mut commands: Commands) {
    commands.entity(press.event_target()).insert(Pressed);
}

fn release(release: On<Pointer<Release>>, mut commands: Commands) {
    commands.entity(release.event_target()).remove::<Pressed>();
}

/// The nodes whose state components were inserted or removed since
/// the last update.
#[derive(Resource, Default, Debug)]
pub struct DirtyNodes(pub Vec<Entity>);

fn state_inserted<S: Component>(
    insert: On<Insert, S>,
    mut dirty: ResMut<DirtyNodes>,
) {
    dirty.0.push(insert.event_target());
}

fn state_removed<S: Component>(
    remove: On<Remove, S>,
    mut dirty: ResMut<DirtyNodes>,
) {
    dirty.0.push(remove.event_target());
}

/// On a node whose state `S` is already reported to [`DirtyNodes`].
#[derive(Component)]
struct Watched<S: Component>(PhantomData<fn() -> S>);

/// Queues the node whenever the state component `S` is inserted on
/// or removed from it, once however many rules read it. The states
/// this crate sets itself are kept up to date from picking too.
fn watch_state<S: Component>(world: &mut World, node: Entity) {
    let Ok(mut entity) = world.get_entity_mut(node) else {
        return;
    };
    if entity.contains::<Watched<S>>() {
        return;
    }
    entity
        .insert(Watched::<S>(PhantomData))
        .observe(state_inserted::<S>)
        .observe(state_removed::<S>);
    if TypeId::of::<S>() == TypeId::of::<Entering>() {
        crate::leave::enter(world, node);
        return;
    }
    let Ok(mut entity) = world.get_entity_mut(node) else {
        return;
    };
    let tracked = [TypeId::of::<Hovered>(), TypeId::of::<Pressed>()];
    if tracked.contains(&TypeId::of::<S>())
        && !entity.contains::<PickingHovered>()
    {
        entity
            .insert(PickingHovered::default())
            .observe(sync_hover)
            .observe(press)
            .observe(release);
    }
}

/// The state of a node holding the component `S`, for rules to wait
/// on. See [`StateExt::when`].
pub struct State<S>(PhantomData<fn() -> S>);

impl<S: Component> Condition<Bevy> for State<S> {
    fn holds(world: &World, node: Entity) -> bool {
        world
            .get_entity(node)
            .is_ok_and(|entity| entity.contains::<S>())
    }

    fn watch(world: &mut World, node: Entity) {
        watch_state::<S>(world, node);
    }
}

/// What any view can be given state rules with.
pub trait StateExt: Sized {
    /// This view, with the rules `rules` sets holding while its root
    /// node holds the component `S`: on the root they beat its call
    /// site, and on a view under it they fill what its call site left
    /// unset. See [`Cx::when`].
    ///
    /// ```ignore
    /// button(row((icon(save), label("Save"))))
    ///     .when::<Hovered, _>(|cx: &mut Cx<Bevy, Theme>| {
    ///         cx.set::<Label>(|l, _| l.tone(Tone::Accent));
    ///     })
    /// ```
    fn when<S: Component, F>(
        self,
        rules: F,
    ) -> When<Self, F, State<S>> {
        self.when_in::<State<S>, F>(rules)
    }

    /// This view, animating in from what `away` sets and out to it
    /// again: `away` holds while it is [`Entering`] and while it is
    /// [`Leaving`](crate::Leaving). It needs a transition to move.
    ///
    /// ```ignore
    /// line(name).appear(hidden).transition(Motion::Expand)
    /// ```
    #[allow(clippy::type_complexity)]
    fn appear<T: 'static>(
        self,
        away: fn(&mut Cx<'_, Bevy, T>),
    ) -> When<
        When<Self, fn(&mut Cx<'_, Bevy, T>), State<Entering>>,
        fn(&mut Cx<'_, Bevy, T>),
        State<crate::Leaving>,
    > {
        self.when::<Entering, _>(away)
            .when::<crate::Leaving, _>(away)
    }
}

impl<V> StateExt for V {}

/// Where a view is while out of sight, for [`StateExt::appear`]:
/// every element in it transparent, and its root scaled down a
/// little. Scale is only set on the root, as children take their
/// parent's.
pub fn hidden<T: 'static>(cx: &mut Cx<'_, Bevy, T>) {
    cx.set::<crate::Visual>(|visual, _| visual.opacity(0.0));
    cx.root(|cx| {
        cx.set::<crate::Visual>(|visual, _| visual.scale(0.9))
    });
}

/// A rule an element writes on itself, as the block of a
/// [`StateExt::when`].
pub fn own<V: Styled, T: 'static>(
    rule: impl Fn(V, &T) -> V + Send + Sync + 'static,
) -> impl FnOnce(&mut Cx<'_, Bevy, T>) {
    move |cx| cx.set::<V>(rule)
}

/// An element `E` built as `view`, with `rule` restyling it while its
/// node holds the component `S`, under the theme `T`. See the `when`
/// of [`Label`](crate::views::Label), [`Frame`](crate::views::Frame)
/// and [`Icon`](crate::views::Icon).
///
/// It has a `when` of its own, taking a rule for `E` again, so rules
/// chain: `label(x).when::<A, _>(a).when::<B, _>(b)`. The one written
/// later wins while both hold.
pub struct OwnWhen<E, V, S, R, T> {
    view: V,
    rule: R,
    marker: PhantomData<fn(E, &T) -> S>,
}

impl<E, V, S, R, T> OwnWhen<E, V, S, R, T> {
    pub(crate) fn new(view: V, rule: R) -> Self {
        Self {
            view,
            rule,
            marker: PhantomData,
        }
    }

    /// This, with `rule` restyling the element too while its node
    /// holds the component `S2`.
    pub fn when<S2, R2>(self, rule: R2) -> OwnWhen<E, Self, S2, R2, T>
    where
        R2: Fn(E, &T) -> E + Send + Sync + 'static,
    {
        OwnWhen::new(self, rule)
    }
}

impl<E, V, S, R, T> View<Bevy, T> for OwnWhen<E, V, S, R, T>
where
    E: Styled,
    V: View<Bevy, T>,
    S: Component,
    R: Fn(E, &T) -> E + Send + Sync + 'static,
    T: 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        cx.scope(|cx| {
            cx.when::<State<S>>(|cx| cx.set::<E>(self.rule));
            self.view.build(cx)
        })
    }
}

/// An inherent `when` for an element, taking one rule for the element
/// itself instead of a block.
macro_rules! own_when {
    ($element:ty) => {
        impl $element {
            /// This element, with `rule` restyling it while its node
            /// holds the component `S`. It beats the call site.
            pub fn when<S, T>(
                self,
                rule: impl Fn(Self, &T) -> Self + Send + Sync + 'static,
            ) -> $crate::state::OwnWhen<
                Self,
                Self,
                S,
                impl Fn(Self, &T) -> Self + Send + Sync + 'static,
                T,
            > {
                $crate::state::OwnWhen::new(self, rule)
            }
        }
    };
}

pub(crate) use own_when;

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::text::{FontSize, TextColor, TextFont};
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use fynix::{Curve, Motion, MotionTokens, ScopedExt};
    use motiongfx_interp::interpolation::Interpolation;

    use super::*;
    use crate::mounted::Mounts;
    use crate::tokens::{TextTokens, Tone};
    use crate::transition::{BevyMarker, ReducedMotion};
    use crate::views::{Label, label, row};
    use crate::{
        AnyView, FynixPlugin, Theme, derived, every_frame, mount,
        resource,
    };

    struct Test;

    impl TextTokens for Test {
        fn tone(&self, tone: Tone) -> Color {
            match tone {
                Tone::Body => Color::BLACK,
                Tone::Dim | Tone::Faint => DIM,
                Tone::Accent | Tone::Critical => Color::WHITE,
            }
        }

        fn body_size(&self) -> f32 {
            14.0
        }

        fn small_size(&self) -> f32 {
            11.0
        }
    }

    impl MotionTokens for Test {
        fn motion(&self, _: Motion) -> Curve {
            Curve {
                duration: Duration::from_millis(100),
                ease: |t| t,
            }
        }
    }

    impl crate::tokens::SpacingTokens for Test {
        fn gap(&self) -> f32 {
            0.0
        }

        fn row(&self) -> f32 {
            20.0
        }

        fn radius(&self) -> f32 {
            0.0
        }
    }

    const BLACK: Color = Color::BLACK;
    const DIM: Color = Color::srgb(0.2, 0.2, 0.2);

    /// Halfway from black to white.
    fn halfway() -> Color {
        blend(&BLACK, &Color::WHITE, 0.5)
    }

    fn blend(from: &Color, to: &Color, t: f32) -> Color {
        <Color as Interpolation<BevyMarker>>::interp(from, to, t)
    }

    #[derive(Component)]
    struct Selected;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((TimePlugin, FynixPlugin::<Test>::default()))
            .insert_resource(Theme(Test))
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ));
        // The first update only starts the clock.
        app.update();
        app
    }

    fn color(app: &App, node: Entity) -> Color {
        app.world().get::<TextColor>(node).expect("a label").0
    }

    fn accent(label: Label, _: &Test) -> Label {
        label.tone(Tone::Accent)
    }

    fn hover(app: &mut App, node: Entity, on: bool) {
        let mut node = app.world_mut().entity_mut(node);
        if on {
            node.insert(Hovered);
        } else {
            node.remove::<Hovered>();
        }
    }

    fn children(app: &App, node: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(node)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_state_rule_applies_while_its_state_holds() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x").when::<Hovered, _>(accent),
        );
        assert_eq!(color(&app, node), BLACK);

        hover(&mut app, node, true);
        app.update();
        assert_eq!(color(&app, node), Color::WHITE);

        hover(&mut app, node, false);
        app.update();
        assert_eq!(color(&app, node), BLACK);
    }

    #[test]
    fn an_app_can_use_its_own_state_component() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x").when::<Selected, _>(accent),
        );

        app.world_mut().entity_mut(node).insert(Selected);
        app.update();

        assert_eq!(color(&app, node), Color::WHITE);
    }

    #[test]
    fn the_later_rule_wins_while_both_hold() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x").when::<Hovered, _>(accent).when::<Pressed, _>(
                |l: Label, _: &Test| l.tone(Tone::Dim),
            ),
        );

        hover(&mut app, node, true);
        app.update();
        assert_eq!(color(&app, node), Color::WHITE);

        app.world_mut().entity_mut(node).insert(Pressed);
        app.update();
        assert_eq!(color(&app, node), DIM, "the one written later");
    }

    #[test]
    fn a_state_rule_beats_the_call_site_and_a_binding() {
        let mut app = app();
        let fixed = mount::<Test>(
            app.world_mut(),
            label("x").tone(Tone::Dim).when::<Hovered, _>(accent),
        );
        let bound = mount::<Test>(
            app.world_mut(),
            label("x")
                .tone(derived(|_| Tone::Dim).when(every_frame()))
                .when::<Hovered, _>(accent),
        );
        hover(&mut app, fixed, true);
        hover(&mut app, bound, true);
        app.update();

        assert_eq!(color(&app, fixed), Color::WHITE);
        assert_eq!(color(&app, bound), Color::WHITE);

        hover(&mut app, bound, false);
        app.update();
        assert_eq!(
            color(&app, bound),
            DIM,
            "and the binding is back"
        );
    }

    #[test]
    fn a_state_rule_on_a_composite_reaches_the_labels_in_it() {
        let mut app = app();
        let root = mount::<Test>(
            app.world_mut(),
            row((label("lit"), label("kept").tone(Tone::Dim)))
                .when::<Hovered, _>(|cx: &mut Cx<Bevy, Test>| {
                    cx.set::<Label>(accent);
                }),
        );
        let [lit, kept] = children(&app, root)[..] else {
            panic!("two labels");
        };

        hover(&mut app, root, true);
        app.update();

        assert_eq!(color(&app, lit), Color::WHITE);
        assert_eq!(color(&app, kept), DIM, "its call site opts out");

        hover(&mut app, root, false);
        app.update();
        assert_eq!(color(&app, lit), BLACK);
    }

    #[test]
    fn a_label_under_the_hovered_node_does_not_need_its_own_hover() {
        let mut app = app();
        let root = mount::<Test>(
            app.world_mut(),
            row((label("x"),)).when::<Hovered, _>(
                |cx: &mut Cx<Bevy, Test>| cx.set::<Label>(accent),
            ),
        );
        let child = children(&app, root)[0];

        hover(&mut app, child, true);
        app.update();

        assert_eq!(color(&app, child), BLACK, "read on the row");
    }

    #[test]
    fn an_element_with_a_state_rule_stays_mounted() {
        let mut app = app();
        mount::<Test>(
            app.world_mut(),
            label("x").when::<Hovered, _>(accent),
        );
        mount::<Test>(app.world_mut(), label("y"));

        assert_eq!(app.world().resource::<Mounts<Test>>().len(), 1);
    }

    #[test]
    fn a_set_rule_still_fills_under_a_state_rule() {
        let mut app = app();
        let root = mount::<Test>(
            app.world_mut(),
            AnyView::<Bevy, Test>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.set::<Label>(|l, _| l.size(20.0));
                    cx.build(label("x").when::<Hovered, _>(accent));
                });
                root
            }),
        );
        let node = children(&app, root)[0];
        hover(&mut app, node, true);
        app.update();

        assert_eq!(color(&app, node), Color::WHITE);
        let size =
            app.world().get::<TextFont>(node).unwrap().font_size;
        assert_eq!(size, FontSize::Px(20.0));
    }

    #[test]
    fn nothing_animates_unless_asked() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x").when::<Hovered, _>(accent),
        );

        hover(&mut app, node, true);
        app.update();

        assert_eq!(color(&app, node), Color::WHITE);
    }

    #[test]
    fn a_transition_reaches_its_target_over_the_curve() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(accent)
                .transition(Motion::Interact),
        );

        hover(&mut app, node, true);
        app.update();
        assert_eq!(color(&app, node), halfway(), "50ms of 100ms");

        app.update();
        assert_eq!(color(&app, node), Color::WHITE);

        app.update();
        assert_eq!(color(&app, node), Color::WHITE, "and stays");
    }

    #[test]
    fn a_transition_follows_a_bound_prop() {
        let mut app = app();
        app.insert_resource(Selection(Tone::Body));
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .tone(resource::<Selection, _>(|selection| {
                    selection.0
                }))
                .transition(Motion::Interact),
        );

        app.world_mut().resource_mut::<Selection>().0 = Tone::Accent;
        app.update();

        assert_eq!(color(&app, node), halfway());
    }

    #[derive(Resource)]
    struct Selection(Tone);

    #[test]
    fn an_interrupted_transition_starts_from_where_it_got_to() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(accent)
                .transition(Motion::Interact),
        );

        hover(&mut app, node, true);
        app.update();
        assert_eq!(color(&app, node), halfway());

        hover(&mut app, node, false);
        app.update();
        assert_eq!(
            color(&app, node),
            blend(&halfway(), &BLACK, 0.5),
            "halfway back to black from where it was, not from white"
        );

        app.update();
        assert_eq!(color(&app, node), BLACK);
    }

    #[test]
    fn a_transition_rule_in_a_scope_reaches_the_elements_in_it() {
        let mut app = app();
        let root = mount::<Test>(
            app.world_mut(),
            AnyView::<Bevy, Test>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.transition(Motion::Interact);
                    cx.build(label("x").when::<Hovered, _>(accent));
                });
                root
            }),
        );
        let node = children(&app, root)[0];

        hover(&mut app, node, true);
        app.update();

        assert_eq!(color(&app, node), halfway());
    }

    #[test]
    fn reduced_motion_snaps() {
        let mut app = app();
        app.insert_resource(ReducedMotion(true));
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(accent)
                .transition(Motion::Interact),
        );

        hover(&mut app, node, true);
        app.update();

        assert_eq!(color(&app, node), Color::WHITE);
    }

    #[test]
    fn reduced_motion_finishes_a_transition_under_way() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(accent)
                .transition(Motion::Interact),
        );
        hover(&mut app, node, true);
        app.update();
        assert_eq!(color(&app, node), halfway());

        app.insert_resource(ReducedMotion(true));
        app.update();

        assert_eq!(color(&app, node), Color::WHITE);
    }

    #[test]
    fn inserting_and_removing_a_state_re_applies_its_rule() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x").when::<Selected, _>(accent),
        );

        app.world_mut().entity_mut(node).insert(Selected);
        assert_eq!(
            app.world().resource::<DirtyNodes>().0,
            vec![node],
            "the insert observer queued the node"
        );
        app.update();
        assert_eq!(color(&app, node), Color::WHITE);
        assert!(app.world().resource::<DirtyNodes>().0.is_empty());

        app.world_mut().entity_mut(node).remove::<Selected>();
        assert_eq!(
            app.world().resource::<DirtyNodes>().0,
            vec![node]
        );
        app.update();
        assert_eq!(color(&app, node), BLACK);
    }

    #[test]
    fn a_node_read_by_many_rules_is_queued_once() {
        let mut app = app();
        let root = mount::<Test>(
            app.world_mut(),
            row((label("a"), label("b"))).when::<Selected, _>(
                |cx: &mut Cx<Bevy, Test>| cx.set::<Label>(accent),
            ),
        );

        app.world_mut().entity_mut(root).insert(Selected);

        assert_eq!(
            app.world().resource::<DirtyNodes>().0,
            vec![root]
        );
    }

    #[test]
    fn a_state_the_element_has_no_rule_for_does_not_queue_it() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x").when::<Hovered, _>(accent),
        );

        app.world_mut().entity_mut(node).insert(Selected);

        assert!(app.world().resource::<DirtyNodes>().0.is_empty());
    }

    #[test]
    fn a_despawned_element_with_a_state_rule_is_dropped() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(accent)
                .transition(Motion::Interact),
        );
        assert_eq!(app.world().resource::<Mounts<Test>>().len(), 1);

        app.world_mut().despawn(node);
        app.update();

        assert!(app.world().resource::<Mounts<Test>>().is_empty());
    }
}
