//! The core's mounted elements, kept as a resource and updated each
//! frame.

use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::world::World;
use bevy::time::Time;

use crate::backend::Unmounted;
use crate::state::DirtyNodes;
use crate::transition::ReducedMotion;
use crate::{Bevy, Theme};

/// Every mounted element built with the theme `T`.
#[derive(Resource)]
pub struct Mounts<T: 'static>(pub fynix::Mounted<Bevy, T>);

impl<T: 'static> Default for Mounts<T> {
    fn default() -> Self {
        Self(fynix::Mounted::default())
    }
}

impl<T: 'static> Deref for Mounts<T> {
    type Target = fynix::Mounted<Bevy, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: 'static> DerefMut for Mounts<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The despawned and dirty nodes the theme `T`'s mounts have yet to
/// see. Every theme gets each node, out of the queues all of them
/// share.
#[derive(Resource)]
pub(crate) struct Pending<T> {
    gone: Vec<Entity>,
    dirty: Vec<Entity>,
    theme: PhantomData<fn() -> T>,
}

impl<T> Default for Pending<T> {
    fn default() -> Self {
        Self {
            gone: Vec::new(),
            dirty: Vec::new(),
            theme: PhantomData,
        }
    }
}

/// Hands the despawned and dirty nodes to one theme's [`Pending`].
type Receive = fn(&mut World, &[Entity], &[Entity]);

/// One [`Receive`] per theme an app builds views with.
#[derive(Resource, Default)]
pub(crate) struct Receivers(pub(crate) Vec<Receive>);

/// The [`Receive`] of the theme `T`.
pub(crate) fn receive<T: Send + Sync + 'static>(
    world: &mut World,
    gone: &[Entity],
    dirty: &[Entity],
) {
    let mut pending = world.resource_mut::<Pending<T>>();
    pending.gone.extend_from_slice(gone);
    pending.dirty.extend_from_slice(dirty);
}

/// Moves the shared queues into every theme's [`Pending`].
fn fan_out(world: &mut World) {
    let gone =
        core::mem::take(&mut world.resource_mut::<Unmounted>().0);
    let dirty =
        core::mem::take(&mut world.resource_mut::<DirtyNodes>().0);
    if gone.is_empty() && dirty.is_empty() {
        return;
    }
    let receivers = world.resource::<Receivers>().0.clone();
    for receive in receivers {
        receive(world, &gone, &dirty);
    }
}

/// Drops the mounts of the nodes despawned since the last drain.
fn drain_unmounted<T: Send + Sync + 'static>(
    world: &mut World,
    mounts: &mut Mounts<T>,
) {
    fan_out(world);
    let gone =
        core::mem::take(&mut world.resource_mut::<Pending<T>>().gone);
    for node in gone {
        mounts.unmount(node);
    }
}

/// Rebuilds what the world's changes call for, then brings every
/// mounted element up to date.
pub(crate) fn update<T: Send + Sync + 'static>(world: &mut World) {
    let tick = fynix::Tick {
        delta: world.resource::<Time>().delta(),
        reduced_motion: world
            .get_resource::<ReducedMotion>()
            .is_some_and(|reduced| reduced.0),
    };
    world.resource_scope::<Mounts<T>, _>(|world, mut mounts| {
        drain_unmounted(world, &mut mounts);
        // The states an effect sets are dirty for this update too.
        mounts.0.run_effects(world);
        fan_out(world);
        let dirty = core::mem::take(
            &mut world.resource_mut::<Pending<T>>().dirty,
        );
        for node in dirty {
            mounts.mark_dirty(node);
        }
        world.resource_scope::<Theme<T>, _>(|world, theme| {
            mounts.0.update_structure(world, &theme.0);
            // A rebuild despawns nodes whose elements are still
            // mounted.
            drain_unmounted(world, &mut mounts);
            mounts.0.update_elements(world, &theme.0, tick);
        });
    });
}
