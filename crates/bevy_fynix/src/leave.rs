//! Views animating in and out: the states a view's root is in on its
//! first frame, while it comes in and while it leaves, and the
//! collapse and expansion of the space it takes.
//!
//! A view a `keyed` or `each` builds after its first build comes in
//! in two steps. It is held out of the layout and out of sight until
//! its natural size has been measured, then its space expands from
//! nothing, and only then is it released and animates in from its
//! [`Entering`] state. A view it drops leaves in two steps too: it
//! animates out to its [`Leaving`] state, then its space collapses.

use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::resource::Resource;
use bevy::ecs::world::{EntityWorldMut, World};
use bevy::picking::Pickable;
use bevy::ui::{
    ComputedNode, Display, FlexDirection, Node, Overflow,
    PositionType, UiRect, Val, px,
};

/// On a view's root for its first frame, so a `.when::<Entering, _>`
/// rule sets where it animates in from. A view built after the first
/// build keeps it until its space has expanded.
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
    ignore_pointer(world, node);
}

/// Stops `node` and every node under it from being picked, and gives
/// back what each had been set to.
fn ignore_pointer(
    world: &mut World,
    node: Entity,
) -> Vec<(Entity, Option<Pickable>)> {
    let mut was = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        let Ok(mut entity) = world.get_entity_mut(node) else {
            continue;
        };
        was.push((node, entity.get::<Pickable>().copied()));
        entity.insert(Pickable::IGNORE);
        if let Some(children) = entity.get::<Children>() {
            stack.extend(children.iter());
        }
    }
    was
}

/// What a node was before a hold or a collapse changed how it takes
/// space, put back when it is released.
#[derive(Clone, Copy, Debug)]
struct Before {
    position_type: PositionType,
    inset: UiRect,
    overflow: Overflow,
    flex_shrink: f32,
    padding: UiRect,
    border: UiRect,
    margin: UiRect,
    width: Val,
    height: Val,
    min_width: Val,
    min_height: Val,
    visibility: Visibility,
}

impl Before {
    /// What `ui` takes space with, and `visibility`.
    fn of(ui: &Node, visibility: Visibility) -> Self {
        Self {
            position_type: ui.position_type,
            inset: UiRect {
                left: ui.left,
                right: ui.right,
                top: ui.top,
                bottom: ui.bottom,
            },
            overflow: ui.overflow,
            flex_shrink: ui.flex_shrink,
            padding: ui.padding,
            border: ui.border,
            margin: ui.margin,
            width: ui.width,
            height: ui.height,
            min_width: ui.min_width,
            min_height: ui.min_height,
            visibility,
        }
    }

    /// Writes what this takes space with back onto `ui`.
    fn put(&self, ui: &mut Node) {
        ui.position_type = self.position_type;
        ui.left = self.inset.left;
        ui.right = self.inset.right;
        ui.top = self.inset.top;
        ui.bottom = self.inset.bottom;
        ui.overflow = self.overflow;
        ui.flex_shrink = self.flex_shrink;
        ui.padding = self.padding;
        ui.border = self.border;
        ui.margin = self.margin;
        ui.width = self.width;
        ui.height = self.height;
        ui.min_width = self.min_width;
        ui.min_height = self.min_height;
    }
}

/// Runs `write` on what a held node takes space with once released,
/// leaving the node itself to the hold. False when it is not held.
pub(crate) fn write_held(
    entity: &mut EntityWorldMut,
    write: impl FnOnce(&mut Node),
) -> bool {
    let Some(mut held) = entity.get_mut::<Held>() else {
        return false;
    };
    let mut ui = Node::default();
    held.before.put(&mut ui);
    write(&mut ui);
    held.before = Before::of(&ui, held.before.visibility);
    true
}

/// On the root of a view built after the first build, from the moment
/// it is built until it is released: out of the layout, so it takes
/// no space while its natural size is measured, unseen and ignoring
/// the pointer.
#[derive(Component, Debug)]
pub struct Held {
    before: Before,
    /// What the nodes under it had for a pointer, to give back.
    unpicked: Vec<(Entity, Option<Pickable>)>,
}

/// On a node whose space is collapsing or expanding: its natural size
/// along its parent's main axis, and what takes space around it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Collapsing {
    size: f32,
    gap: f32,
    /// Whether the parent lays its children out in a row.
    row: bool,
    /// Whether the node is its parent's first child, so the gap to
    /// take back is after it, not before.
    first: bool,
    padding: UiRect,
    border: UiRect,
    margin: UiRect,
}

fn px_of(val: Val) -> f32 {
    match val {
        Val::Px(px) => px,
        _ => 0.0,
    }
}

/// `val` kept `keep` of the way, if it is a length.
fn kept(val: Val, keep: f32) -> Val {
    match val {
        Val::Px(length) => px(length * keep),
        _ => px(0.0),
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
    // Nodes that take no space leave no gap beside the node either.
    let alone = siblings.is_none_or(|children| {
        !children.iter().copied().any(|sibling| {
            sibling != node
                && world.get::<Node>(sibling).is_some_and(|ui| {
                    ui.position_type == PositionType::Relative
                        && ui.display != Display::None
                })
        })
    });
    let ui = world.get::<Node>(node);
    Collapsing {
        size: if row { size.x } else { size.y },
        gap: if alone { 0.0 } else { gap },
        row,
        first,
        padding: ui.map_or(UiRect::ZERO, |ui| ui.padding),
        border: ui.map_or(UiRect::ZERO, |ui| ui.border),
        margin: ui.map_or(UiRect::ZERO, |ui| ui.margin),
    }
}

/// Keeps `node`, just built, out of the layout and out of sight,
/// until the layout that runs next has sized it for [`collapse`] to
/// measure.
pub(crate) fn hold(world: &mut World, node: Entity) {
    let Some(ui) = world.get::<Node>(node) else {
        return;
    };
    let before = Before::of(
        ui,
        world.get::<Visibility>(node).copied().unwrap_or_default(),
    );
    let row = world
        .get::<ChildOf>(node)
        .and_then(|child| world.get::<Node>(child.parent()))
        .is_some_and(|parent| {
            matches!(
                parent.flex_direction,
                FlexDirection::Row | FlexDirection::RowReverse
            )
        });
    let unpicked = ignore_pointer(world, node);
    // Spanning the parent across its main axis measures the node at
    // the size it will have in the flow.
    let mut ui = world.get_mut::<Node>(node).expect("a node");
    ui.position_type = PositionType::Absolute;
    if row {
        ui.top = px(0.0);
        ui.bottom = px(0.0);
    } else {
        ui.left = px(0.0);
        ui.right = px(0.0);
    }
    world
        .entity_mut(node)
        .insert((Held { before, unpicked }, Visibility::Hidden));
    world
        .resource_mut::<Entrances>()
        .0
        .retain(|&entered| entered != node);
}

/// Sets the space `node` takes along its parent's main axis,
/// `progress` of the way from its natural size to nothing, the
/// parent's gap beside it included. The first call measures it and
/// clips it, and puts a held node in the layout.
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
    let held = world.get::<Held>(node).map(|held| held.before);
    let keep = 1.0 - progress;
    // A negative margin takes the parent's gap back. bevy_ui lays it
    // out wrongly beside a node under a pixel in size, so the node
    // keeps that much and the margin takes it back too.
    let natural = collapsing.size * keep;
    let own = if collapsing.gap > 0.0 {
        natural.max(1.0)
    } else {
        natural
    };
    let size = px(own);
    let gap = natural - own - collapsing.gap * progress;
    let Some(mut ui) = world.get_mut::<Node>(node) else {
        return;
    };
    if let Some(before) = held {
        ui.position_type = before.position_type;
        ui.left = before.inset.left;
        ui.right = before.inset.right;
        ui.top = before.inset.top;
        ui.bottom = before.inset.bottom;
    }
    ui.overflow = Overflow::clip();
    // With a shrink under 1 a negative margin makes bevy_ui lay the
    // parent out at nothing, so the node shrinks, and its minimum
    // size, set below, is what keeps it from.
    ui.flex_shrink = 1.0;
    let (padding, border, margin) =
        (collapsing.padding, collapsing.border, collapsing.margin);
    if collapsing.row {
        ui.width = size;
        ui.min_width = size;
        ui.padding.left = kept(padding.left, keep);
        ui.padding.right = kept(padding.right, keep);
        ui.border.left = kept(border.left, keep);
        ui.border.right = kept(border.right, keep);
        ui.margin.left = kept(margin.left, keep);
        ui.margin.right = kept(margin.right, keep);
        if collapsing.first {
            ui.margin.right = px(px_of(ui.margin.right) + gap);
        } else {
            ui.margin.left = px(px_of(ui.margin.left) + gap);
        }
    } else {
        ui.height = size;
        ui.min_height = size;
        ui.padding.top = kept(padding.top, keep);
        ui.padding.bottom = kept(padding.bottom, keep);
        ui.border.top = kept(border.top, keep);
        ui.border.bottom = kept(border.bottom, keep);
        ui.margin.top = kept(margin.top, keep);
        ui.margin.bottom = kept(margin.bottom, keep);
        if collapsing.first {
            ui.margin.bottom = px(px_of(ui.margin.bottom) + gap);
        } else {
            ui.margin.top = px(px_of(ui.margin.top) + gap);
        }
    }
    if let Some(before) = held
        && let Some(mut visibility) =
            world.get_mut::<Visibility>(node)
    {
        *visibility = before.visibility;
    }
}

/// Makes `node` whole again: laid out as it was built, seen, picked,
/// and out of [`Entering`], so what it animates in from starts to
/// move.
pub(crate) fn release(world: &mut World, node: Entity) {
    let Ok(mut entity) = world.get_entity_mut(node) else {
        return;
    };
    entity.remove::<Collapsing>();
    entity.remove::<Entering>();
    let Some(held) = entity.take::<Held>() else {
        return;
    };
    let before = held.before;
    if let Some(mut ui) = entity.get_mut::<Node>() {
        before.put(&mut ui);
    }
    entity.insert(before.visibility);
    for (node, pickable) in held.unpicked {
        let Ok(mut entity) = world.get_entity_mut(node) else {
            continue;
        };
        match pickable {
            Some(pickable) => entity.insert(pickable),
            None => entity.remove::<Pickable>(),
        };
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
            px(1.0),
            "nothing measured without layout, so the one pixel kept"
        );
        assert_eq!(
            ui.margin.top,
            px(-6.0),
            "5 of the gap and the pixel"
        );
        assert_eq!(ui.overflow, Overflow::clip());
    }
}
