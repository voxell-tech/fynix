//! A click counter you can see: a Bevy window with one fynix-built
//! button. Hovering eases its fill colour, clicking bumps a resource,
//! and a binding writes the count back into the label.
//!
//! Run with `cargo run -p bevy_fynix --example counter`.

use bevy::color::Mix;
use bevy::color::palettes::css;
use bevy::prelude::*;
use bevy_fynix::prelude::*;
use bevy_fynix::watch_root;
use fynix::motiongfx_interp::ease;

/// How many times the button has been clicked. The label binds to it.
#[derive(Resource, Default)]
struct Clicks(u32);

/// The app's theme: every colour the element reads comes from here.
#[derive(Resource, Clone)]
struct Palette {
    idle: Color,
    hover: Color,
    text: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            idle: css::SLATE_BLUE.into(),
            hover: css::MEDIUM_PURPLE.into(),
            text: css::WHITE.into(),
        }
    }
}

type Host = BevyHost<Palette>;

/// A padded button whose fill eases from `idle` to `hover` while the
/// pointer is over it, with a text label as its child.
#[element(host = Host, build = button_chrome)]
pub struct CounterButton {
    #[elem(child)]
    pub label: Label,

    // 0 at rest, eased to 1 while `Hovered`. `WriteFill` mixes the
    // two theme colours by it.
    #[elem(default = 0.0, patch = WriteFill, anim(
        ms = 140,
        ease = ease::cubic::ease_in_out,
        on(Hovered, read = lit),
    ))]
    pub heat: f32,

    // Where the hover line heads, read in place, never written out.
    #[elem(default = 1.0)]
    pub lit: f32,
}

fn button_chrome(_: &CounterButton, b: &mut Build<Host, CounterButton>) {
    b.insert((
        Button,
        Node {
            padding: UiRect::axes(Val::Px(28.0), Val::Px(16.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
    ));
    b.observe(|_: On<Pointer<Click>>, mut clicks: ResMut<Clicks>| {
        clicks.0 += 1;
    });
}

pub struct WriteFill;

impl FieldPatch<Host> for WriteFill {
    type Target = f32;

    fn patch(patch: &mut Patch<Host>, heat: &f32) {
        let fill = patch.theme.idle.mix(&patch.theme.hover, *heat);
        let node = patch.id();
        patch.world.entity_mut(node).insert(BackgroundColor(fill));
    }
}

/// One text field, written straight into a `Text` component.
#[element(host = Host, build = label_chrome)]
pub struct Label {
    #[elem(default = String::new(), patch = WriteText)]
    pub text: String,
}

fn label_chrome(_: &Label, b: &mut Build<Host, Label>) {
    let color = b.theme.text;
    b.insert((TextColor(color), TextFont::from_font_size(30.0)));
}

pub struct WriteText;

impl FieldPatch<Host> for WriteText {
    type Target = String;

    fn patch(patch: &mut Patch<Host>, text: &String) {
        let node = patch.id();
        patch.world.entity_mut(node).insert(Text::new(text.clone()));
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(FynixPlugin::<Palette>::default())
        .init_resource::<Clicks>()
        .add_systems(Startup, setup)
        .run();
}

fn setup(world: &mut World) {
    world.spawn(Camera2d);

    let root = world
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();

    // Build the button once. `pointer_tags` keeps `Hovered` current
    // so the fill line has something to answer to, and the binding
    // rewrites the label from `Clicks` on every flush.
    watch_root::<Palette>(world, root, |ui| {
        ui.elem(elem!(CounterButton)).pointer_tags().bind(
            |button| button.label().text(),
            |_| true,
            |node| label_for(node.resource::<Clicks>().0),
        );
    });
}

fn label_for(clicks: u32) -> String {
    match clicks {
        0 => "click me".to_owned(),
        1 => "1 click".to_owned(),
        n => format!("{n} clicks"),
    }
}
