//! A button showing the chosen option, opening a list of the others.
//!
//! The open and close behaviour is `bevy_ui_widgets`' menu: its
//! button toggles the popup, focus keeps it open, Escape and arrow
//! keys work on the rows. Showing and hiding the popup is
//! [`on_menu_event`].

use bevy::asset::Handle;
use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::input_focus::tab_navigation::{NavAction, TabIndex};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::ui::{AlignItems, Overflow, UiRect, percent, px};
use bevy::ui_widgets::{
    MenuAction, MenuButton, MenuEvent, MenuFocusState,
};

use crate::modifier::ModifierExt;
use crate::prop::{Prop, component};
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens, TextTokens,
    Tone,
};
use crate::views::foldable::Open;
use crate::views::frame::{Frame, FrameProps};
use crate::views::menu::{menu_item, menu_popup};
use crate::views::popup::corners;
use crate::views::{
    BehaviorExt, button, frame, icon, label, menu_bar, row, tint,
};
use crate::{
    AnyView, Bevy, Cx, ScopedExt, Styled, View, ViewExt as _,
};

/// What runs with the index of a chosen option.
type Select = Box<dyn Fn(&mut World, usize) + Send + Sync>;

/// A control showing one of `options`, which opens a list to choose
/// another from. Its frame styles the control, not the list.
pub struct Dropdown {
    pub frame: Frame,
    options: Vec<String>,
    selected: Prop<usize>,
    placeholder: String,
    chevron: Handle<Image>,
    on_select: Select,
}

/// A [`Dropdown`] over `options`, showing the one at `selected`,
/// which may be bound to the world, and ending in a dim `chevron`
/// icon, drawn pointing up, that points down while the list is shut.
/// `on_select` runs with the index
/// of the option chosen; showing it is up to whatever `selected`
/// reads.
pub fn dropdown<S: Into<String>>(
    options: impl IntoIterator<Item = S>,
    selected: impl Into<Prop<usize>>,
    chevron: Handle<Image>,
    on_select: impl Fn(&mut World, usize) + Send + Sync + 'static,
) -> Dropdown {
    Dropdown {
        frame: Frame::unset(),
        options: options.into_iter().map(Into::into).collect(),
        selected: selected.into(),
        placeholder: String::new(),
        chevron,
        on_select: Box::new(on_select),
    }
}

impl Dropdown {
    /// What the control shows while `selected` is past the last
    /// option, as for a dropdown that is a menu of actions with no
    /// choice to keep. Nothing when unset.
    pub fn placeholder(
        mut self,
        placeholder: impl Into<String>,
    ) -> Self {
        self.placeholder = placeholder.into();
        self
    }
}

impl FrameProps for Dropdown {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// On a dropdown's root node: the button that opens its popup, and
/// the popup.
#[derive(Component)]
pub(crate) struct Parts {
    button: Entity,
    popup: Entity,
}

/// How far the up-pointing chevron turns while the list is shut, in
/// degrees, so it points down.
const SHUT_TURN: f32 = 180.0;

/// On a dropdown's root node: what its options run when chosen.
#[derive(Component)]
struct OnSelect(Select);

/// The control's defaults, for its button alone.
fn control_defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SpacingTokens + 'static,
{
    cx.set::<Frame>(|frame, theme: &T| {
        frame
            .min_width(px(72.0))
            .max_width(px(160.0))
            .min_height(px(theme.row()))
            .padding(UiRect::axes(px(theme.gap()), px(0.0)))
            .overflow(Overflow::clip())
    });
}

impl<T> View<Bevy, T> for Dropdown
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
        let Self {
            frame: control,
            options,
            selected,
            placeholder,
            chevron,
            on_select,
        } = self;
        let root = cx.build(frame());
        let shown = {
            let options = options.clone();
            selected.map(move |at| {
                options
                    .get(at)
                    .cloned()
                    .unwrap_or_else(|| placeholder.clone())
            })
        };
        let rows = options
            .into_iter()
            .enumerate()
            .map(|(at, text)| {
                menu_item(label(text).wrap(false))
                    .on_activate(move |world| choose(world, root, at))
            })
            .collect::<Vec<_>>();
        let end = icon(chevron)
            .tone(Tone::Dim)
            .size(cx.theme().small_size())
            .rotation(component::<Open, _>(root, |open| {
                if open.is_some() { 0.0 } else { SHUT_TURN }
            }))
            .transition(Motion::Interact);
        let (button, popup) = cx.under(root, |cx| {
            let button = cx.scope(|cx| {
                cx.defaults(|cx| cx.root(control_defaults));
                let mut button = button(
                    row((label(shown).wrap(false).grown(1.0), end))
                        .grow(1.0),
                );
                button.frame = control;
                cx.build(button)
            });
            cx.world
                .entity_mut(button)
                .insert((MenuButton, TabIndex(0)));
            let popup = cx.build(menu_popup(
                rows,
                percent(100.0),
                corners(LIST_GAP),
                MenuFocusState::Closed,
            ));
            cx.world.entity_mut(popup).insert(Visibility::Hidden);
            (button, popup)
        });
        cx.world
            .entity_mut(root)
            .insert((Parts { button, popup }, OnSelect(on_select)));
        root
    }
}

/// The space between a list and its button.
const LIST_GAP: f32 = 2.0;

/// Runs the handler of the dropdown at `root` with `at`, and closes
/// its popup.
fn choose(world: &mut World, root: Entity, at: usize) {
    let Some(OnSelect(select)) = world
        .get_entity_mut(root)
        .ok()
        .and_then(|mut entity| entity.take::<OnSelect>())
    else {
        return;
    };
    select(world, at);
    let Ok(mut entity) = world.get_entity_mut(root) else {
        return;
    };
    entity.insert(OnSelect(select));
    if let Some(popup) =
        entity.get::<Parts>().map(|parts| parts.popup)
    {
        close(world, popup);
    }
}

fn close(world: &mut World, popup: Entity) {
    if let Ok(mut entity) = world.get_entity_mut(popup) {
        entity.insert((Visibility::Hidden, MenuFocusState::Closed));
    }
    let root = world.get::<ChildOf>(popup).map(ChildOf::parent);
    if let Some(mut root) =
        root.and_then(|r| world.get_entity_mut(r).ok())
    {
        root.remove::<Open>();
    }
}

/// One row of a [`menu_button`]'s list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    /// A row that runs the handler when chosen.
    Item(String),
    /// A heading over the rows after it. It is not chosen, and does
    /// not count in the index the handler gets.
    Section(String),
}

impl MenuEntry {
    /// A heading over the rows after it.
    pub fn section(text: impl Into<String>) -> Self {
        Self::Section(text.into())
    }
}

impl From<&str> for MenuEntry {
    fn from(text: &str) -> Self {
        Self::Item(text.into())
    }
}

impl From<String> for MenuEntry {
    fn from(text: String) -> Self {
        Self::Item(text)
    }
}

/// What a [`MenuTitle`] shows on its button.
enum Face {
    Title(String),
    Icon(Handle<Image>),
}

/// A button that opens a list of actions, as a menu bar's titles do:
/// it shows its title, or an icon, never a choice, and has no
/// chevron. See [`menu_button`].
pub struct MenuTitle {
    pub frame: Frame,
    face: Face,
    entries: Vec<MenuEntry>,
    on_select: Select,
}

/// A [`MenuTitle`] showing `title`, opening a list of `entries`:
/// rows, and the headings between them, see [`MenuEntry`].
/// `on_select` runs with the index of the row chosen, among the rows.
pub fn menu_button<E: Into<MenuEntry>>(
    title: impl Into<String>,
    entries: impl IntoIterator<Item = E>,
    on_select: impl Fn(&mut World, usize) + Send + Sync + 'static,
) -> MenuTitle {
    MenuTitle {
        frame: Frame::unset(),
        face: Face::Title(title.into()),
        entries: entries.into_iter().map(Into::into).collect(),
        on_select: Box::new(on_select),
    }
}

impl MenuTitle {
    /// This, showing `image` instead of a title, in the [`tint`]
    /// look.
    pub fn icon(mut self, image: Handle<Image>) -> Self {
        self.face = Face::Icon(image);
        self
    }
}

impl FrameProps for MenuTitle {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// A heading row: dim, small, on a faint surface, with nothing to
/// choose.
fn section_row<T>(text: String) -> AnyView<Bevy, T>
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + Send
        + Sync
        + 'static,
{
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let theme = cx.theme();
        let (fill, gap, radius, size) = (
            theme.fill(),
            theme.gap(),
            theme.menu_item_radius(),
            theme.small_size(),
        );
        cx.build(
            row((label(text.clone())
                .tone(Tone::Dim)
                .size(size)
                .wrap(false),))
            .width(percent(100.0))
            .align(AlignItems::Center)
            .padding(UiRect::axes(px(gap), px(gap / 2.0)))
            .radius(radius)
            .fill(fill),
        )
    })
}

impl<T> View<Bevy, T> for MenuTitle
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
        let Self {
            frame: control,
            face,
            entries,
            on_select,
        } = self;
        let root = cx.build(frame().height(percent(100.0)));
        let mut chosen = 0;
        let rows = entries
            .into_iter()
            .map(|entry| match entry {
                MenuEntry::Item(text) => {
                    let at = chosen;
                    chosen += 1;
                    menu_item(label(text).wrap(false))
                        .on_activate(move |world| {
                            choose(world, root, at);
                        })
                        .boxed()
                }
                MenuEntry::Section(text) => section_row::<T>(text),
            })
            .collect::<Vec<_>>();
        let (button, popup) = cx.under(root, |cx| {
            let button = match face {
                Face::Title(title) => {
                    let mut button = button(label(title).wrap(false));
                    button.frame = control;
                    cx.build(button.rules(menu_bar))
                }
                Face::Icon(image) => {
                    let mut button = button(icon(image));
                    button.frame = control;
                    cx.build(button.rules(tint))
                }
            };
            cx.world
                .entity_mut(button)
                .insert((MenuButton, TabIndex(0)));
            let width = cx.theme().menu_width();
            let popup = cx.build(menu_popup(
                rows,
                px(width),
                corners(LIST_GAP),
                MenuFocusState::Closed,
            ));
            cx.world.entity_mut(popup).insert(Visibility::Hidden);
            (button, popup)
        });
        cx.world
            .entity_mut(root)
            .insert((Parts { button, popup }, OnSelect(on_select)));
        root
    }
}

/// Opens and closes the popup of the dropdown a [`MenuEvent`] bubbles
/// up to, and keeps [`Open`] on the dropdown in step.
pub(crate) fn on_menu_event(
    mut event: On<MenuEvent>,
    dropdowns: Query<&Parts>,
    popups: Query<&Visibility>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let root = event.event_target();
    let Ok(parts) = dropdowns.get(root) else {
        return;
    };
    let shown = popups
        .get(parts.popup)
        .is_ok_and(|shown| *shown == Visibility::Visible);
    let opens = match event.action {
        MenuAction::Open(nav) => Some(Some(nav)),
        MenuAction::Toggle => {
            Some((!shown).then_some(NavAction::First))
        }
        MenuAction::CloseAll => Some(None),
        MenuAction::FocusRoot => None,
    };
    match opens {
        Some(Some(nav)) => {
            commands.entity(parts.popup).insert((
                Visibility::Visible,
                MenuFocusState::Opening(nav),
            ));
            commands.entity(root).insert(Open);
        }
        Some(None) => {
            commands
                .entity(parts.popup)
                .insert((Visibility::Hidden, MenuFocusState::Closed));
            commands.entity(root).remove::<Open>();
        }
        None => focus.set(parts.button, FocusCause::Navigated),
    }
    event.propagate(false);
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::input::keyboard::{Key, KeyCode};
    use bevy::math::Vec2;
    use bevy::picking::pointer::PointerButton;
    use bevy::ui::widget::{ImageNode, Text};
    use bevy::ui::{Node, UiTransform};
    use bevy::ui_widgets::{Activate, MenuPlugin};
    use bevy::window::Window;

    use super::*;
    use crate::tests::{self, Plain};
    use crate::{mount, resource};

    /// The index the dropdown was last asked to show.
    #[derive(Resource)]
    struct Chosen(usize);

    fn app() -> App {
        let mut app = tests::app();
        app.add_plugins(MenuPlugin).insert_resource(Chosen(1));
        app
    }

    fn pick(app: &mut App) -> Entity {
        mount::<Plain>(
            app.world_mut(),
            dropdown(
                ["Linear", "Ease in", "Ease out"],
                resource::<Chosen, _>(|chosen| chosen.0),
                Handle::default(),
                |world, at| world.resource_mut::<Chosen>().0 = at,
            ),
        )
    }

    fn parts(app: &App, root: Entity) -> (Entity, Entity) {
        let parts = app.world().get::<Parts>(root).unwrap();
        (parts.button, parts.popup)
    }

    fn text(app: &App, node: Entity) -> &str {
        &app.world().get::<Text>(node).expect("a label").0
    }

    fn kids(app: &App, node: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(node)
            .map(|kids| kids.iter().collect())
            .unwrap_or_default()
    }

    /// The text of the label in the control.
    fn shown(app: &App, root: Entity) -> String {
        let (button, _) = parts(app, root);
        let content = kids(app, button)[0];
        text(app, kids(app, content)[0]).to_string()
    }

    fn shut(app: &App, popup: Entity) -> bool {
        app.world().get::<Visibility>(popup)
            == Some(&Visibility::Hidden)
    }

    #[test]
    fn it_starts_shut_showing_the_selected_option() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);

        assert_eq!(shown(&app, root), "Ease in");
        assert!(shut(&app, popup));
        assert!(app.world().get::<MenuButton>(button).is_some());
        assert_eq!(kids(&app, root), [button, popup]);
    }

    #[test]
    fn its_list_holds_a_row_per_option() {
        let mut app = app();
        let root = pick(&mut app);
        let (_, popup) = parts(&app, root);

        let rows = kids(&app, popup)
            .into_iter()
            .map(|row| text(&app, kids(&app, row)[0]).to_string())
            .collect::<Vec<_>>();
        assert_eq!(rows, ["Linear", "Ease in", "Ease out"]);
    }

    #[test]
    fn activating_the_button_opens_the_list_and_again_shuts_it() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);

        app.world_mut().trigger(Activate { entity: button });
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(popup),
            Some(&Visibility::Visible)
        );
        // The menu plugin focuses the first row once it is open.
        assert_eq!(
            app.world().get::<MenuFocusState>(popup),
            Some(&MenuFocusState::Open)
        );
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(kids(&app, popup)[0])
        );

        app.world_mut().trigger(Activate { entity: button });
        app.update();
        assert!(shut(&app, popup));
    }

    #[test]
    fn choosing_a_row_calls_the_handler_and_shuts_the_list() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);
        app.world_mut().trigger(Activate { entity: button });
        app.update();

        let third = kids(&app, popup)[2];
        app.world_mut().trigger(Activate { entity: third });
        app.update();

        assert_eq!(app.world().resource::<Chosen>().0, 2);
        assert!(shut(&app, popup));
    }

    #[test]
    fn the_shown_label_follows_a_bound_selection() {
        let mut app = app();
        let root = pick(&mut app);

        app.world_mut().resource_mut::<Chosen>().0 = 0;
        app.update();

        assert_eq!(shown(&app, root), "Linear");
    }

    #[test]
    fn a_selection_past_the_options_shows_the_placeholder() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            dropdown(
                ["a", "b"],
                resource::<Chosen, _>(|chosen| chosen.0),
                Handle::default(),
                |_, _| {},
            )
            .placeholder("Pick one"),
        );
        app.world_mut().resource_mut::<Chosen>().0 = 2;
        app.update();
        assert_eq!(shown(&app, root), "Pick one");

        app.world_mut().resource_mut::<Chosen>().0 = 0;
        app.update();
        assert_eq!(shown(&app, root), "a");
    }

    #[test]
    fn without_a_placeholder_a_missing_option_shows_nothing() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            dropdown(["a"], 5, Handle::default(), |_, _| {}),
        );

        assert_eq!(shown(&app, root), "");
    }

    #[test]
    fn a_fixed_selection_shows_its_option() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            dropdown(["a", "b"], 1, Handle::default(), |_, _| {}),
        );

        assert_eq!(shown(&app, root), "b");
    }

    #[test]
    fn a_close_from_a_row_reaches_the_dropdown() {
        let mut app = app();
        let root = pick(&mut app);
        let (_, popup) = parts(&app, root);
        app.world_mut()
            .entity_mut(popup)
            .insert(Visibility::Visible);
        let row = kids(&app, popup)[0];

        // What `bevy_ui_widgets` sends when a row is activated or
        // escape is pressed.
        app.world_mut().trigger(MenuEvent {
            source: row,
            action: MenuAction::CloseAll,
        });
        app.update();

        assert!(shut(&app, popup));
    }

    #[test]
    fn a_menu_button_shows_its_title_with_no_chevron_and_runs_an_entry()
     {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            menu_button("File", ["New", "Open"], |world, at| {
                world.resource_mut::<Chosen>().0 = at;
            }),
        );
        let (button, popup) = parts(&app, root);
        assert_eq!(text(&app, kids(&app, button)[0]), "File");
        assert!(
            kids(&app, button).iter().all(|&node| app
                .world()
                .get::<ImageNode>(node)
                .is_none()),
            "a menu title has no chevron"
        );
        assert!(shut(&app, popup));

        open(&mut app, button);
        assert_eq!(
            app.world().get::<Visibility>(popup),
            Some(&Visibility::Visible)
        );
        let open_row = kids(&app, popup)[1];
        app.world_mut().trigger(Activate { entity: open_row });
        app.update();
        assert_eq!(app.world().resource::<Chosen>().0, 1);
        assert!(shut(&app, popup), "choosing shuts the list");
    }

    #[test]
    fn sections_head_the_rows_and_do_not_count_in_the_index() {
        let mut app = app();
        let entries = [
            MenuEntry::section("Cameras"),
            "Camera 2d".into(),
            "Camera 3d".into(),
            MenuEntry::section("Lighting"),
            "Point Light".into(),
        ];
        let root = mount::<Plain>(
            app.world_mut(),
            menu_button("Add", entries, |world, at| {
                world.resource_mut::<Chosen>().0 = at;
            })
            .icon(Handle::default()),
        );
        let (button, popup) = parts(&app, root);
        assert!(
            app.world()
                .get::<ImageNode>(kids(&app, button)[0])
                .is_some(),
            "an icon on the button"
        );

        let rows = kids(&app, popup);
        assert_eq!(rows.len(), 5);
        fn heading(app: &App, node: Entity) -> Option<String> {
            if let Some(text) = app.world().get::<Text>(node) {
                return Some(text.0.clone());
            }
            kids(app, node)
                .into_iter()
                .find_map(|child| heading(app, child))
        }
        assert_eq!(
            heading(&app, rows[0]).as_deref(),
            Some("Cameras")
        );
        assert_eq!(
            heading(&app, rows[3]).as_deref(),
            Some("Lighting")
        );

        open(&mut app, button);
        app.world_mut().trigger(Activate { entity: rows[4] });
        app.update();
        assert_eq!(
            app.world().resource::<Chosen>().0,
            2,
            "the third row, not the fifth entry"
        );
    }

    fn open(app: &mut App, button: Entity) {
        app.world_mut().trigger(Activate { entity: button });
        app.update();
        app.update();
    }

    fn turn(app: &App, root: Entity) -> f32 {
        let (button, _) = parts(app, root);
        let content = kids(app, button)[0];
        let chevron = kids(app, content)[1];
        app.world()
            .get::<UiTransform>(chevron)
            .expect("an icon")
            .rotation
            .as_degrees()
    }

    #[test]
    fn the_chevron_turns_with_the_list() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, _) = parts(&app, root);
        let content = kids(&app, button)[0];
        let chevron = kids(&app, content)[1];
        assert!(app.world().get::<ImageNode>(chevron).is_some());
        // Half a turn reads back as either sign.
        let down = |app: &App| {
            (turn(app, root).abs() - SHUT_TURN).abs() < 0.01
        };
        assert!(down(&app), "pointing down while shut");

        open(&mut app, button);
        for _ in 0..4 {
            app.update();
        }
        assert!(turn(&app, root).abs() < 0.01, "up while open");

        open(&mut app, button);
        for _ in 0..4 {
            app.update();
        }
        assert!(down(&app));
    }

    #[test]
    fn choosing_a_row_turns_the_chevron_back() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);
        open(&mut app, button);
        assert!(app.world().get::<Open>(root).is_some());

        let row = kids(&app, popup)[0];
        app.world_mut().trigger(Activate { entity: row });
        app.update();

        assert!(app.world().get::<Open>(root).is_none());
    }

    #[test]
    fn a_press_elsewhere_shuts_the_list() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);
        open(&mut app, button);
        assert!(!shut(&app, popup));
        let other = app.world_mut().spawn(Node::default()).id();

        tests::pointer_press(
            &mut app,
            other,
            PointerButton::Primary,
            Vec2::ONE,
        );
        app.update();

        assert!(shut(&app, popup));
        assert!(app.world().get::<Open>(root).is_none());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    #[test]
    fn a_press_on_empty_space_shuts_the_list() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);
        open(&mut app, button);
        let window = app.world_mut().spawn(Window::default()).id();

        tests::pointer_press(
            &mut app,
            window,
            PointerButton::Primary,
            Vec2::ONE,
        );
        app.update();

        assert!(shut(&app, popup));
    }

    #[test]
    fn a_press_inside_the_list_or_on_its_button_leaves_it_to_them() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);
        open(&mut app, button);

        let row = kids(&app, popup)[1];
        tests::pointer_press(
            &mut app,
            row,
            PointerButton::Primary,
            Vec2::ONE,
        );
        app.update();
        assert!(!shut(&app, popup));

        // The button's own toggle shuts it, once.
        tests::pointer_press(
            &mut app,
            button,
            PointerButton::Primary,
            Vec2::ONE,
        );
        app.update();
        assert!(!shut(&app, popup));
        app.world_mut().trigger(Activate { entity: button });
        app.update();
        assert!(shut(&app, popup));
    }

    #[test]
    fn escape_on_a_row_shuts_the_list_and_refocuses_the_button() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);
        open(&mut app, button);
        tests::keyboard(&mut app);
        app.update();

        tests::key_down(&mut app, KeyCode::Escape, Key::Escape);
        app.update();

        assert!(shut(&app, popup));
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(button)
        );
    }

    #[test]
    fn leaving_by_escape_gives_the_focus_back_to_the_button() {
        let mut app = app();
        let root = pick(&mut app);
        let (button, popup) = parts(&app, root);

        app.world_mut().trigger(MenuEvent {
            source: popup,
            action: MenuAction::FocusRoot,
        });
        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(button)
        );
    }
}
