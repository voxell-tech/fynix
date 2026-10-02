//! A box of editable text, and the frame both it and a
//! [`NumberField`](super::NumberField) are drawn in.

use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::world::World;
use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::text::{EditableText, TextCursorStyle};
use bevy::ui::{AlignItems, UiRect, px};
use bevy::window::SystemCursorIcon;

use super::frame::{Frame, FrameProps};
use super::text_input::{TextCommit, TextInput, focus, show};
use crate::cursor::EntityCursor;
use crate::prop::Prop;
use crate::state::State;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
    Tone,
};
use crate::{Bevy, Cx, Focused, Hovered, Styled, View};

/// How many glyphs wide a text field is, when its frame has no
/// width.
const VISIBLE_WIDTH: f32 = 12.0;

/// A single-line editable text box in a [`Frame`].
///
/// While it has focus it shows what the user typed, and a change of
/// the bound value waits until focus leaves. Enter, or focus leaving
/// after an edit, runs the handler with the text. Escape puts the
/// value back and drops focus.
pub struct TextField {
    pub frame: Frame,
    input: TextInput,
    commit: TextCommit,
}

pub fn text_field(
    value: impl Into<Prop<String>>,
    on_commit: impl Fn(&mut World, String) + Send + Sync + 'static,
) -> TextField {
    TextField {
        frame: Frame::unset(),
        input: TextInput::unset().text(value),
        commit: TextCommit(Box::new(on_commit)),
    }
}

impl FrameProps for TextField {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// A field's defaults, for its root frame alone: the fill, a hover
/// fill, and an accent border while focused.
fn defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + 'static,
{
    cx.set::<Frame>(|frame, theme: &T| {
        frame
            .fill(theme.fill())
            .radius(theme.radius())
            .padding(UiRect::horizontal(px(theme.gap())))
            .min_height(px(theme.row()))
            .border(1.0)
            .align(AlignItems::Center)
    });
    cx.when::<State<Hovered>>(|cx| {
        cx.set::<Frame>(|frame, theme: &T| frame.fill(theme.hover()));
    });
    cx.when::<State<Focused>>(|cx| {
        cx.set::<Frame>(|frame, theme: &T| {
            frame.border_color(theme.tone(Tone::Accent))
        });
    });
    cx.transition(Motion::Interact);
}

/// Builds `frame` with `input` in it, as a field's root and its
/// editable text node.
pub(super) fn build_field<T>(
    cx: &mut Cx<'_, Bevy, T>,
    frame: Frame,
    input: TextInput,
    visible_width: f32,
) -> (Entity, Entity)
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
{
    cx.scope(|cx| {
        cx.defaults(|cx| cx.root(defaults));
        let root = cx.build(frame);
        let input = cx.under(root, |cx| cx.build(input));
        let theme = cx.theme();
        let caret = TextCursorStyle {
            color: theme.tone(Tone::Body),
            selection_color: theme.selection(),
            ..TextCursorStyle::default()
        };
        let mut entity = cx.world.entity_mut(input);
        entity.insert(caret);
        if let Some(mut text) = entity.get_mut::<EditableText>() {
            text.visible_width = Some(visible_width);
        }
        (root, input)
    })
}

impl<T> View<Bevy, T> for TextField
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let (root, input) =
            build_field(cx, self.frame, self.input, VISIBLE_WIDTH);
        cx.world.entity_mut(input).insert(self.commit);
        // A press on the frame outside the text focuses it too.
        cx.world
            .entity_mut(root)
            .insert(EntityCursor(SystemCursorIcon::Text))
            .observe(
                move |press: On<Pointer<Press>>,
                      mut commands: Commands| {
                    if press.button == PointerButton::Primary {
                        commands.queue(move |world: &mut World| {
                            focus(world, input);
                        });
                    }
                },
            );
        show(cx.world, input);
        root
    }
}

#[cfg(test)]
pub(super) mod fixtures {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::ecs::hierarchy::Children;
    use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::input_focus::{
        InputDispatchPlugin, InputFocus, InputFocusPlugin,
    };
    use bevy::math::Vec2;
    use bevy::picking::backend::HitData;
    use bevy::picking::events::{Click, Drag, DragEnd};
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::window::PrimaryWindow;

    use super::*;
    pub(crate) use crate::tests::Plain;

    pub fn app() -> App {
        let mut app = crate::tests::app();
        app.add_plugins((
            InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
        ));
        app.world_mut().spawn(PrimaryWindow);
        app
    }

    /// The editable text node under a field's `root`.
    pub fn input_of(app: &App, root: Entity) -> Entity {
        app.world()
            .get::<Children>(root)
            .and_then(|kids| kids.first().copied())
            .expect("an input")
    }

    /// What the input displays.
    pub fn shown(app: &App, input: Entity) -> String {
        app.world()
            .get::<EditableText>(input)
            .expect("editable text")
            .value()
            .to_string()
    }

    pub fn focused(app: &App) -> Option<Entity> {
        app.world().resource::<InputFocus>().get()
    }

    /// Focuses `input` and replaces its text, as typing would.
    pub fn type_into(app: &mut App, input: Entity, text: &str) {
        focus(app.world_mut(), input);
        app.update();
        app.world_mut()
            .get_mut::<EditableText>(input)
            .unwrap()
            .editor_mut()
            .set_text(text);
    }

    /// Presses and releases `key` for the focused entity.
    pub fn press_key(app: &mut App, key: Key, code: KeyCode) {
        let window = app
            .world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        app.update();
        app.update();
    }

    pub fn enter(app: &mut App) {
        press_key(app, Key::Enter, KeyCode::Enter);
    }

    pub fn escape(app: &mut App) {
        press_key(app, Key::Escape, KeyCode::Escape);
    }

    /// Takes focus off whatever has it, as a click elsewhere would.
    pub fn blur(app: &mut App) {
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        app.update();
    }

    fn location() -> Location {
        Location {
            target: NormalizedRenderTarget::None {
                width: 100,
                height: 100,
            },
            position: Vec2::ZERO,
        }
    }

    fn hit() -> HitData {
        HitData::new(Entity::PLACEHOLDER, 0.0, None, None)
    }

    /// A primary-button press on `target`.
    pub fn press(app: &mut App, target: Entity) {
        let event = Press {
            button: PointerButton::Primary,
            hit: hit(),
            count: 1,
        };
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(),
            event,
            target,
        ));
        app.update();
    }

    /// A primary-button drag of `target`, `dx` pixels from where it
    /// began.
    pub fn drag(app: &mut App, target: Entity, dx: f32) {
        let event = Drag {
            button: PointerButton::Primary,
            distance: Vec2::new(dx, 0.0),
            delta: Vec2::ZERO,
        };
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(),
            event,
            target,
        ));
        app.update();
    }

    pub fn drag_end(app: &mut App, target: Entity) {
        let event = DragEnd {
            button: PointerButton::Primary,
            distance: Vec2::ZERO,
        };
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(),
            event,
            target,
        ));
        app.update();
    }

    /// A primary-button click on `target`.
    pub fn click(app: &mut App, target: Entity) {
        let event = Click {
            button: PointerButton::Primary,
            hit: hit(),
            duration: Duration::ZERO,
            count: 1,
        };
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(),
            event,
            target,
        ));
        app.update();
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::resource::Resource;

    use super::fixtures::*;
    use super::*;
    use crate::{mount, resource};

    #[derive(Resource)]
    struct Name(String);

    #[derive(Resource, Default)]
    struct Commits(Vec<String>);

    fn field(app: &mut App) -> Entity {
        app.insert_resource(Name("ann".into()))
            .init_resource::<Commits>();
        mount::<Plain>(
            app.world_mut(),
            text_field(
                resource::<Name, _>(|name| name.0.clone()),
                |world, text| {
                    world.resource_mut::<Commits>().0.push(text);
                },
            ),
        )
    }

    fn commits(app: &App) -> Vec<String> {
        app.world().resource::<Commits>().0.clone()
    }

    #[test]
    fn it_shows_the_bound_value_and_follows_it_while_unfocused() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        assert_eq!(shown(&app, input), "ann");

        app.world_mut().resource_mut::<Name>().0 = "bob".into();
        app.update();

        assert_eq!(shown(&app, input), "bob");
    }

    #[test]
    fn a_world_change_does_not_clobber_an_edit_in_progress() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        type_into(&mut app, input, "typing");

        app.world_mut().resource_mut::<Name>().0 = "bob".into();
        app.update();
        assert_eq!(shown(&app, input), "typing");

        escape(&mut app);
        assert_eq!(shown(&app, input), "bob", "the world's value");
    }

    #[test]
    fn enter_commits_the_typed_text_once_and_drops_focus() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        type_into(&mut app, input, "hello");

        enter(&mut app);

        assert_eq!(commits(&app), ["hello"]);
        assert_eq!(focused(&app), None);
        assert_eq!(
            shown(&app, input),
            "hello",
            "until the world says"
        );
    }

    #[test]
    fn focus_leaving_after_an_edit_commits() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        type_into(&mut app, input, "later");

        blur(&mut app);

        assert_eq!(commits(&app), ["later"]);
    }

    #[test]
    fn focus_leaving_without_an_edit_commits_nothing() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        focus(app.world_mut(), input);
        app.update();

        blur(&mut app);

        assert!(commits(&app).is_empty());
    }

    #[test]
    fn escape_restores_the_value_and_drops_focus_without_committing()
    {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        type_into(&mut app, input, "oops");

        escape(&mut app);

        assert_eq!(shown(&app, input), "ann");
        assert_eq!(focused(&app), None);
        assert!(commits(&app).is_empty());
    }

    #[test]
    fn the_frame_holds_focus_while_its_text_has_it() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);
        assert!(app.world().get::<Focused>(root).is_none());

        focus(app.world_mut(), input);
        app.update();
        assert!(app.world().get::<Focused>(root).is_some());

        blur(&mut app);
        assert!(app.world().get::<Focused>(root).is_none());
    }

    #[test]
    fn the_cursor_is_a_text_cursor_and_the_caret_is_the_body_tone() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);

        assert_eq!(
            app.world().get::<EntityCursor>(root),
            Some(&EntityCursor(SystemCursorIcon::Text))
        );
        let caret = app.world().get::<TextCursorStyle>(input);
        assert_eq!(caret.map(|c| c.color), Some(Color::WHITE));
    }

    #[test]
    fn a_press_on_the_frame_focuses_the_text() {
        let mut app = app();
        let root = field(&mut app);
        let input = input_of(&app, root);

        press(&mut app, root);

        assert_eq!(focused(&app), Some(input));
    }
}
