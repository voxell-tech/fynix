//! The Bevy backend of the core.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Despawn;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::ecs::world::{EntityWorldMut, World};
use bevy::ui::Node;

/// Bevy's ECS, with `bevy_ui` doing layout, text and picking.
pub struct Bevy;

/// The UI nodes despawned since the last update.
#[derive(Resource, Default, Debug)]
pub struct Unmounted(pub Vec<Entity>);

/// A child its parent spawned for itself, kept when the views under
/// the parent are put in order.
#[derive(Component, Default)]
pub(crate) struct Kept;

/// A change to a node as it is spawned.
type Sow = Box<dyn FnOnce(&mut EntityWorldMut) + Send + Sync>;

/// What the next node spawned is given before anything is written to
/// it, put there by [`Seeded`](crate::views::Seeded).
#[derive(Resource, Default)]
pub(crate) struct Seed(Vec<Sow>);

impl Seed {
    /// Has `seed` run on the next node spawned.
    pub(crate) fn push(
        &mut self,
        seed: impl FnOnce(&mut EntityWorldMut) + Send + Sync + 'static,
    ) {
        self.0.push(Box::new(seed));
    }

    /// Forgets what the next node would have been given.
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
}

/// Queues every despawned [`Node`]. Only a queue, as the mounts are
/// out of the world while they update, and a despawn can happen then.
pub(crate) fn queue_unmounted(
    despawn: On<Despawn, Node>,
    mut queue: ResMut<Unmounted>,
) {
    queue.0.push(despawn.entity);
}

impl fynix::Backend for Bevy {
    type World = World;
    type Node = Entity;

    fn spawn(world: &mut World, parent: Option<Entity>) -> Entity {
        let seeds = world
            .get_resource_mut::<Seed>()
            .map(|mut seed| core::mem::take(&mut seed.0))
            .unwrap_or_default();
        let mut entity = world.spawn(Node::default());
        for seed in seeds {
            seed(&mut entity);
        }
        let node = entity.id();
        if let Some(parent) = parent {
            world.entity_mut(parent).add_child(node);
        }
        node
    }

    fn despawn(world: &mut World, node: Entity) {
        if let Ok(node) = world.get_entity_mut(node) {
            node.despawn();
        }
    }

    fn reorder(
        world: &mut World,
        parent: Entity,
        children: &[Entity],
    ) {
        // Ahead of the views, where it was spawned: what the node
        // made for itself.
        let kept = world
            .get::<Children>(parent)
            .into_iter()
            .flatten()
            .copied()
            .filter(|&child| world.get::<Kept>(child).is_some());
        let all =
            kept.chain(children.iter().copied()).collect::<Vec<_>>();
        world.entity_mut(parent).replace_children(&all);
    }

    fn leave(world: &mut World, node: Entity) {
        crate::leave::leave(world, node);
    }

    fn hold(world: &mut World, node: Entity) {
        crate::leave::hold(world, node);
    }

    fn collapse(world: &mut World, node: Entity, progress: f32) {
        crate::leave::collapse(world, node, progress);
    }

    fn release(world: &mut World, node: Entity) {
        crate::leave::release(world, node);
    }
}
