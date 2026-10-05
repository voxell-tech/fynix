#![doc = include_str!("../README.md")]

pub mod backend;
pub mod cursor;
pub mod dock;
pub mod leave;
pub mod modifier;
pub mod mounted;
pub mod patch;
pub mod prop;
pub mod shortcut;
pub mod state;
pub mod style;
mod tab;
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
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::world::World;
use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::TabGroup;
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
    DirtyNodes, Dragging, Focused, Hovered, Pressed, State, StateExt,
    hidden, own,
};
pub use style::{Style, style};
pub use transition::{BevyMarker, ReducedMotion};
pub use views::TooltipTiming;
pub use visual::Visual;

/// The theme views are built with, as a resource.
#[derive(Resource)]
pub struct Theme<T>(pub T);

/// Keeps every mounted view's bound props in step with the world.
/// The app inserts [`Theme<T>`] itself. An app may add one per theme
/// it builds views with.
pub struct FynixPlugin<T>(PhantomData<fn() -> T>);

impl<T> Default for FynixPlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Send + Sync + 'static> Plugin for FynixPlugin<T> {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CorePlugin>() {
            app.add_plugins(CorePlugin);
        }
        app.init_resource::<Mounts<T>>()
            .init_resource::<mounted::Pending<T>>()
            .add_systems(
                Update,
                mounted::update::<T>.in_set(FynixSystems::Mount),
            );
        app.world_mut()
            .resource_mut::<mounted::Receivers>()
            .0
            .push(mounted::receive::<T>);
    }
}

/// The steps of an update, in order.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FynixSystems {
    Prepare,
    Mount,
    Settle,
}

/// What every theme's [`FynixPlugin`] shares, added once.
struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<CursorPlugin>() {
            app.add_plugins(CursorPlugin);
        }
        shortcut::plugin(app);
        app.init_resource::<InputFocus>()
            .init_resource::<Unmounted>()
            .init_resource::<DirtyNodes>()
            .init_resource::<mounted::Receivers>()
            .init_resource::<ReducedMotion>()
            .init_resource::<Entrances>()
            .init_resource::<TooltipTiming>()
            .add_observer(backend::queue_unmounted)
            .add_observer(views::close_on_outside_press)
            .add_observer(views::close_on_escape)
            .add_observer(views::toggle_dropdown)
            .add_observer(views::dismiss_context_menu)
            .add_observer(tab::tab)
            .configure_sets(
                Update,
                (
                    FynixSystems::Prepare,
                    FynixSystems::Mount,
                    FynixSystems::Settle,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                views::tick_tooltips.in_set(FynixSystems::Prepare),
            )
            .add_systems(
                Update,
                (
                    views::sync_text_inputs,
                    leave::settle_entrances,
                    views::focus_first,
                )
                    .chain()
                    .in_set(FynixSystems::Settle),
            )
            .add_systems(Update, views::despawn_orphans);
    }
}

/// Builds `view` at the root of the UI, with no rules in force. Its
/// root is a [`TabGroup`], unless the view made it one already.
pub fn mount<T: Send + Sync + 'static>(
    world: &mut World,
    view: impl View<Bevy, T>,
) -> Entity {
    let root =
        world.resource_scope::<Mounts<T>, _>(|world, mut mounts| {
            world.resource_scope::<Theme<T>, _>(|world, theme| {
                let mut cx = Cx::new(world, &theme.0, &mut mounts.0);
                view.build(&mut cx)
            })
        });
    let mut entity = world.entity_mut(root);
    if !entity.contains::<TabGroup>() {
        entity.insert(TabGroup::default());
    }
    root
}
