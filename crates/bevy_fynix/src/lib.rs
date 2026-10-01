//! The Bevy backend of [`fynix`].
//!
//! The core owns views, set rules, props and transitions. This crate
//! says what a world and a node are in Bevy, writes the elements and
//! composites against `bevy_ui`, and keeps mounted elements in step with
//! the world.

pub mod backend;
pub mod cursor;
pub mod leave;
pub mod modifier;
pub mod mounted;
pub mod prop;
mod props;
pub mod state;
pub mod tokens;
pub mod transition;
pub mod views;
pub mod visual;

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::world::World;
pub use fynix::{
    AnyView, Cx, Element, Layered, ScopedExt, Styled, View, ViewExt,
    ViewSeq,
};

pub use backend::{Bevy, Unmounted};
pub use cursor::{CursorPlugin, EntityCursor};
pub use leave::{Entering, Entrances, Leaving};
pub use modifier::ModifierExt;
pub use mounted::Mounts;
pub use prop::{
    Derived, Each, Keyed, Prop, Signal, component, derived, each,
    every_frame, keyed, resource,
};
pub use state::{
    DirtyNodes, Hovered, Pressed, State, StateExt, hidden, own,
};
pub use transition::{BevyMarker, ReducedMotion};
pub use visual::Visual;

/// The theme views are built with, as a resource.
#[derive(Resource)]
pub struct Theme<T>(pub T);

/// Keeps every mounted view's bound props in step with the world.
/// The app inserts [`Theme<T>`] itself.
pub struct FynixPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for FynixPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Send + Sync + 'static> Plugin for FynixPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_plugins(CursorPlugin)
            .init_resource::<Mounts<T>>()
            .init_resource::<Unmounted>()
            .init_resource::<DirtyNodes>()
            .init_resource::<ReducedMotion>()
            .init_resource::<Entrances>()
            .add_observer(backend::queue_unmounted)
            .add_systems(
                Update,
                (mounted::update::<T>, leave::settle_entrances)
                    .chain(),
            );
    }
}

/// Builds `view` at the root of the UI, with no rules in force.
pub fn mount<T: Send + Sync + 'static>(
    world: &mut World,
    view: impl View<Bevy, T>,
) -> Entity {
    world.resource_scope::<Mounts<T>, _>(|world, mut mounts| {
        world.resource_scope::<Theme<T>, _>(|world, theme| {
            let mut cx = Cx::new(world, &theme.0, &mut mounts.0);
            view.build(&mut cx)
        })
    })
}
