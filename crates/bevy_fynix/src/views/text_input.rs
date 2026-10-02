//! The editable text node inside a [`TextField`](super::TextField)
//! or a [`NumberField`](super::NumberField).
//!
//! It needs Bevy's `EditableTextInputPlugin` for the keys and the
//! pointer, which `DefaultPlugins` adds.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::query::Changed;
use bevy::ecs::system::{Commands, Query};
use bevy::ecs::world::World;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::tab_navigation::TabIndex;
use bevy::input_focus::{
    FocusCause, FocusGained, FocusLost, FocusedInput, InputFocus,
};
use bevy::text::{EditableText, LineBreak, TextEdit, TextLayout};
use bevy::ui::{Node, px};
use bevy::window::SystemCursorIcon;
use fynix::element;

use super::label::PatchTextSize;
use crate::Bevy;
use crate::cursor::EntityCursor;
use crate::patch::{
    Paint, PatchInk, PatchOpacity, PatchScale, patch,
};
use crate::prop::Prop;
use crate::state::own_when;
use crate::tokens::{TextTokens, Tone};
use crate::transition::{blend_color, blend_f32};

/// A single line of editable text.
///
/// It shows `text`, or `number` inside a number field, while it is
/// not being edited, and what the user typed while it is.
#[element(backend = Bevy, theme = TextTokens, prepare = prepare)]
pub struct TextInput {
    #[elem(patch = PatchEntryText)]
    pub text: Prop<String>,
    #[elem(patch = PatchEntryNumber)]
    pub number: Prop<f64>,
    /// The theme's body size when unset.
    #[elem(
        default = theme.body_size(),
        patch = PatchTextSize,
        blend = blend_f32
    )]
    pub size: Prop<f32>,
    /// The colour, by role. Body when unset.
    #[elem(
        shown = Color,
        with = |tone, theme| theme.tone(tone),
        patch = PatchInk,
        blend = blend_color
    )]
    pub tone: Prop<Tone>,
    /// How opaque it is, 1.0 when unset.
    #[elem(default = 1.0, patch = PatchOpacity, blend = blend_f32)]
    pub opacity: Prop<f32>,
    /// The factor it is scaled by around its centre after layout,
    /// 1.0 when unset.
    #[elem(default = 1.0, patch = PatchScale, blend = blend_f32)]
    pub scale: Prop<f32>,
}

own_when!(TextInput);

/// What an input shows when nobody is editing it, and where an edit
/// stands.
#[derive(Component, Default)]
pub(crate) struct Entry {
    pub(super) text: String,
    pub(super) number: Option<f64>,
    /// The text last written to the display.
    shown: String,
    /// From the focus gained until the edit ends, which may be ahead
    /// of the focus leaving.
    editing: bool,
}

/// How a number is read and written. On the node of an input that
/// holds one.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct NumberSpec {
    pub step: f64,
    pub precision: Option<usize>,
    pub min: f64,
    pub max: f64,
    /// Whether the value is whole, so it is rounded and shown with
    /// no decimals.
    pub integer: bool,
}

impl NumberSpec {
    /// `value` cleaned of float noise, rounded if whole, and held to
    /// the range.
    pub fn clamp(&self, value: f64) -> f64 {
        let value = round9(value);
        let value = if self.integer { value.round() } else { value };
        value.max(self.min).min(self.max) + 0.0
    }

    fn format(&self, value: f64) -> String {
        match self.precision {
            _ if self.integer => {
                format!("{:.0}", value.round() + 0.0)
            }
            Some(precision) => format!("{value:.precision$}"),
            None => round9(value).to_string(),
        }
    }
}

fn round9(value: f64) -> f64 {
    if value.abs() < 1e15 {
        (value * 1e9).round() / 1e9 + 0.0
    } else {
        value
    }
}

impl Entry {
    fn shown(&self, spec: Option<&NumberSpec>) -> String {
        match (spec, self.number) {
            (Some(spec), Some(number)) => spec.format(number),
            _ => self.text.clone(),
        }
    }
}

/// A handler run with the whole world and a value.
type Handler<V> = Box<dyn Fn(&mut World, V) + Send + Sync>;

/// Run with the text of a text field when it is committed.
#[derive(Component)]
pub(super) struct TextCommit(pub Handler<String>);

/// Run with the value of a number field when it changes.
#[derive(Component)]
pub(super) struct NumberChange(pub Handler<f64>);

/// Runs `call` with the `C` on `node` taken off it, so `call` can
/// use the whole world.
pub(super) fn call<C: Component>(
    world: &mut World,
    node: Entity,
    call: impl FnOnce(&C, &mut World),
) {
    let Some(handler) = world
        .get_entity_mut(node)
        .ok()
        .and_then(|mut entity| entity.take::<C>())
    else {
        return;
    };
    call(&handler, world);
    if let Ok(mut entity) = world.get_entity_mut(node) {
        entity.insert(handler);
    }
}

fn prepare(world: &mut World, node: Entity) {
    if let Some(mut layout) = world.get_mut::<Node>(node) {
        layout.flex_grow = 1.0;
        layout.min_width = px(0.0);
    }
    world
        .entity_mut(node)
        .insert((
            EditableText::default(),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..TextLayout::default()
            },
            Paint::default(),
            Entry::default(),
            EntityCursor(SystemCursorIcon::Text),
            TabIndex(0),
        ))
        .observe(gained)
        .observe(lost)
        .observe(key);
}

patch!(PatchEntryText, String, |entity, v| {
    if let Some(mut entry) = entity.get_mut::<Entry>() {
        entry.text.clone_from(v);
    }
});
patch!(PatchEntryNumber, f64, |entity, v| {
    if let Some(mut entry) = entity.get_mut::<Entry>() {
        entry.number = Some(*v);
    }
});

fn is_focused(world: &World, node: Entity) -> bool {
    world
        .get_resource::<InputFocus>()
        .is_some_and(|focus| focus.get() == Some(node))
}

/// Whether `node` is being typed into.
pub(super) fn is_editing(world: &World, node: Entity) -> bool {
    is_focused(world, node)
        || world.get::<Entry>(node).is_some_and(|entry| entry.editing)
}

/// Writes what `node` shows when nobody is editing it into its
/// display.
pub(super) fn show(world: &mut World, node: Entity) {
    let Ok(mut entity) = world.get_entity_mut(node) else {
        return;
    };
    let Some(entry) = entity.get::<Entry>() else {
        return;
    };
    let shown = entry.shown(entity.get::<NumberSpec>());
    if entry.shown != shown
        && let Some(mut entry) = entity.get_mut::<Entry>()
    {
        entry.shown.clone_from(&shown);
    }
    if let Some(mut text) = entity.get_mut::<EditableText>()
        && text.value().to_string() != shown
    {
        text.editor_mut().set_text(&shown);
    }
}

/// Brings the display of every input whose entry changed up to date,
/// except the one being edited.
pub(crate) fn sync_text_inputs(
    changed: Query<Entity, Changed<Entry>>,
    mut commands: Commands,
) {
    for node in &changed {
        commands.queue(move |world: &mut World| {
            if !is_editing(world, node) {
                show(world, node);
            }
        });
    }
}

/// Gives `node` focus, with the caret at the end of its text.
pub(super) fn focus(world: &mut World, node: Entity) {
    if is_focused(world, node) {
        return;
    }
    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
        focus.set(node, FocusCause::Pressed);
    }
    if let Some(mut text) = world.get_mut::<EditableText>(node) {
        text.queue_edit(TextEdit::TextEnd(false));
    }
}

fn gained(gained: On<FocusGained>, mut commands: Commands) {
    let node = gained.event_target();
    commands.queue(move |world: &mut World| {
        if let Some(mut entry) = world.get_mut::<Entry>(node) {
            entry.editing = true;
        }
    });
}

fn lost(lost: On<FocusLost>, mut commands: Commands) {
    let node = lost.event_target();
    commands.queue(move |world: &mut World| {
        if world.get::<Entry>(node).is_some_and(|e| e.editing) {
            commit(world, node, false);
        }
    });
}

fn key(
    key: On<FocusedInput<KeyboardInput>>,
    texts: Query<&EditableText>,
    mut commands: Commands,
) {
    let node = key.focused_entity;
    let composing =
        texts.get(node).is_ok_and(EditableText::is_composing);
    if !key.input.state.is_pressed() || composing {
        return;
    }
    let enter = match key.input.logical_key {
        Key::Enter => true,
        Key::Escape => false,
        _ => return,
    };
    commands.queue(move |world: &mut World| {
        if enter {
            commit(world, node, true);
        } else {
            show(world, node);
        }
        if let Some(mut entry) = world.get_mut::<Entry>(node) {
            entry.editing = false;
        }
        if let Some(mut focus) =
            world.get_resource_mut::<InputFocus>()
            && focus.get() == Some(node)
        {
            focus.clear();
        }
    });
}

/// Hands what was typed in `node` to its handler, if it differs from
/// what was shown (or `always`, for text), and shows what stands.
fn commit(world: &mut World, node: Entity, always: bool) {
    let Some(typed) = world
        .get::<EditableText>(node)
        .map(|text| text.value().to_string())
    else {
        return;
    };
    let Some(entry) = world.get::<Entry>(node) else {
        return;
    };
    let unchanged = typed == entry.shown;
    match world.get::<NumberSpec>(node).copied() {
        None if always || !unchanged => {
            if let Some(mut entry) = world.get_mut::<Entry>(node) {
                entry.text.clone_from(&typed);
            }
            call::<TextCommit>(world, node, |commit, world| {
                (commit.0)(world, typed);
            });
        }
        Some(spec) if !unchanged => {
            let parsed = typed.trim().parse::<f64>().ok();
            if let Some(value) = parsed.filter(|v| v.is_finite()) {
                let value = spec.clamp(value);
                if let Some(mut entry) = world.get_mut::<Entry>(node)
                {
                    entry.number = Some(value);
                }
                call::<NumberChange>(world, node, |change, world| {
                    (change.0)(world, value);
                });
            }
        }
        _ => {}
    }
    if let Some(mut entry) = world.get_mut::<Entry>(node) {
        entry.editing = false;
    }
    show(world, node);
}
