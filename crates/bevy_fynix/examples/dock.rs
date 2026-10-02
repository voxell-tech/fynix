//! Docking on its own. Three panels start as tabs in one area. Try:
//! - dragging a tab along its bar to reorder it,
//! - dragging a tab onto another area's bar to move it there,
//! - dragging a tab onto an area's edge to split the area,
//! - dragging the line between two areas to resize them,
//! - pressing Escape mid-drag to cancel,
//! - closing a tab, and adding it back from the "+" of a bar.
//!
//! `cargo run -p bevy_fynix --example dock`

mod common;

use bevy::DefaultPlugins;
use bevy::app::{App, Startup};
use bevy::camera::Camera2d;
use bevy::ecs::world::World;
use bevy::ui::percent;
use bevy_fynix::dock::{
    DockAreaStyle, DockLeaf, DockPlugin, DockRegistry, DockTree,
    DockWindowKind, dock,
};
use bevy_fynix::views::{FrameProps, column, label};
use bevy_fynix::{AnyView, Bevy, FynixPlugin, Theme, ViewExt, mount};
use common::Monokai;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FynixPlugin::<Monokai>::default(),
            DockPlugin::<Monokai>::default(),
        ))
        .insert_resource(Theme(Monokai))
        .add_systems(Startup, setup)
        .run();
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);

    let mut registry = world.resource_mut::<DockRegistry<Monokai>>();
    for (id, name) in [
        ("panel_a", "Panel A"),
        ("panel_b", "Panel B"),
        ("panel_c", "Panel C"),
    ] {
        registry.register(
            id,
            DockWindowKind::new(name, move || panel(name)),
        );
    }

    world.resource_mut::<DockTree>().set_root_leaf(
        DockLeaf::new("root", DockAreaStyle::TabBar).with_windows(
            vec![
                "panel_a".into(),
                "panel_b".into(),
                "panel_c".into(),
            ],
        ),
    );

    mount::<Monokai>(
        world,
        column((dock::<Monokai>(),))
            .width(percent(100.0))
            .height(percent(100.0)),
    );
}

/// A panel's whole content: its name.
fn panel(name: &'static str) -> AnyView<Bevy, Monokai> {
    label(name).size(20.0).boxed()
}
