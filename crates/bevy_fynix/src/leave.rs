//! Views animating in and out: the states a view's root is in on its
//! first frame and while it leaves, and the collapse of the space a
//! leaving view took.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::resource::Resource;
use bevy::ecs::world::World;
use bevy::picking::Pickable;
use bevy::ui::{
    ComputedNode, FlexDirection, Node, Overflow, UiRect, Val, px,
};

/// On a view's root for its first frame, so a `.when::<Entering, _>`
/// rule sets where it animates in from.
///
/// Only views with a rule waiting on it get it: the rule asks for it
/// when it is built.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Entering;

/// On the root of a view a `keyed` or `each` dropped, while it
/// animates out. Only a view whose root element has a transition
/// does: others are despawned at once.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Leaving;

/// The nodes that took [`Entering`] since the last update, to lose it
/// after the update writes them once.
#[derive(Resource, Default, Debug)]
pub struct Entrances(pub Vec<Entity>);

/// Gives `node` [`Entering`] until the end of the next update.
pub(crate) fn enter(world: &mut World, node: Entity) {
    if let Ok(mut entity) = world.get_entity_mut(node) {
        entity.insert(Entering);
        world.resource_mut::<Entrances>().0.push(node);
    }
}

/// Takes [`Entering`] off every node that has been written with it.
/// The removal marks them dirty, so they animate in from the next
/// update.
pub(crate) fn settle_entrances(world: &mut World) {
    let entered =
        core::mem::take(&mut world.resource_mut::<Entrances>().0);
    for node in entered {
        if let Ok(mut entity) = world.get_entity_mut(node) {
            entity.remove::<Entering>();
        }
    }
}

/// Puts `node` in [`Leaving`], and stops it and every node under it
/// from being picked.
pub(crate) fn leave(world: &mut World, node: Entity) {
    let Ok(mut entity) = world.get_entity_mut(node) else {
        return;
    };
    entity.insert(Leaving);
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        let Ok(mut entity) = world.get_entity_mut(node) else {
            continue;
        };
        entity.insert(Pickable::IGNORE);
        if let Some(children) = entity.get::<Children>() {
            stack.extend(children.iter());
        }
    }
}

/// On a node whose space is collapsing: its size along its parent's
/// main axis when the collapse began, and the gap the parent keeps
/// beside it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Collapsing {
    size: f32,
    gap: f32,
    /// Whether the parent lays its children out in a row.
    row: bool,
    /// Whether the node is its parent's first child, so the gap to
    /// take back is after it, not before.
    first: bool,
}

fn px_of(val: Val) -> f32 {
    match val {
        Val::Px(px) => px,
        _ => 0.0,
    }
}

/// How `node` sits in its parent, measured as it is laid out now.
fn measure(world: &World, node: Entity) -> Collapsing {
    let entity = world.entity(node);
    let size = entity
        .get::<ComputedNode>()
        .map(|computed| {
            computed.size() * computed.inverse_scale_factor()
        })
        .unwrap_or_default();
    let parent = entity.get::<ChildOf>().map(ChildOf::parent);
    let layout = parent.and_then(|parent| world.get::<Node>(parent));
    let row = layout.is_some_and(|layout| {
        matches!(
            layout.flex_direction,
            FlexDirection::Row | FlexDirection::RowReverse
        )
    });
    let gap = layout.map_or(0.0, |layout| {
        px_of(if row {
            layout.column_gap
        } else {
            layout.row_gap
        })
    });
    let siblings =
        parent.and_then(|parent| world.get::<Children>(parent));
    let first = siblings
        .is_some_and(|children| children.first() == Some(&node));
    let alone = siblings.is_none_or(|children| children.len() < 2);
    Collapsing {
        size: if row { size.x } else { size.y },
        gap: if alone { 0.0 } else { gap },
        row,
        first,
    }
}

/// Shrinks the space `node` takes along its parent's main axis,
/// `progress` of the way to nothing, the parent's gap beside it
/// included. The first call measures it and clips it.
pub(crate) fn collapse(
    world: &mut World,
    node: Entity,
    progress: f32,
) {
    if world.get_entity(node).is_err() {
        return;
    }
    let collapsing = match world.get::<Collapsing>(node) {
        Some(collapsing) => *collapsing,
        None => {
            let collapsing = measure(world, node);
            world.entity_mut(node).insert(collapsing);
            collapsing
        }
    };
    let Some(mut ui) = world.get_mut::<Node>(node) else {
        return;
    };
    let size = px(collapsing.size * (1.0 - progress));
    let margin = px(-collapsing.gap * progress);
    ui.overflow = Overflow::clip();
    ui.flex_shrink = 0.0;
    ui.padding = UiRect::ZERO;
    if collapsing.row {
        ui.width = size;
        ui.min_width = px(0.0);
        if collapsing.first {
            ui.margin.right = margin;
        } else {
            ui.margin.left = margin;
        }
    } else {
        ui.height = size;
        ui.min_height = px(0.0);
        if collapsing.first {
            ui.margin.bottom = margin;
        } else {
            ui.margin.top = margin;
        }
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::color::{Alpha, Color};
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::text::TextColor;
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use fynix::{Curve, Motion, MotionTokens, ScopedExt};

    use super::*;
    use crate::tokens::{SpacingTokens, TextTokens, Tone};
    use crate::views::{Label, column, label};
    use crate::{
        Bevy, FynixPlugin, Theme, ViewExt, each, mount, resource,
    };

    struct Test;

    impl TextTokens for Test {
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

    impl SpacingTokens for Test {
        fn gap(&self) -> f32 {
            4.0
        }

        fn row(&self) -> f32 {
            20.0
        }

        fn radius(&self) -> f32 {
            0.0
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

    #[derive(Resource)]
    struct Ids(Vec<u32>);

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((TimePlugin, FynixPlugin::<Test>::default()))
            .insert_resource(Theme(Test))
            .insert_resource(Ids(vec![1, 2]))
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ));
        // The first update only starts the clock.
        app.update();
        app
    }

    fn alpha(app: &App, node: Entity) -> f32 {
        app.world()
            .get::<TextColor>(node)
            .expect("a label")
            .0
            .alpha()
    }

    fn faded(label: Label, _: &Test) -> Label {
        label.opacity(0.0)
    }

    /// A row that fades in and out.
    fn row(id: &u32) -> fynix::AnyView<Bevy, Test> {
        label(id.to_string())
            .when::<Entering, _>(faded)
            .when::<Leaving, _>(faded)
            .transition(Motion::Interact)
            .boxed()
    }

    fn list(app: &mut App) -> Entity {
        mount::<Test>(
            app.world_mut(),
            column((each(
                resource::<Ids, _>(|ids| ids.0.clone()),
                |id| *id,
                row,
            ),)),
        )
    }

    fn rows(app: &App, root: Entity) -> Vec<Entity> {
        let container = app.world().get::<Children>(root).unwrap()[0];
        app.world()
            .get::<Children>(container)
            .map(|children| children.iter().collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_view_starts_in_its_entering_state_and_animates_out_of_it() {
        let mut app = app();
        let node = mount::<Test>(
            app.world_mut(),
            label("x")
                .when::<Entering, _>(faded)
                .transition(Motion::Interact),
        );
        assert_eq!(alpha(&app, node), 0.0, "written entering");

        app.update();
        assert!(app.world().get::<Entering>(node).is_none());
        assert_eq!(alpha(&app, node), 0.0, "lets go after one write");

        app.update();
        assert_eq!(alpha(&app, node), 0.5, "50ms of 100ms");
        app.update();
        assert_eq!(alpha(&app, node), 1.0);
    }

    #[test]
    fn a_view_without_an_entering_rule_never_gets_the_state() {
        let mut app = app();
        let node = mount::<Test>(app.world_mut(), label("x"));

        assert!(app.world().get::<Entering>(node).is_none());
        assert!(app.world().resource::<Entrances>().0.is_empty());
    }

    #[test]
    fn a_dropped_row_fades_collapses_and_goes() {
        let mut app = app();
        let root = list(&mut app);
        // Past the rows' own entrance.
        for _ in 0..3 {
            app.update();
        }
        let two = rows(&app, root)[1];
        assert_eq!(alpha(&app, two), 1.0);

        app.world_mut().resource_mut::<Ids>().0 = vec![1];
        app.update();
        assert!(app.world().get::<Leaving>(two).is_some());
        assert_eq!(
            app.world().get::<Pickable>(two),
            Some(&Pickable::IGNORE)
        );

        app.update();
        assert_eq!(alpha(&app, two), 0.5, "fading");
        app.update();
        assert_eq!(alpha(&app, two), 0.0);
        assert!(
            app.world().get::<Collapsing>(two).is_some(),
            "then collapsing"
        );

        app.update();
        app.update();
        assert!(app.world().get_entity(two).is_err());
        assert_eq!(rows(&app, root).len(), 1);
    }

    #[test]
    fn a_collapse_takes_the_gap_before_the_node_back() {
        let mut app = app();
        let root = list(&mut app);
        let two = rows(&app, root)[1];
        let container = app.world().get::<Children>(root).unwrap()[0];
        let mut layout =
            app.world_mut().get_mut::<Node>(container).unwrap();
        layout.flex_direction = FlexDirection::Column;
        layout.row_gap = px(10.0);

        collapse(app.world_mut(), two, 0.5);

        let ui = app.world().get::<Node>(two).unwrap();
        assert_eq!(
            ui.height,
            px(0.0),
            "nothing measured without layout"
        );
        assert_eq!(ui.margin.top, px(-5.0));
        assert_eq!(ui.overflow, Overflow::clip());
    }
}
