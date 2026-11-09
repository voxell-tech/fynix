//! What reaches a rich field: keys, the pointer, focus and an input
//! method.

use core::time::Duration;

use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ecs::world::World;
use bevy::input::ButtonInput;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{
    FocusGained, FocusLost, FocusedInput, InputFocus,
};
use bevy::math::Vec2;
use bevy::picking::events::{PointerDrag, PointerPress};
use bevy::picking::pointer::PointerButton;
use bevy::time::{Real, Time};
use bevy::ui::{
    ComputedNode, ComputedUiRenderTargetInfo, UiGlobalTransform,
    UiScale,
};
use bevy::window::Ime;

use super::edit::{Edit, edit_for, modifiers};
use super::{Enter, OnSubmit, RichField};
use crate::views::text_input::call;

pub(super) fn key(
    mut key: On<FocusedInput<KeyboardInput>>,
    mut fields: Query<&mut RichField>,
    keys: Res<ButtonInput<Key>>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let node = key.focused_entity;
    let Ok(mut field) = fields.get_mut(node) else {
        return;
    };
    // An input method has the keys while it composes.
    if field.preedit.is_some() {
        key.propagate(false);
        return;
    }
    if !key.input.state.is_pressed() {
        return;
    }
    let held = modifiers(|pressed| keys.pressed(pressed));
    let shift = keys.pressed(Key::Shift);
    let edit = match key.input.logical_key {
        Key::Enter if shift || field.enter == Enter::Newline => {
            Some(Edit::Insert("\n".into()))
        }
        Key::Enter => {
            let pieces = field.pieces();
            commands.queue(move |world: &mut World| {
                call::<OnSubmit>(world, node, |submit, world| {
                    (submit.0)(world, pieces);
                });
            });
            key.propagate(false);
            return;
        }
        // Escape belongs to what the field is in as well: the focus
        // leaves, and the press goes on.
        Key::Escape => {
            field.pending.push(Edit::Collapse);
            focus.clear();
            return;
        }
        _ => edit_for(&key.input, held, cfg!(target_os = "macos")),
    };
    if let Some(edit) = edit {
        field.pending.push(edit);
        key.propagate(false);
    }
}

/// The point of the text at `position` in the window, as the layout
/// has it: in physical pixels from its origin, held to the text.
fn point(
    nodes: &Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiRenderTargetInfo,
    )>,
    text: Entity,
    position: Vec2,
    ui_scale: f32,
) -> Option<Vec2> {
    let (node, transform, target) = nodes.get(text).ok()?;
    let inverse = transform.try_inverse()?;
    let local = inverse.transform_point2(
        position * target.scale_factor() / ui_scale,
    );
    let content = node.content_box();
    Some(content.clamp_point(local) - content.min)
}

pub(super) fn press(
    mut press: On<PointerPress>,
    mut fields: Query<&mut RichField>,
    nodes: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiRenderTargetInfo,
    )>,
    keys: Res<ButtonInput<Key>>,
    ui_scale: Option<Res<UiScale>>,
) {
    if press.button != PointerButton::Primary {
        return;
    }
    let Ok(mut field) = fields.get_mut(press.event_target()) else {
        return;
    };
    press.propagate(false);
    if field.preedit.is_some() {
        return;
    }
    let scale = ui_scale.map_or(1.0, |scale| scale.0);
    if let Some(at) =
        point(&nodes, field.text, press.pointer.position, scale)
    {
        field.pending.push(Edit::Press {
            at,
            taps: usize::from(press.count),
            extend: keys.pressed(Key::Shift),
        });
    }
}

pub(super) fn drag(
    mut drag: On<PointerDrag>,
    mut fields: Query<&mut RichField>,
    nodes: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiRenderTargetInfo,
    )>,
    ui_scale: Option<Res<UiScale>>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let Ok(mut field) = fields.get_mut(drag.event_target()) else {
        return;
    };
    drag.propagate(false);
    if field.preedit.is_some() {
        return;
    }
    let scale = ui_scale.map_or(1.0, |scale| scale.0);
    if let Some(at) =
        point(&nodes, field.text, drag.pointer.position, scale)
    {
        field.pending.push(Edit::Drag(at));
    }
}

pub(super) fn gained(
    gained: On<FocusGained>,
    mut fields: Query<&mut RichField>,
    time: Option<Res<Time<Real>>>,
) {
    if let Ok(mut field) = fields.get_mut(gained.event_target()) {
        field.active =
            time.map_or(Duration::ZERO, |time| time.elapsed());
    }
}

pub(super) fn lost(
    lost: On<FocusLost>,
    mut fields: Query<&mut RichField>,
) {
    if let Ok(mut field) = fields.get_mut(lost.event_target()) {
        field.preedit = None;
        field.pending.push(Edit::Collapse);
    }
}

/// Hands what an input method composes, and what it commits, to the
/// field that has the focus.
pub(super) fn ime(
    events: Option<MessageReader<Ime>>,
    focus: Res<InputFocus>,
    mut fields: Query<&mut RichField>,
) {
    let Some(mut events) = events else {
        return;
    };
    let mut field =
        focus.get().and_then(|focused| fields.get_mut(focused).ok());
    for event in events.read() {
        let Some(field) = field.as_mut() else {
            continue;
        };
        match event {
            Ime::Preedit { value, .. } => {
                field.preedit =
                    (!value.is_empty()).then(|| value.clone());
            }
            Ime::Commit { value, .. } => {
                field.preedit = None;
                field.pending.push(Edit::Insert(value.clone()));
            }
            Ime::Enabled { .. } | Ime::Disabled { .. } => {
                field.preedit = None;
            }
        }
    }
}
