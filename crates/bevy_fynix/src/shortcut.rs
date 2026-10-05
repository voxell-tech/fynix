//! Commands and the keys bound to them.
//!
//! A command is a named thing the user can run. A [`Keymap`] binds
//! [`Chord`]s to commands, and one observer runs the command a key
//! press is bound to, for a press no focused node kept to itself.

use core::fmt;

use bevy::app::{App, PreUpdate};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
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
use bevy::picking::hover::HoverMap;
use bevy::picking::pointer::PointerId;
use bevy::reflect::Reflect;
use bevy::reflect::std_traits::ReflectDefault;
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
    /// A drag or other gesture under way.
    Gesture,
    /// A popup, menu or dialog that is open.
    Modal,
    /// A panel of the window, in force while the pointer is over it.
    Panel,
    Global,
}

/// What ran a command.
#[derive(Clone, Copy, Debug, Default)]
pub struct Invoke {
    /// The node carrying the [`Scope`] the command was found under,
    /// for a command of a panel.
    pub target: Option<Entity>,
}

/// A place commands belong to.
#[derive(Clone, Copy, Debug)]
pub struct ScopeSpec {
    pub id: ScopeId,
    pub label: &'static str,
    pub layer: Layer,
    /// Whether it is in force, for a scope no node stands for.
    pub active: Option<fn(&World) -> bool>,
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
    pub run: fn(&mut World, Invoke),
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

    /// Every command, in the order they were registered.
    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }

    /// Every scope, in the order they were registered.
    pub fn scopes(&self) -> &[ScopeSpec] {
        &self.scopes
    }

    fn layer(&self, scope: ScopeId) -> Layer {
        self.scopes
            .iter()
            .find(|spec| spec.id == scope)
            .map_or(Layer::Global, |spec| spec.layer)
    }
}

/// Modifier keys held with a [`Chord`], left and right alike.
#[derive(
    Reflect, Clone, Copy, Debug, Default, PartialEq, Eq, Hash,
)]
#[reflect(Default, Clone, PartialEq)]
pub struct Mods {
    /// Command on macOS, Control elsewhere.
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
    /// Control on macOS, where it is not `primary`.
    pub ctrl: bool,
    /// The Super key off macOS, where it is not `primary`.
    pub meta: bool,
}

impl Mods {
    pub const NONE: Self = Self {
        primary: false,
        shift: false,
        alt: false,
        ctrl: false,
        meta: false,
    };
    pub const PRIMARY: Self = Self {
        primary: true,
        ..Self::NONE
    };
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };
    pub const META: Self = Self {
        meta: true,
        ..Self::NONE
    };

    /// The modifiers down in `keys`.
    pub fn held(keys: &ButtonInput<KeyCode>) -> Self {
        let down = |codes: [KeyCode; 2]| keys.any_pressed(codes);
        let control =
            down([KeyCode::ControlLeft, KeyCode::ControlRight]);
        let command = down([KeyCode::SuperLeft, KeyCode::SuperRight]);
        // Held with a key, Super makes another chord of it, not the
        // bare key.
        let (primary, ctrl, meta) = if cfg!(target_os = "macos") {
            (command, control, false)
        } else {
            (control, false, command)
        };
        Self {
            primary,
            shift: down([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            alt: down([KeyCode::AltLeft, KeyCode::AltRight]),
            ctrl,
            meta,
        }
    }

    pub const fn with(self, other: Self) -> Self {
        Self {
            primary: self.primary || other.primary,
            shift: self.shift || other.shift,
            alt: self.alt || other.alt,
            ctrl: self.ctrl || other.ctrl,
            meta: self.meta || other.meta,
        }
    }

    /// Whether every modifier of `other` is among these.
    pub const fn has(self, other: Self) -> bool {
        (self.primary || !other.primary)
            && (self.shift || !other.shift)
            && (self.alt || !other.alt)
            && (self.ctrl || !other.ctrl)
            && (self.meta || !other.meta)
    }
}

/// A key, by its place on the keyboard, with the modifiers held as
/// it goes down.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[reflect(Clone, PartialEq)]
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
            (Mods::META, "Super"),
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

/// The bindings in force: the ones commands were registered with,
/// and over them the user's own.
#[derive(Resource, Default)]
pub struct Keymap {
    defaults: Vec<Binding>,
    /// The chords of each command the user rebound, which stand in
    /// for its defaults even when there are none.
    overrides: Vec<(CommandId, Vec<Chord>)>,
}

/// One chord bound to several commands of one scope, of which only
/// the first can answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub chord: Chord,
    pub scope: ScopeId,
    pub commands: Vec<CommandId>,
}

impl Keymap {
    /// The chords bound to `command`, in the order they were bound.
    pub fn chords(
        &self,
        command: CommandId,
    ) -> impl Iterator<Item = Chord> + '_ {
        let own = self.rebound(command);
        let defaults = self
            .defaults
            .iter()
            .filter(move |binding| {
                own.is_none() && binding.command == command
            })
            .map(|binding| binding.chord);
        own.into_iter().flatten().copied().chain(defaults)
    }

    /// The user's own chords for `command`, if they rebound it.
    pub fn rebound(&self, command: CommandId) -> Option<&[Chord]> {
        self.overrides
            .iter()
            .find(|(id, _)| *id == command)
            .map(|(_, chords)| chords.as_slice())
    }

    /// Binds `command` to `chords` and nothing else. With no chords
    /// it is bound to no key.
    pub fn rebind(&mut self, command: CommandId, chords: Vec<Chord>) {
        self.reset(command);
        self.overrides.push((command, chords));
    }

    /// Puts `command` back on the chords it was registered with.
    pub fn reset(&mut self, command: CommandId) {
        self.overrides.retain(|(id, _)| *id != command);
    }

    /// The chords bound to more than one command of a scope.
    pub fn conflicts(&self, list: &CommandList) -> Vec<Conflict> {
        let mut found = Vec::<Conflict>::new();
        for command in list.commands() {
            for chord in self.chords(command.id) {
                let at = found.iter().position(|conflict| {
                    conflict.chord == chord
                        && conflict.scope == command.scope
                });
                match at {
                    Some(at) => found[at].commands.push(command.id),
                    None => found.push(Conflict {
                        chord,
                        scope: command.scope,
                        commands: vec![command.id],
                    }),
                }
            }
        }
        found.retain(|conflict| conflict.commands.len() > 1);
        found
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
        world.resource_mut::<Keymap>().defaults.extend(
            chords.iter().map(|&chord| Binding {
                command: command.id,
                chord,
            }),
        );
        self
    }
}

/// Runs the command `id`, if it is registered and enabled. Whether
/// it ran.
pub fn run_command(
    world: &mut World,
    id: CommandId,
    invoke: Invoke,
) -> bool {
    let command = world
        .get_resource::<CommandList>()
        .and_then(|list| list.get(id).copied());
    match command {
        Some(command) if (command.enabled)(world) => {
            (command.run)(world, invoke);
            true
        }
        _ => false,
    }
}

/// The panel scopes under the pointer, innermost first, or under
/// where it last was over anything.
#[derive(Resource, Default)]
struct Hovered(Vec<(Entity, ScopeId)>);

fn track_hovered(
    hover: Option<Res<HoverMap>>,
    scopes: Query<&Scope>,
    parents: Query<&ChildOf>,
    list: Res<CommandList>,
    mut hovered: ResMut<Hovered>,
) {
    let top = hover
        .as_deref()
        .and_then(|hover| hover.get(&PointerId::Mouse))
        .and_then(|hits| {
            hits.iter()
                .min_by(|(_, a), (_, b)| a.depth.total_cmp(&b.depth))
        })
        .map(|(&node, _)| node);
    // Off the window, the pointer is still where it left.
    let Some(top) = top else {
        return;
    };
    let chain = core::iter::once(top)
        .chain(parents.iter_ancestors(top))
        .filter_map(|node| Some((node, scopes.get(node).ok()?.0)))
        .filter(|&(_, scope)| list.layer(scope) == Layer::Panel)
        .collect::<Vec<_>>();
    if hovered.0 != chain {
        hovered.0 = chain;
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
            id: GLOBAL,
            label: "Everywhere",
            layer: Layer::Global,
            active: None,
        })
        .add_scope(ScopeSpec {
            id: MENU,
            label: "Menu",
            layer: Layer::Modal,
            active: None,
        })
        .init_resource::<Hovered>()
        .add_systems(PreUpdate, track_hovered)
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
    let plain = !(mods.primary || mods.ctrl || mods.alt || mods.meta);
    let in_field =
        focus.get().is_some_and(|held| typing.contains(held));
    if plain && in_field {
        return;
    }
    let repeat = key.repeat;
    commands.queue(move |world: &mut World| {
        // The first of them that is enabled.
        for (id, invoke) in bound(world, chord, repeat) {
            if run_command(world, id, invoke) {
                break;
            }
        }
    });
}

/// The scopes in force, the first to answer first. A gesture under
/// way answers alone, and failing one, whatever is open over
/// everything else. Otherwise the panels under the pointer do,
/// innermost first, and then whatever belongs to no place.
fn in_force(world: &World) -> Vec<(ScopeId, Option<Entity>)> {
    let list = world.resource::<CommandList>();
    let gestures = list
        .scopes
        .iter()
        .filter(|spec| spec.layer == Layer::Gesture)
        .filter(|spec| {
            spec.active.is_some_and(|active| active(world))
        })
        .map(|spec| (spec.id, None))
        .collect::<Vec<_>>();
    if !gestures.is_empty() {
        return gestures;
    }
    let modal = world
        .try_query::<&Scope>()
        .map(|mut open| {
            open.iter(world)
                .filter(|scope| list.layer(scope.0) == Layer::Modal)
                .map(|scope| (scope.0, None))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !modal.is_empty() {
        return modal;
    }
    world
        .resource::<Hovered>()
        .0
        .iter()
        // A panel closed with the pointer off the window is still
        // listed.
        .filter(|&&(node, scope)| {
            world
                .get::<Scope>(node)
                .is_some_and(|open| open.0 == scope)
        })
        .map(|&(node, scope)| (scope, Some(node)))
        .chain([(GLOBAL, None)])
        .collect()
}

/// The commands `chord` is bound to in the scopes in force, in the
/// order they answer.
fn bound(
    world: &World,
    chord: Chord,
    repeat: bool,
) -> Vec<(CommandId, Invoke)> {
    let list = world.resource::<CommandList>();
    let keymap = world.resource::<Keymap>();
    in_force(world)
        .into_iter()
        .flat_map(|(scope, target)| {
            list.commands()
                .iter()
                .filter(move |command| command.scope == scope)
                .filter(|command| {
                    keymap.chords(command.id).any(|own| own == chord)
                })
                .filter(move |command| command.repeat || !repeat)
                .map(move |command| (command.id, Invoke { target }))
        })
        .collect()
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
                run: |world, _| world.resource_mut::<Ran>().0 += 1,
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

    #[test]
    fn a_gesture_under_way_answers_alone() {
        const DRAG: ScopeId = ScopeId("test.drag");
        let mut app = app();
        app.add_scope(ScopeSpec {
            id: DRAG,
            label: "Drag",
            layer: Layer::Gesture,
            // Until its own command has run.
            active: Some(|world| world.resource::<Ran>().0 < 10),
        })
        .add_command(
            CommandSpec {
                id: CommandId("test.cancel"),
                label: "Cancel",
                scope: DRAG,
                run: |world, _| world.resource_mut::<Ran>().0 += 10,
                enabled: |_| true,
                repeat: false,
            },
            &[Chord::key(KeyCode::Escape)],
        );
        let w = || Key::Character("w".into());

        tests::key_down(&mut app, KeyCode::KeyW, w());
        assert_eq!(ran(&app), 0, "the global one is shut out");
        tests::key_down(&mut app, KeyCode::Escape, Key::Escape);
        assert_eq!(ran(&app), 10);
        tests::key_down(&mut app, KeyCode::KeyW, w());
        assert_eq!(ran(&app), 11, "the gesture is over");
    }

    #[test]
    fn a_rebound_command_answers_its_new_key_only() {
        let mut app = app();
        let w = Chord::key(KeyCode::KeyW);
        let e = Chord::key(KeyCode::KeyE);
        app.world_mut()
            .resource_mut::<Keymap>()
            .rebind(BUMP, vec![e]);
        tests::key_down(
            &mut app,
            KeyCode::KeyW,
            Key::Character("w".into()),
        );
        assert_eq!(ran(&app), 0, "its old key is free");
        tests::key_down(
            &mut app,
            KeyCode::KeyE,
            Key::Character("e".into()),
        );
        assert_eq!(ran(&app), 1);

        // A second command of the scope, on the key the first went
        // to.
        app.add_command(
            CommandSpec {
                id: CommandId("test.other"),
                label: "Other",
                scope: GLOBAL,
                run: |_, _| {},
                enabled: |_| true,
                repeat: false,
            },
            &[e],
        );
        let world = app.world_mut();
        let conflicts = world
            .resource::<Keymap>()
            .conflicts(world.resource::<CommandList>());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].commands.len(), 2);

        world.resource_mut::<Keymap>().reset(BUMP);
        let keymap = world.resource::<Keymap>();
        assert_eq!(keymap.chords(BUMP).collect::<Vec<_>>(), [w]);
        assert!(
            keymap
                .conflicts(world.resource::<CommandList>())
                .is_empty()
        );
    }

    #[derive(Resource, Default)]
    struct Aimed(Option<Entity>);

    #[test]
    fn a_panels_key_runs_only_with_the_pointer_over_the_panel() {
        const PANEL: ScopeId = ScopeId("test.panel");
        let mut app = app();
        app.init_resource::<Aimed>()
            .add_scope(ScopeSpec {
                id: PANEL,
                label: "Panel",
                layer: Layer::Panel,
                active: None,
            })
            .add_command(
                CommandSpec {
                    id: CommandId("test.aim"),
                    label: "Aim",
                    scope: PANEL,
                    run: |world, invoke| {
                        world.resource_mut::<Aimed>().0 =
                            invoke.target;
                    },
                    enabled: |_| true,
                    repeat: false,
                },
                // The key the global command is on, which it
                // shadows.
                &[Chord::key(KeyCode::KeyW)],
            );
        let panel = mount::<Plain>(
            app.world_mut(),
            frame().tagged(Scope(PANEL)),
        );
        let w = || Key::Character("w".into());

        tests::key_down(&mut app, KeyCode::KeyW, w());
        assert_eq!(app.world().resource::<Aimed>().0, None);
        assert_eq!(ran(&app), 1, "the global one answers");

        app.world_mut().resource_mut::<Hovered>().0 =
            vec![(panel, PANEL)];
        tests::key_down(&mut app, KeyCode::KeyW, w());
        assert_eq!(app.world().resource::<Aimed>().0, Some(panel));
        assert_eq!(ran(&app), 1, "shadowed by the panel's");
    }
}
