//! A small motion editor made of the crate's views, docked in panels
//! that can be dragged, split and closed. Try:
//! - the menus and their keys: Space plays, Cmd or Ctrl+N adds a
//!   layer,
//! - right-clicking a layer, and its Move submenu,
//! - the inspector's fields, which the viewport follows,
//! - scrubbing the timeline, and the log of 100,000 rows below it,
//! - the Styles tab beside the viewport, for every button look.
//!
//! `cargo run -p bevy_fynix --example showcase`

mod commands;
#[path = "../common/mod.rs"]
mod common;
mod icons;
mod inspector;
mod output;
mod scene;
mod styles;
mod timeline;

use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup, Startup, Update};
use bevy::asset::{AssetId, Assets};
use bevy::camera::Camera2d;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::world::World;
use bevy::text::Font;
use bevy::time::Time;
use bevy::ui::{AlignItems, UiRect, percent, px};
use bevy::utils::default;
use bevy::window::{
    MonitorSelection, Window, WindowPlugin, WindowPosition,
};
use bevy_fynix::dock::{
    DockAreaStyle, DockIcons, DockLeaf, DockPlugin, DockRegistry,
    DockTree, DockWindowKind, Edge, dock,
};
use bevy_fynix::shortcut::{
    CommandId, Invoke, run_command, shortcut_text,
};
use bevy_fynix::tokens::{SurfaceTokens, Tone};
use bevy_fynix::views::{
    BehaviorExt, FrameProps, Label, MenuEntry, MenuTitle, TooltipExt,
    button, column, label, menu_button, primary, row, segmented,
};
use bevy_fynix::{
    AnyView, Bevy, Cx, FynixPlugin, FynixSystems, ReducedMotion,
    ScopedExt, Theme, View, mount, resource,
};
use commands::{
    DELETE, FOLLOW, JUMP, NEW_LAYER, PLAY, REDUCE_MOTION, RENDER,
    REVERSE, REWIND,
};
use common::Monokai;

/// What a view is built with, in this app.
type Build<'a> = Cx<'a, Bevy, Monokai>;

/// Inter, cut down to Latin.
const INTER: &[u8] =
    include_bytes!("../../../../assets/fonts/Inter.ttf");

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "fynix showcase".into(),
                    resolution: (1360, 820).into(),
                    position: WindowPosition::Centered(
                        MonitorSelection::Primary,
                    ),
                    ..default()
                }),
                ..default()
            }),
            FynixPlugin::<Monokai>::default(),
            DockPlugin::<Monokai>::default(),
            commands::plugin,
        ))
        .insert_resource(Theme(Monokai))
        .insert_resource(DockIcons {
            close: Some(icons::CLOSE),
            add: Some(icons::PLUS),
        })
        .insert_resource(Clip {
            playing: true,
            time: 0.0,
        })
        .insert_resource(Shape {
            name: "Cube".into(),
            x: 140.0,
            y: 150.0,
            size: 96.0,
            travel: 440.0,
            radius: 18.0,
            opacity: 1.0,
            visible: true,
        })
        .insert_resource(Easing(3))
        .insert_resource(Tool(0))
        .insert_resource(Page(0))
        .insert_resource(Layers {
            ids: vec![1, 2, 3],
            next: 4,
        })
        .insert_resource(LogRows(100_000))
        .insert_resource(FollowLog(true))
        .init_resource::<timeline::Pose>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (timeline::play, output::follow_from_bottom)
                .before(FynixSystems::Mount),
        )
        .run();
}

/// The length of the clip, in seconds.
const DURATION: f32 = 4.0;

/// Where the playhead is, and whether it moves.
#[derive(Resource)]
struct Clip {
    playing: bool,
    time: f32,
}

/// The one object in the scene, as the inspector edits it.
#[derive(Resource)]
struct Shape {
    name: String,
    x: f32,
    y: f32,
    size: f32,
    /// How far right it moves over the clip.
    travel: f32,
    radius: f32,
    opacity: f32,
    visible: bool,
}

/// Which of [`timeline::EASINGS`] the clip moves by.
#[derive(Resource)]
struct Easing(usize);

/// The tool picked in the menu bar.
#[derive(Resource)]
struct Tool(usize);

/// The inspector's page.
#[derive(Resource)]
struct Page(usize);

/// The ids of the scene's layers, top first, and the next id.
#[derive(Resource)]
struct Layers {
    ids: Vec<u32>,
    next: u32,
}

/// How many rows the log has.
#[derive(Resource)]
struct LogRows(usize);

/// Whether the log follows its bottom.
#[derive(Resource)]
struct FollowLog(bool);

fn setup(world: &mut World) {
    let inter = Font::from_bytes(INTER.to_vec());
    world
        .resource_mut::<Assets<Font>>()
        .insert(AssetId::default(), inter)
        .unwrap();
    icons::draw(world);
    world.spawn(Camera2d);
    seed_dock(world);
    mount::<Monokai>(world, app());
}

/// The panels: the scene on the left, the inspector on the right,
/// and the timeline and log under the viewport.
fn seed_dock(world: &mut World) {
    let mut registry = world.resource_mut::<DockRegistry<Monokai>>();
    for (id, name, view) in [
        ("viewport", "Viewport", timeline::viewport as fn() -> _),
        ("styles", "Styles", styles::styles),
        ("scene", "Scene", scene::scene),
        ("inspector", "Inspector", inspector::inspector),
        ("timeline", "Timeline", timeline::timeline),
        ("log", "Log", output::output),
    ] {
        registry.register(id, DockWindowKind::new(name, view));
    }

    let mut tree = world.resource_mut::<DockTree>();
    let center = tree.set_root_leaf(
        DockLeaf::new("center", DockAreaStyle::TabBar)
            .with_windows(vec!["viewport".into(), "styles".into()]),
    );
    let (scene, _) =
        tree.split(center, Edge::Left, "scene".into()).unwrap();
    let (inspector, _) =
        tree.split(center, Edge::Right, "inspector".into()).unwrap();
    let (bottom, timeline) =
        tree.split(center, Edge::Bottom, "timeline".into()).unwrap();
    tree.add_tab(bottom, "log");
    tree.set_active(bottom, timeline);
    for (leaf, fraction) in
        [(scene, 0.17), (inspector, 0.72), (bottom, 0.66)]
    {
        let split = tree.parent_of(leaf).unwrap();
        tree.set_fraction(split, fraction);
    }
}

fn app() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        // The app's preamble: every label is 13px unless its call
        // site or an inner scope says otherwise.
        cx.set::<Label>(|label, _| label.size(13.0));
        let panel = cx.theme().panel();
        cx.build(
            column((
                menu_strip(),
                column((dock::<Monokai>(),))
                    .grow(1.0)
                    .min_height(px(0.0)),
                status_bar(),
            ))
            .width(percent(100.0))
            .height(percent(100.0))
            .gap(0.0)
            .fill(panel),
        )
    })
}

/// A run of [`menu_button`] rows and headings.
enum Item {
    Heading(&'static str),
    Run(CommandId),
}

/// A menu of `items`, each command shown with its key.
fn menu(
    world: &World,
    title: &'static str,
    items: &'static [Item],
) -> MenuTitle {
    let entries = items.iter().filter_map(|item| match item {
        Item::Heading(text) => Some(MenuEntry::section(*text)),
        Item::Run(id) => MenuEntry::command(world, *id),
    });
    menu_button(title, entries, move |world, at| {
        let id = items
            .iter()
            .filter_map(|item| match item {
                Item::Run(id) => Some(*id),
                Item::Heading(_) => None,
            })
            .nth(at);
        if let Some(id) = id {
            run_command(world, id, Invoke::default());
        }
    })
}

/// The app's name, its menus, a tool picker and a render button.
fn menu_strip() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        let layer = menu(
            cx.world,
            "Layer",
            &[
                Item::Run(NEW_LAYER),
                Item::Run(REVERSE),
                Item::Run(DELETE),
            ],
        );
        let playback = menu(
            cx.world,
            "Playback",
            &[
                Item::Heading("Clip"),
                Item::Run(PLAY),
                Item::Run(REWIND),
                Item::Heading("Motion"),
                Item::Run(REDUCE_MOTION),
            ],
        );
        let view = menu(
            cx.world,
            "View",
            &[Item::Run(FOLLOW), Item::Run(JUMP)],
        );
        let render_key =
            shortcut_text(cx.world, RENDER).unwrap_or_default();
        let hairline = cx.theme().hairline();
        cx.build(
            row((
                label("fynix")
                    .size(15.0)
                    .bold(true)
                    .tone(Tone::Accent),
                // Each title fills the bar's height.
                row((layer, playback, view))
                    .gap(0.0)
                    .height(percent(100.0)),
                column(()).grow(1.0),
                segmented(
                    ["Select", "Move", "Scale"],
                    resource::<Tool, _>(|tool| tool.0),
                    |world, at| world.resource_mut::<Tool>().0 = at,
                )
                .width(px(220.0)),
                button(
                    row((
                        label("Render"),
                        label(render_key).tone(Tone::OnAccent),
                    ))
                    .gap(10.0),
                )
                .rules(primary)
                .on_activate(commands::run(RENDER))
                .tooltip(|| {
                    label("Adds a second of frames to the log")
                }),
            ))
            .gap(12.0)
            .align(AlignItems::Center)
            .height(px(46.0))
            .padding(UiRect::horizontal(px(14.0)))
            .border(1.0)
            .border_color(hairline),
        )
    })
}

/// Live values along the bottom of the window.
fn status_bar() -> AnyView<Bevy, Monokai> {
    AnyView::new(|cx: &mut Build| {
        let fill = cx.theme().fill();
        let small = |text| label(text).size(11.0).tone(Tone::Dim);
        cx.build(
            row((
                small(resource::<Clip, _>(|clip| {
                    if clip.playing { "Playing" } else { "Paused" }
                        .to_string()
                }))
                .tone(Tone::Accent),
                small(resource::<Layers, _>(|layers| {
                    format!("{} layers", layers.ids.len())
                })),
                small(resource::<LogRows, _>(|rows| {
                    format!("{} log rows", rows.0)
                })),
                column(()).grow(1.0),
                small(resource::<Time, _>(|time| {
                    format!("{:.1}s up", time.elapsed_secs())
                })),
                small(resource::<ReducedMotion, _>(|reduced| {
                    if reduced.0 {
                        "Motion reduced"
                    } else {
                        "Full motion"
                    }
                    .to_string()
                })),
            ))
            .gap(16.0)
            .align(AlignItems::Center)
            .padding(UiRect::axes(px(14.0), px(5.0)))
            .fill(fill),
        )
    })
}

/// A panel's content, filling its area.
fn panel(
    content: impl View<Bevy, Monokai>,
) -> impl View<Bevy, Monokai> {
    column((content,))
        .width(percent(100.0))
        .height(percent(100.0))
        .padding(UiRect::all(px(12.0)))
}

/// A dim caption over its contents.
fn section(
    title: &'static str,
    body: impl View<Bevy, Monokai>,
) -> impl View<Bevy, Monokai> {
    column((label(title).size(11.0).tone(Tone::Dim), body)).gap(6.0)
}
