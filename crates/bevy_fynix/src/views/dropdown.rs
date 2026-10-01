//! A button showing the chosen option, opening a list of the others.
//!
//! The open and close behaviour is `bevy_ui_widgets`' menu: its
//! button toggles the popup, focus keeps it open, Escape and arrow
//! keys work on the rows. Showing and hiding the popup is
//! [`on_menu_event`].

use bevy::camera::visibility::Visibility;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, EntityCommands, Query, ResMut};
use bevy::ecs::world::World;
use bevy::input_focus::tab_navigation::{NavAction, TabIndex};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::ui::{Overflow, UiRect, percent, px};
use bevy::ui_widgets::popover::{
    PopoverAlign, PopoverPlacement, PopoverSide,
};
use bevy::ui_widgets::{
    MenuAction, MenuButton, MenuEvent, MenuFocusState,
};

use crate::modifier::ModifierExt;
use crate::prop::Prop;
use crate::tokens::{
    MotionTokens, SpacingTokens, SurfaceTokens, TextTokens, Tone,
};
use crate::views::frame::{Frame, FrameProps};
use crate::views::menu::{menu_item, popup};
use crate::views::{BehaviorExt, button, frame, label, row};
use crate::{Bevy, Cx, Styled, View};

/// What runs with the index of a chosen option.
type Select = Box<dyn Fn(&mut World, usize) + Send + Sync>;

/// A control showing one of `options`, which opens a list to choose
/// another from. Its frame styles the control, not the list.
pub struct Dropdown {
    pub frame: Frame,
    options: Vec<String>,
    selected: Prop<usize>,
    on_select: Select,
}

/// A [`Dropdown`] over `options`, showing the one at `selected`,
/// which may be bound to the world. `on_select` runs with the index
/// of the option chosen; showing it is up to whatever `selected`
/// reads.
pub fn dropdown<S: Into<String>>(
    options: impl IntoIterator<Item = S>,
    selected: impl Into<Prop<usize>>,
    on_select: impl Fn(&mut World, usize) + Send + Sync + 'static,
) -> Dropdown {
    Dropdown {
        frame: Frame::unset(),
        options: options.into_iter().map(Into::into).collect(),
        selected: selected.into(),
        on_select: Box::new(on_select),
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
            on_select,
        } = self;
        let root = cx.build(frame());
        let shown = {
            let options = options.clone();
            selected.map(move |at| {
                options.get(at).cloned().unwrap_or_default()
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
        let (button, popup) = cx.under(root, |cx| {
            let button = cx.scope(|cx| {
                cx.defaults(|cx| cx.root(control_defaults));
                let mut button = button(
                    row((
                        label(shown).wrap(false).grown(1.0),
                        label("v").tone(Tone::Dim),
                    ))
                    .grow(1.0),
                );
                button.frame = control;
                cx.build(button)
            });
            cx.world
                .entity_mut(button)
                .insert((MenuButton, TabIndex(0)));
            let popup = cx.build(popup(
                rows,
                percent(100.0),
                vec![
                    placement(PopoverSide::Bottom),
                    placement(PopoverSide::Top),
                ],
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

fn placement(side: PopoverSide) -> PopoverPlacement {
    PopoverPlacement {
        side,
        align: PopoverAlign::Start,
        gap: 2.0,
    }
}

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
}

/// Opens and closes the popup of the dropdown a [`MenuEvent`] bubbles
/// up to.
pub(crate) fn on_menu_event(
    mut event: On<MenuEvent>,
    dropdowns: Query<&Parts>,
    popups: Query<&Visibility>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Ok(parts) = dropdowns.get(event.event_target()) else {
        return;
    };
    let mut popup = commands.entity(parts.popup);
    let open = |popup: &mut EntityCommands, nav| {
        popup.insert((
            Visibility::Visible,
            MenuFocusState::Opening(nav),
        ));
    };
    match event.action {
        MenuAction::Open(nav) => open(&mut popup, nav),
        MenuAction::Toggle => {
            if popups
                .get(parts.popup)
                .is_ok_and(|shown| *shown == Visibility::Visible)
            {
                popup.insert((
                    Visibility::Hidden,
                    MenuFocusState::Closed,
                ));
            } else {
                open(&mut popup, NavAction::First);
            }
        }
        MenuAction::CloseAll => {
            popup
                .insert((Visibility::Hidden, MenuFocusState::Closed));
        }
        MenuAction::FocusRoot => {
            focus.set(parts.button, FocusCause::Navigated);
        }
    }
    event.propagate(false);
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::ui::widget::Text;
    use bevy::ui_widgets::{Activate, MenuPlugin};

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
    fn a_fixed_selection_shows_its_option() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            dropdown(["a", "b"], 1, |_, _| {}),
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
