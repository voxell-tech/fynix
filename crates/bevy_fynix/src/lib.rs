#![doc = include_str!("../README.md")]

pub mod backend;
pub mod cursor;
pub mod dock;
pub mod leave;
pub mod modifier;
pub mod mounted;
pub mod patch;
pub mod prop;
pub mod state;
pub mod style;
pub mod tokens;
pub mod transition;
pub mod views;
pub mod visual;

#[cfg(test)]
mod tests;

use core::marker::PhantomData;

pub use backend::{Bevy, Unmounted};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::world::World;
use bevy::input_focus::InputFocus;
pub use cursor::{CursorPlugin, EntityCursor, OverrideCursor};
pub use fynix::{
    AnyView, Cx, Element, Layered, ScopedExt, Styled, View, ViewExt,
    ViewSeq,
};
pub use leave::{Collapsing, Entering, Entrances, Held, Leaving};
pub use modifier::ModifierExt;
pub use mounted::Mounts;
pub use prop::{
    Derived, Each, Keyed, Prop, Signal, component, derived, each,
    every_frame, keyed, resource,
};
pub use state::{
    DirtyNodes, Focused, Hovered, Pressed, State, StateExt, hidden,
    own,
};
pub use style::{Style, style};
pub use transition::{BevyMarker, ReducedMotion};
pub use views::TooltipTiming;
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
            .init_resource::<InputFocus>()
            .init_resource::<Mounts<T>>()
            .init_resource::<Unmounted>()
            .init_resource::<DirtyNodes>()
            .init_resource::<ReducedMotion>()
            .init_resource::<Entrances>()
            .init_resource::<TooltipTiming>()
            .add_observer(backend::queue_unmounted)
            .add_observer(views::toggle_dropdown)
            .add_observer(views::dismiss_context_menu)
            .add_systems(
                Update,
                (
                    views::tick_tooltips,
                    mounted::update::<T>,
                    views::sync_text_inputs,
                    leave::settle_entrances,
                    views::focus_first,
                )
                    .chain(),
            )
            .add_systems(Update, views::despawn_orphans);
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
