//! What the menus and keys run.

use bevy::app::App;
use bevy::ecs::world::World;
use bevy::input::keyboard::KeyCode;
use bevy_fynix::ReducedMotion;
use bevy_fynix::shortcut::{
    Chord, CommandId, CommandSpec, GLOBAL, Invoke, Mods,
    ShortcutAppExt, run_command,
};
use bevy_fynix::views::VirtualList;

use crate::{Clip, FollowLog, Layers, LogRows};

pub const PLAY: CommandId = CommandId("clip.play");
pub const REWIND: CommandId = CommandId("clip.rewind");
pub const RENDER: CommandId = CommandId("clip.render");
pub const NEW_LAYER: CommandId = CommandId("layer.new");
pub const REVERSE: CommandId = CommandId("layer.reverse");
pub const DELETE: CommandId = CommandId("layer.delete");
pub const REDUCE_MOTION: CommandId = CommandId("view.reduce_motion");
pub const FOLLOW: CommandId = CommandId("view.follow");
pub const JUMP: CommandId = CommandId("view.jump");

pub fn plugin(app: &mut App) {
    let primary = |key| Chord {
        key,
        mods: Mods::PRIMARY,
    };
    let shifted = |key| Chord {
        key,
        mods: Mods::PRIMARY.with(Mods::SHIFT),
    };
    for (id, label, run, chord) in [
        (
            PLAY,
            "Play or pause",
            play as fn(&mut World, Invoke),
            Chord::key(KeyCode::Space),
        ),
        (REWIND, "Back to start", rewind, Chord::key(KeyCode::Home)),
        (RENDER, "Render", render, primary(KeyCode::KeyR)),
        (NEW_LAYER, "New layer", new_layer, primary(KeyCode::KeyN)),
        (REVERSE, "Reverse layers", reverse, shifted(KeyCode::KeyR)),
        (
            DELETE,
            "Delete the top layer",
            delete,
            primary(KeyCode::Backspace),
        ),
        (
            REDUCE_MOTION,
            "Reduce motion",
            reduce_motion,
            shifted(KeyCode::KeyM),
        ),
        (FOLLOW, "Follow the log", follow, primary(KeyCode::KeyL)),
        (JUMP, "Jump to row 50,000", jump, primary(KeyCode::KeyJ)),
    ] {
        app.add_command(
            CommandSpec {
                id,
                label,
                scope: GLOBAL,
                run,
                enabled: |_| true,
                repeat: false,
            },
            &[chord],
        );
    }
}

fn play(world: &mut World, _: Invoke) {
    let mut clip = world.resource_mut::<Clip>();
    clip.playing = !clip.playing;
}

fn rewind(world: &mut World, _: Invoke) {
    world.resource_mut::<Clip>().time = 0.0;
}

/// A second of frames, added to the log.
fn render(world: &mut World, _: Invoke) {
    world.resource_mut::<LogRows>().0 += 60;
    world.resource_mut::<FollowLog>().0 = true;
}

fn new_layer(world: &mut World, _: Invoke) {
    let mut layers = world.resource_mut::<Layers>();
    let id = layers.next;
    layers.next += 1;
    layers.ids.insert(0, id);
}

fn reverse(world: &mut World, _: Invoke) {
    world.resource_mut::<Layers>().ids.reverse();
}

fn delete(world: &mut World, _: Invoke) {
    let mut layers = world.resource_mut::<Layers>();
    if !layers.ids.is_empty() {
        layers.ids.remove(0);
    }
}

fn reduce_motion(world: &mut World, _: Invoke) {
    let mut reduced = world.resource_mut::<ReducedMotion>();
    reduced.0 = !reduced.0;
}

fn follow(world: &mut World, _: Invoke) {
    let mut follow = world.resource_mut::<FollowLog>();
    follow.0 = !follow.0;
}

fn jump(world: &mut World, _: Invoke) {
    on_log(world, |list| list.scroll_to(50_000));
}

/// Runs `run` on the log's [`VirtualList`].
pub fn on_log(world: &mut World, run: impl FnOnce(&mut VirtualList)) {
    let mut lists = world.query::<&mut VirtualList>();
    if let Ok(mut list) = lists.single_mut(world) {
        run(&mut list);
    }
}

/// Runs the command `id`, from a button.
pub fn run(id: CommandId) -> impl Fn(&mut World) + Send + Sync {
    move |world| {
        run_command(world, id, Invoke::default());
    }
}
