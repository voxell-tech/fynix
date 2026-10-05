//! Commands and the keys bound to them.
//!
//! A command is a named thing the user can run. A [`Keymap`] binds
//! [`Chord`]s to commands, and one observer runs the command a key
//! press is bound to, for a press no focused node kept to itself.

use core::fmt;

use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::ecs::world::World;
use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::input_focus::{FocusedInput, InputFocus};
use bevy::picking::events::{Pointer, Press};
use bevy::text::EditableText;
use bevy::ui_widgets::popover::Popover;
use bevy::window::Window;

/// Name of a command, unique across the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CommandId(pub &'static str);

/// Name of a scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScopeId(pub &'static str);

/// The scope of a command that runs wherever the pointer is.
pub const GLOBAL: ScopeId = ScopeId("global");
/// The scope of an open menu, dropdown list or context menu.
pub const MENU: ScopeId = ScopeId("menu");

/// How a scope ranks against the others. Earlier outranks later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    /// A popup, menu or dialog that is open.
    Modal,
    Global,
}

/// A place commands belong to.
#[derive(Clone, Copy, Debug)]
pub struct ScopeSpec {
    pub id: ScopeId,
    pub label: &'static str,
    pub layer: Layer,
}

/// On the root node of what a scope covers, while it is there.
#[derive(Component, Clone, Copy)]
pub struct Scope(pub ScopeId);

/// Something the user can run by key, menu or button.
#[derive(Clone, Copy)]
pub struct CommandSpec {
    pub id: CommandId,
    pub label: &'static str,
    pub scope: ScopeId,
    pub run: fn(&mut World),
    pub enabled: fn(&World) -> bool,
    /// Whether a held key runs it again.
    pub repeat: bool,
}

/// Every registered command and scope.
#[derive(Resource, Default)]
pub struct CommandList {
    commands: Vec<CommandSpec>,
    scopes: Vec<ScopeSpec>,
}

impl CommandList {
    pub fn get(&self, id: CommandId) -> Option<&CommandSpec> {
        self.commands.iter().find(|command| command.id == id)
    }

    fn layer(&self, scope: ScopeId) -> Layer {
        self.scopes
            .iter()
            .find(|spec| spec.id == scope)
            .map_or(Layer::Global, |spec| spec.layer)
    }
}

/// Modifier keys held with a [`Chord`], left and right alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods(u8);

impl Mods {
    pub const NONE: Self = Self(0);
    /// Command on macOS, Control elsewhere.
    pub const PRIMARY: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);
    /// Control on macOS, where it is not [`Self::PRIMARY`].
    pub const CTRL: Self = Self(8);

    /// The modifiers down in `keys`.
    pub fn held(keys: &ButtonInput<KeyCode>) -> Self {
        let down = |codes: [KeyCode; 2]| keys.any_pressed(codes);
        let control =
            down([KeyCode::ControlLeft, KeyCode::ControlRight]);
        let command = down([KeyCode::SuperLeft, KeyCode::SuperRight]);
        let (primary, ctrl) = if cfg!(target_os = "macos") {
            (command, control)
        } else {
            (control, false)
        };
        let mut mods = Self::NONE;
        for (held, bit) in [
            (primary, Self::PRIMARY),
            (
                down([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
                Self::SHIFT,
            ),
            (down([KeyCode::AltLeft, KeyCode::AltRight]), Self::ALT),
            (ctrl, Self::CTRL),
        ] {
            if held {
                mods = mods.with(bit);
            }
        }
        mods
    }

    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// A key, by its place on the keyboard, with the modifiers held as
/// it goes down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: KeyCode,
    pub mods: Mods,
}

impl Chord {
    pub const fn key(key: KeyCode) -> Self {
        Self {
            key,
            mods: Mods::NONE,
        }
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let primary = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        for (bit, name) in [
            (Mods::PRIMARY, primary),
            (Mods::CTRL, "Ctrl"),
            (Mods::ALT, "Alt"),
            (Mods::SHIFT, "Shift"),
        ] {
            if self.mods.has(bit) {
                write!(f, "{name}+")?;
            }
        }
        let key = format!("{:?}", self.key);
        // `KeyW` and `Digit1` read as the key's own face.
        let face = key
            .strip_prefix("Key")
            .or_else(|| key.strip_prefix("Digit"))
            .unwrap_or(&key);
        write!(f, "{face}")
    }
}

/// A chord bound to a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub command: CommandId,
    pub chord: Chord,
}

/// The bindings in force.
#[derive(Resource, Default)]
pub struct Keymap(Vec<Binding>);

impl Keymap {
    /// The chords bound to `command`, in the order they were bound.
    pub fn chords(
        &self,
        command: CommandId,
    ) -> impl Iterator<Item = Chord> + '_ {
        self.0
            .iter()
            .filter(move |binding| binding.command == command)
            .map(|binding| binding.chord)
    }
}

/// Registers commands and scopes with an [`App`].
pub trait ShortcutAppExt {
    fn add_scope(&mut self, scope: ScopeSpec) -> &mut Self;

    /// Registers `command`, bound to `chords`.
    fn add_command(
        &mut self,
        command: CommandSpec,
        chords: &[Chord],
    ) -> &mut Self;
}

impl ShortcutAppExt for App {
    fn add_scope(&mut self, scope: ScopeSpec) -> &mut Self {
        self.init_resource::<CommandList>();
        self.world_mut()
            .resource_mut::<CommandList>()
            .scopes
            .push(scope);
        self
    }

    fn add_command(
        &mut self,
        command: CommandSpec,
        chords: &[Chord],
    ) -> &mut Self {
        self.init_resource::<CommandList>()
            .init_resource::<Keymap>();
        let world = self.world_mut();
        world.resource_mut::<CommandList>().commands.push(command);
        world.resource_mut::<Keymap>().0.extend(chords.iter().map(
            |&chord| Binding {
                command: command.id,
                chord,
            },
        ));
        self
    }
}

/// Runs the command `id`, if it is registered and enabled. Whether
/// it ran.
pub fn run_command(world: &mut World, id: CommandId) -> bool {
    let command = world
        .get_resource::<CommandList>()
        .and_then(|list| list.get(id).copied());
    match command {
        Some(command) if (command.enabled)(world) => {
            (command.run)(world);
            true
        }
        _ => false,
    }
}

/// The first chord bound to `id`, as the platform writes it.
pub fn shortcut_text(world: &World, id: CommandId) -> Option<String> {
    let chord = world.get_resource::<Keymap>()?.chords(id).next()?;
    Some(chord.to_string())
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<CommandList>()
        .init_resource::<Keymap>()
        .add_scope(ScopeSpec {
            id: MENU,
            label: "Menu",
            layer: Layer::Modal,
        })
        .add_observer(dispatch)
        .add_observer(blur_on_outside_press);
}

/// Runs the command a key press is bound to, for a press that reached
/// the window: one the focused node, or anything over it, did not
/// keep to itself.
fn dispatch(
    event: On<FocusedInput<KeyboardInput>>,
    windows: Query<(), With<Window>>,
    (keys, focus): (
        Option<Res<ButtonInput<KeyCode>>>,
        Res<InputFocus>,
    ),
    typing: Query<(), With<EditableText>>,
    open: Query<&Scope>,
    (list, keymap): (Res<CommandList>, Res<Keymap>),
    mut commands: Commands,
) {
    let key = &event.input;
    if !windows.contains(event.event_target())
        || key.state != ButtonState::Pressed
    {
        return;
    }
    let mods = keys.as_deref().map_or(Mods::NONE, Mods::held);
    let chord = Chord {
        key: key.key_code,
        mods,
    };
    // A bare key in a text field is that field's to type. The field
    // is read off the focus: the event names whatever it has bubbled
    // to.
    let plain = !(mods.has(Mods::PRIMARY)
        || mods.has(Mods::CTRL)
        || mods.has(Mods::ALT));
    let in_field =
        focus.get().is_some_and(|held| typing.contains(held));
    if plain && in_field {
        return;
    }
    // Only what is open over everything else answers while it is.
    let modal =
        open.iter().any(|scope| list.layer(scope.0) == Layer::Modal);
    let in_force = |scope: ScopeId| match list.layer(scope) {
        Layer::Modal => open.iter().any(|open| open.0 == scope),
        Layer::Global => !modal,
    };
    let bound = keymap
        .0
        .iter()
        .filter(|binding| binding.chord == chord)
        .filter_map(|binding| list.get(binding.command))
        .filter(|command| in_force(command.scope))
        .filter(|command| command.repeat || !key.repeat)
        .map(|command| command.id)
        .collect::<Vec<_>>();
    if bound.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        // The first of them that is enabled.
        for id in bound {
            if run_command(world, id) {
                break;
            }
        }
    });
}

/// Drops the focus on a press that lands away from the node holding
/// it, as nothing else does.
fn blur_on_outside_press(
    press: On<Pointer<Press>>,
    parents: Query<&ChildOf>,
    popups: Query<(), With<Popover>>,
    mut focus: ResMut<InputFocus>,
) {
    if press.entity != press.original_event_target() {
        return;
    }
    let Some(held) = focus.get() else {
        return;
    };
    // A popup holds the focus to stay open, and shuts itself.
    let in_popup = core::iter::once(held)
        .chain(parents.iter_ancestors(held))
        .any(|node| popups.contains(node));
    if in_popup {
        return;
    }
    let pressed = press.entity;
    // On it, in it, or on the frame it sits in.
    let near = pressed == held
        || parents.iter_ancestors(pressed).any(|up| up == held)
        || parents.get(held).is_ok_and(|up| up.parent() == pressed);
    if !near {
        focus.clear();
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::keyboard::Key;

    use super::*;
    use crate::mount;
    use crate::tests::{self, Plain};
    use crate::views::{BehaviorExt, frame};

    #[derive(Resource, Default)]
    struct Ran(u32);

    const BUMP: CommandId = CommandId("test.bump");

    fn app() -> App {
        let mut app = tests::app();
        tests::keyboard(&mut app);
        app.init_resource::<Ran>().add_command(
            CommandSpec {
                id: BUMP,
                label: "Bump",
                scope: GLOBAL,
                run: |world| world.resource_mut::<Ran>().0 += 1,
                enabled: |_| true,
                repeat: false,
            },
            &[Chord::key(KeyCode::KeyW)],
        );
        app
    }

    fn ran(app: &App) -> u32 {
        app.world().resource::<Ran>().0
    }

    #[test]
    fn a_bound_key_runs_its_command_unless_something_modal_is_open() {
        let mut app = app();
        tests::key_down(
            &mut app,
            KeyCode::KeyW,
            Key::Character("w".into()),
        );
        assert_eq!(ran(&app), 1);
        tests::key_down(
            &mut app,
            KeyCode::KeyE,
            Key::Character("e".into()),
        );
        assert_eq!(ran(&app), 1, "an unbound key runs nothing");

        let field =
            app.world_mut().spawn(EditableText::default()).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(field, bevy::input_focus::FocusCause::Pressed);
        tests::key_down(
            &mut app,
            KeyCode::KeyW,
            Key::Character("w".into()),
        );
        assert_eq!(ran(&app), 1, "a bare key is the field's to type");
        app.world_mut().resource_mut::<InputFocus>().clear();

        mount::<Plain>(app.world_mut(), frame().tagged(Scope(MENU)));
        app.update();
        tests::key_down(
            &mut app,
            KeyCode::KeyW,
            Key::Character("w".into()),
        );
        assert_eq!(ran(&app), 1, "a menu is open");
        assert_eq!(
            shortcut_text(app.world(), BUMP).as_deref(),
            Some("W")
        );
    }
}
