//! A number edited by dragging across it, or by typing into it.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::system::Commands;
use bevy::ecs::world::World;
use bevy::input_focus::{FocusGained, FocusLost};
use bevy::picking::Pickable;
use bevy::picking::events::{Click, Drag, DragEnd, Pointer};
use bevy::picking::pointer::PointerButton;
use bevy::text::{EditableText, TextEdit};
use bevy::window::SystemCursorIcon;

use super::field::build_field;
use super::frame::{Frame, FrameProps};
use super::text_input::{
    Entry, NumberChange, NumberSpec, TextInput, call, focus,
    is_editing, show,
};
use crate::cursor::EntityCursor;
use crate::prop::Prop;
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
};
use crate::{Bevy, Cx, Styled, View};

/// How far a press must move sideways before it drags rather than
/// clicks, in logical pixels.
const SLOP: f32 = 3.0;

/// How many glyphs wide a number field is, when its frame has no
/// width.
const VISIBLE_WIDTH: f32 = 8.0;

const DRAG_CURSOR: EntityCursor =
    EntityCursor(SystemCursorIcon::EwResize);
const TYPE_CURSOR: EntityCursor =
    EntityCursor(SystemCursorIcon::Text);

/// An `f64` in a [`Frame`], with two modes. In drag mode, which it
/// starts in, dragging sideways moves the value by a step per pixel.
/// A click without a drag, or tabbing in, switches to input mode: the
/// value is typed like a [`TextField`](super::TextField) and read
/// when Enter is pressed or focus leaves, and dragging does nothing.
/// What cannot be read as a number puts the old value back.
pub struct NumberField {
    pub frame: Frame,
    input: TextInput,
    change: NumberChange,
    spec: NumberSpec,
}

pub fn number_field(
    value: impl Into<Prop<f64>>,
    on_change: impl Fn(&mut World, f64) + Send + Sync + 'static,
) -> NumberField {
    NumberField {
        frame: Frame::unset(),
        input: TextInput::unset().number(value),
        change: NumberChange(Box::new(on_change)),
        spec: NumberSpec {
            step: 0.01,
            precision: None,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
        },
    }
}

impl NumberField {
    /// How much one pixel of dragging changes the value. 0.01 when
    /// unset.
    pub fn step(mut self, step: f64) -> Self {
        self.spec.step = step;
        self
    }

    /// How many decimals the value is shown with. As many as it
    /// needs when unset.
    pub fn precision(mut self, precision: usize) -> Self {
        self.spec.precision = Some(precision);
        self
    }

    /// The least and the most the value can be, whether dragged or
    /// typed.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.spec.min = min;
        self.spec.max = max;
        self
    }
}

impl FrameProps for NumberField {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// A drag under way on a field's root.
#[derive(Component)]
struct Scrub {
    /// The value when the drag began.
    start: f64,
    /// The value it last reached.
    last: f64,
}

/// Moves the value `dx` pixels' worth from where the drag began, once
/// the drag is past the slop. Input mode leaves it alone.
fn scrub(world: &mut World, root: Entity, input: Entity, dx: f32) {
    if is_editing(world, input) {
        return;
    }
    let Some(spec) = world.get::<NumberSpec>(input).copied() else {
        return;
    };
    let (start, last) = match world.get::<Scrub>(root) {
        Some(scrub) => (scrub.start, scrub.last),
        None => {
            if dx.abs() < SLOP {
                return;
            }
            let start = world
                .get::<Entry>(input)
                .and_then(|entry| entry.number)
                .unwrap_or(0.0);
            world
                .entity_mut(root)
                .insert(Scrub { start, last: start });
            (start, start)
        }
    };
    let value = spec.clamp(start + f64::from(dx).round() * spec.step);
    if value == last {
        return;
    }
    if let Some(mut scrub) = world.get_mut::<Scrub>(root) {
        scrub.last = value;
    }
    if let Some(mut entry) = world.get_mut::<Entry>(input) {
        entry.number = Some(value);
    }
    show(world, input);
    call::<NumberChange>(world, input, |change, world| {
        (change.0)(world, value);
    });
}

/// Switches to input mode, with the whole value selected to type
/// over, unless the click ended a drag.
fn start_typing(world: &mut World, root: Entity, input: Entity) {
    if world.get::<Scrub>(root).is_some() || is_editing(world, input)
    {
        return;
    }
    focus(world, input);
    if let Some(mut text) = world.get_mut::<EditableText>(input) {
        text.queue_edit(TextEdit::SelectAll);
    }
}

/// Puts the field in input mode, where its text takes the pointer
/// and the cursor, or back in drag mode.
fn typing(world: &mut World, root: Entity, input: Entity, on: bool) {
    if let Ok(mut input) = world.get_entity_mut(input) {
        input.insert(if on {
            Pickable::default()
        } else {
            Pickable::IGNORE
        });
    }
    if let Ok(mut root) = world.get_entity_mut(root) {
        root.insert(if on { TYPE_CURSOR } else { DRAG_CURSOR });
    }
}

impl<T> View<Bevy, T> for NumberField
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
        let world = &mut *cx.world;
        world.entity_mut(input).insert((
            self.change,
            self.spec,
            Pickable::IGNORE,
        ));
        world
            .entity_mut(input)
            .observe(
                move |_: On<FocusGained>, mut commands: Commands| {
                    commands.queue(move |world: &mut World| {
                        typing(world, root, input, true);
                    });
                },
            )
            .observe(
                move |_: On<FocusLost>, mut commands: Commands| {
                    commands.queue(move |world: &mut World| {
                        typing(world, root, input, false);
                    });
                },
            );
        world
            .entity_mut(root)
            .insert(DRAG_CURSOR)
            .observe(
                move |drag: On<Pointer<Drag>>,
                      mut commands: Commands| {
                    if drag.button != PointerButton::Primary {
                        return;
                    }
                    let dx = drag.distance.x;
                    commands.queue(move |world: &mut World| {
                        scrub(world, root, input, dx);
                    });
                },
            )
            .observe(
                move |_: On<Pointer<DragEnd>>,
                      mut commands: Commands| {
                    commands.queue(move |world: &mut World| {
                        if let Ok(mut root) =
                            world.get_entity_mut(root)
                        {
                            root.remove::<Scrub>();
                        }
                    });
                },
            )
            .observe(
                move |click_: On<Pointer<Click>>,
                      mut commands: Commands| {
                    if click_.button != PointerButton::Primary {
                        return;
                    }
                    commands.queue(move |world: &mut World| {
                        start_typing(world, root, input);
                    });
                },
            );
        show(world, input);
        root
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::resource::Resource;

    use super::super::field::testing::*;
    use super::*;
    use crate::{mount, resource};

    #[derive(Resource)]
    struct Level(f64);

    #[derive(Resource, Default)]
    struct Changes(Vec<f64>);

    fn field(
        app: &mut App,
        build: impl FnOnce(NumberField) -> NumberField,
    ) -> Entity {
        app.insert_resource(Level(5.0)).init_resource::<Changes>();
        let field = number_field(
            resource::<Level, _>(|level| level.0),
            |world, value| {
                world.resource_mut::<Level>().0 = value;
                world.resource_mut::<Changes>().0.push(value);
            },
        );
        mount::<Plain>(app.world_mut(), build(field))
    }

    fn changes(app: &App) -> Vec<f64> {
        app.world().resource::<Changes>().0.clone()
    }

    fn level(app: &App) -> f64 {
        app.world().resource::<Level>().0
    }

    fn cursor(app: &App, root: Entity) -> SystemCursorIcon {
        app.world().get::<EntityCursor>(root).unwrap().0
    }

    #[test]
    fn it_shows_the_bound_value_with_its_precision() {
        let mut app = app();
        let root = field(&mut app, |f| f.precision(2));
        let input = input_of(&app, root);
        assert_eq!(shown(&app, input), "5.00");

        app.world_mut().resource_mut::<Level>().0 = 1.239;
        app.update();

        assert_eq!(shown(&app, input), "1.24");
    }

    #[test]
    fn a_drag_changes_the_value_by_a_step_per_pixel() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(0.5));
        let input = input_of(&app, root);

        drag(&mut app, root, 10.0);
        assert_eq!(changes(&app), [10.0]);
        assert_eq!(shown(&app, input), "10");

        drag(&mut app, root, 4.0);
        assert_eq!(changes(&app), [10.0, 7.0], "from where it began");
        assert_eq!(level(&app), 7.0);
        assert_eq!(shown(&app, input), "7");
    }

    #[test]
    fn a_drag_ends_and_the_next_begins_from_the_new_value() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(1.0));

        drag(&mut app, root, 10.0);
        drag_end(&mut app, root);
        drag(&mut app, root, 4.0);

        assert_eq!(level(&app), 19.0);
    }

    #[test]
    fn a_drag_is_held_to_the_range() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(1.0).range(0.0, 8.0));

        drag(&mut app, root, 50.0);
        assert_eq!(level(&app), 8.0);
        drag(&mut app, root, -50.0);
        assert_eq!(level(&app), 0.0);
        assert_eq!(changes(&app), [8.0, 0.0]);
    }

    #[test]
    fn a_wobble_under_the_slop_changes_nothing() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(1.0));

        drag(&mut app, root, 2.0);

        assert!(changes(&app).is_empty());
    }

    #[test]
    fn it_is_in_drag_mode_to_begin_with() {
        let mut app = app();
        let root = field(&mut app, |f| f);
        let input = input_of(&app, root);

        assert_eq!(cursor(&app, root), SystemCursorIcon::EwResize);
        assert_eq!(
            app.world().get::<Pickable>(input),
            Some(&Pickable::IGNORE)
        );
    }

    #[test]
    fn a_click_enters_input_mode_where_a_drag_changes_nothing() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(1.0));
        let input = input_of(&app, root);

        click(&mut app, root);
        app.update();
        assert_eq!(focused(&app), Some(input));
        assert_eq!(cursor(&app, root), SystemCursorIcon::Text);
        assert_eq!(
            app.world().get::<Pickable>(input),
            Some(&Pickable::default())
        );

        drag(&mut app, root, 20.0);

        assert!(changes(&app).is_empty());
        assert_eq!(shown(&app, input), "5");
    }

    #[test]
    fn a_click_ending_a_drag_stays_in_drag_mode() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(1.0));
        let input = input_of(&app, root);

        drag(&mut app, root, 20.0);
        click(&mut app, root);
        drag_end(&mut app, root);

        assert_ne!(focused(&app), Some(input));
        assert_eq!(level(&app), 25.0);
    }

    #[test]
    fn leaving_input_mode_goes_back_to_dragging() {
        let mut app = app();
        let root = field(&mut app, |f| f.step(1.0));

        click(&mut app, root);
        app.update();
        blur(&mut app);
        assert_eq!(cursor(&app, root), SystemCursorIcon::EwResize);

        drag(&mut app, root, 10.0);
        assert_eq!(level(&app), 15.0);
    }

    #[test]
    fn enter_reads_the_typed_number_and_holds_it_to_the_range() {
        let mut app = app();
        let root = field(&mut app, |f| f.range(0.0, 10.0));
        let input = input_of(&app, root);

        type_into(&mut app, input, "7.5");
        enter(&mut app);
        assert_eq!(changes(&app), [7.5]);
        assert_eq!(shown(&app, input), "7.5");
        assert_eq!(focused(&app), None);

        type_into(&mut app, input, "70");
        enter(&mut app);
        assert_eq!(changes(&app), [7.5, 10.0]);
        assert_eq!(shown(&app, input), "10");
    }

    #[test]
    fn focus_leaving_reads_the_typed_number() {
        let mut app = app();
        let root = field(&mut app, |f| f);
        let input = input_of(&app, root);
        type_into(&mut app, input, "3");

        blur(&mut app);

        assert_eq!(changes(&app), [3.0]);
    }

    #[test]
    fn an_unparseable_entry_restores_the_previous_value() {
        let mut app = app();
        let root = field(&mut app, |f| f.precision(1));
        let input = input_of(&app, root);
        type_into(&mut app, input, "abc");

        enter(&mut app);

        assert!(changes(&app).is_empty());
        assert_eq!(shown(&app, input), "5.0");

        type_into(&mut app, input, "nan");
        blur(&mut app);

        assert!(changes(&app).is_empty());
        assert_eq!(shown(&app, input), "5.0");
    }

    #[test]
    fn enter_without_an_edit_leaves_a_rounded_display_alone() {
        let mut app = app();
        let root = field(&mut app, |f| f.precision(1));
        let input = input_of(&app, root);
        app.world_mut().resource_mut::<Level>().0 = 1.2345;
        app.update();
        focus(app.world_mut(), input);
        app.update();

        enter(&mut app);

        assert!(changes(&app).is_empty());
        assert_eq!(level(&app), 1.2345);
    }

    #[test]
    fn escape_restores_the_value() {
        let mut app = app();
        let root = field(&mut app, |f| f);
        let input = input_of(&app, root);
        type_into(&mut app, input, "9");

        escape(&mut app);

        assert!(changes(&app).is_empty());
        assert_eq!(shown(&app, input), "5");
    }
}
