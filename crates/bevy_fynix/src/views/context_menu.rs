//! A menu opened at the pointer by a right-click.

use core::marker::PhantomData;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::input_focus::tab_navigation::NavAction;
use bevy::math::Vec2;
use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::ui::{Pressed, UiScale, px};
use bevy::ui_widgets::{MenuAction, MenuEvent, MenuFocusState};

use crate::prop::component;
use crate::tokens::{MotionTokens, SpacingTokens, SurfaceTokens};
use crate::views::menu::{Floating, FocusOn, float, menu_popup};
use crate::views::popup::corners;
use crate::{AnyView, Bevy, Cx, View, ViewSeq};

/// On a node with a context menu while it is open: where it was
/// opened, in logical pixels, and which opening of the node it is.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct OpenAt {
    at: Vec2,
    serial: u32,
}

/// On a node with a context menu: how many times it has opened
/// one, which outlives each opening so that serials stay distinct.
#[derive(Component)]
struct Openings(u32);

/// On a context menu's popup: the opening it was built for.
#[derive(Component)]
pub(crate) struct OpenedFor(u32);

/// A view whose root node opens a menu at the pointer on a
/// right-click.
pub struct ContextMenu<V, F, S> {
    inner: V,
    items: F,
    rows: PhantomData<fn() -> S>,
}

/// What any view can be given a context menu with.
pub trait ContextMenuExt: Sized {
    /// This view, opening a menu of the rows `items` returns, under
    /// the pointer, on a right-click. `items` runs each time the
    /// menu opens, and the menu is built then and despawned when it
    /// is dismissed: by a click elsewhere, by Escape, or by choosing
    /// a row.
    ///
    /// ```ignore
    /// row_view.context_menu(|| {
    ///     (
    ///         menu_item(label("Rename")).on_activate(rename),
    ///         menu_item(label("Delete")).on_activate(delete),
    ///     )
    /// })
    /// ```
    fn context_menu<F, S>(self, items: F) -> ContextMenu<Self, F, S>
    where
        F: Fn() -> S + Send + Sync + 'static,
    {
        ContextMenu {
            inner: self,
            items,
            rows: PhantomData,
        }
    }
}

impl<V> ContextMenuExt for V {}

fn open(
    mut press: On<Pointer<Press>>,
    scale: Option<Res<UiScale>>,
    openings: Query<&Openings>,
    mut commands: Commands,
) {
    if press.button != PointerButton::Secondary {
        return;
    }
    press.propagate(false);
    let scale = scale.map_or(1.0, |scale| scale.0);
    let source = press.event_target();
    let serial = openings
        .get(source)
        .map_or(0, |count| count.0.wrapping_add(1));
    commands
        .entity(source)
        .insert((
            OpenAt {
                at: press.pointer_location.position / scale,
                serial,
            },
            Openings(serial),
        ))
        // A button is pressed by any pointer button, and letting go
        // would activate it under its own menu.
        .remove::<Pressed>();
}

/// Closes the context menu a close request bubbles up to, unless the
/// request comes from a menu the node has since reopened: the press
/// that moves a menu closes the old one.
pub(crate) fn dismiss(
    mut event: On<MenuEvent>,
    hosts: Query<&Floating>,
    opened: Query<&OpenAt>,
    anchors: Query<&Children>,
    stamps: Query<&OpenedFor>,
    mut commands: Commands,
) {
    let anchor = event.event_target();
    let Ok(Floating(source)) = hosts.get(anchor) else {
        return;
    };
    if !matches!(event.action, MenuAction::CloseAll) {
        return;
    }
    event.propagate(false);
    // The anchor holds the popup of one opening.
    let stale = anchors
        .iter_descendants(anchor)
        .find_map(|node| stamps.get(node).ok())
        .is_some_and(|stamp| {
            opened
                .get(*source)
                .is_ok_and(|open| open.serial != stamp.0)
        });
    if !stale {
        commands.entity(*source).remove::<OpenAt>();
    }
}

impl<T, V, F, S> View<Bevy, T> for ContextMenu<V, F, S>
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    V: View<Bevy, T>,
    F: Fn() -> S + Send + Sync + 'static,
    S: ViewSeq<Bevy, T> + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let source = self.inner.build(cx);
        cx.world.entity_mut(source).observe(open);
        let items = self.items;
        float::<T, OpenAt, OpenAt, u32>(
            cx,
            source,
            |open| open.at,
            component::<OpenAt, _>(source, |open| {
                open.map(|open| vec![*open]).unwrap_or_default()
            }),
            |open| open.serial,
            move |open| menu(open.serial, items()),
        );
        source
    }
}

/// The rows on a surface, placed against the point they open at,
/// whichever corner has room, for the opening `serial`.
fn menu<T, S>(serial: u32, rows: S) -> AnyView<Bevy, T>
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    S: ViewSeq<Bevy, T> + 'static,
{
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let surface = menu_popup(
            rows,
            px(cx.theme().menu_width()),
            corners(0.0),
            MenuFocusState::Closed,
        );
        let node = cx.build(surface);
        cx.world
            .entity_mut(node)
            .insert((FocusOn(NavAction::First), OpenedFor(serial)));
        node
    })
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::resource::Resource;
    use bevy::input::keyboard::{Key, KeyCode};
    use bevy::input_focus::InputFocus;
    use bevy::ui::Node;
    use bevy::ui::widget::Text;
    use bevy::ui_widgets::popover::Popover;
    use bevy::ui_widgets::{Activate, MenuPlugin, MenuPopup};
    use bevy::window::Window;

    use super::*;
    use crate::mount;
    use crate::tests::{self, Plain, kids};
    use crate::views::{BehaviorExt, label, menu_item};

    #[derive(Resource, Default)]
    struct Deleted(u32);

    fn app() -> App {
        let mut app = tests::app();
        app.add_plugins(MenuPlugin).init_resource::<Deleted>();
        app
    }

    fn source(app: &mut App) -> Entity {
        mount::<Plain>(
            app.world_mut(),
            label("row").context_menu(|| {
                (
                    menu_item(label("Rename")),
                    menu_item(label("Delete")).on_activate(|world| {
                        world.resource_mut::<Deleted>().0 += 1;
                    }),
                )
            }),
        )
    }

    fn press(
        app: &mut App,
        on: Entity,
        button: PointerButton,
        at: Vec2,
    ) {
        send_press(app, on, button, at);
        app.update();
    }

    fn send_press(
        app: &mut App,
        on: Entity,
        button: PointerButton,
        at: Vec2,
    ) {
        tests::pointer_press(app, on, button, at);
    }

    /// The popups of every menu open.
    fn popups(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<MenuPopup>>()
            .iter(app.world())
            .collect()
    }

    fn texts(app: &App, popup: Entity) -> Vec<String> {
        kids(app, popup)
            .into_iter()
            .map(|row| {
                let label = kids(app, row)[0];
                app.world().get::<Text>(label).unwrap().0.clone()
            })
            .collect()
    }

    #[test]
    fn nothing_is_built_until_the_right_press() {
        let mut app = app();
        let source = source(&mut app);

        assert!(popups(&mut app).is_empty());
        press(
            &mut app,
            source,
            PointerButton::Primary,
            Vec2::new(10.0, 20.0),
        );
        assert!(popups(&mut app).is_empty());
    }

    #[test]
    fn a_right_press_opens_the_rows_at_the_pointer() {
        let mut app = app();
        let source = source(&mut app);

        press(
            &mut app,
            source,
            PointerButton::Secondary,
            Vec2::new(10.0, 20.0),
        );

        let popup = popups(&mut app);
        assert_eq!(popup.len(), 1);
        assert_eq!(texts(&app, popup[0]), ["Rename", "Delete"]);
        let anchor = app
            .world()
            .get::<bevy::ecs::hierarchy::ChildOf>(popup[0])
            .unwrap()
            .parent();
        let ui = app.world().get::<Node>(anchor).unwrap();
        assert_eq!((ui.left, ui.top), (px(10.0), px(20.0)));
        assert!(app.world().get::<Popover>(popup[0]).is_some());
        assert!(
            app.world()
                .get::<bevy::ecs::hierarchy::ChildOf>(anchor)
                .is_none(),
            "hangs at the root of the window"
        );
    }

    #[test]
    fn a_right_press_opens_the_menu_and_leaves_a_button_unpressed() {
        let mut app = app();
        app.add_plugins(bevy::ui_widgets::ButtonPlugin);
        let source = mount::<Plain>(
            app.world_mut(),
            crate::views::button(label("row"))
                .context_menu(|| (menu_item(label("Delete")),)),
        );

        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);

        assert_eq!(popups(&mut app).len(), 1);
        assert!(app.world().get::<Pressed>(source).is_none());
    }

    #[test]
    fn the_first_row_takes_the_focus() {
        let mut app = app();
        let source = source(&mut app);

        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);

        let popup = popups(&mut app)[0];
        let first = kids(&app, popup)[0];
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(first)
        );
        assert_eq!(
            app.world().get::<MenuFocusState>(popup),
            Some(&MenuFocusState::Open)
        );
    }

    #[test]
    fn a_dismissal_despawns_the_menu() {
        let mut app = app();
        let source = source(&mut app);
        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);
        let popup = popups(&mut app)[0];

        // What `bevy_ui_widgets` sends on a click elsewhere or on
        // escape.
        app.world_mut().trigger(MenuEvent {
            source: popup,
            action: MenuAction::CloseAll,
        });
        app.update();

        assert!(popups(&mut app).is_empty());
        assert!(app.world().get_entity(popup).is_err());
    }

    #[test]
    fn choosing_a_row_runs_it_and_despawns_the_menu() {
        let mut app = app();
        let source = source(&mut app);
        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);
        let popup = popups(&mut app)[0];
        let delete = kids(&app, popup)[1];

        // The row's activation, then what `bevy_ui_widgets` sends
        // after it.
        app.world_mut().trigger(Activate { entity: delete });
        app.world_mut().trigger(MenuEvent {
            source: delete,
            action: MenuAction::CloseAll,
        });
        app.update();

        assert_eq!(app.world().resource::<Deleted>().0, 1);
        assert!(popups(&mut app).is_empty());
    }

    #[test]
    fn it_opens_again_after_a_dismissal() {
        let mut app = app();
        let source = source(&mut app);
        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);
        let popup = popups(&mut app)[0];
        app.world_mut().trigger(MenuEvent {
            source: popup,
            action: MenuAction::CloseAll,
        });
        app.update();

        press(
            &mut app,
            source,
            PointerButton::Secondary,
            Vec2::new(5.0, 5.0),
        );

        assert_eq!(popups(&mut app).len(), 1);
    }

    #[test]
    fn a_second_right_press_moves_the_menu() {
        let mut app = app();
        let source = source(&mut app);
        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);

        // The press takes the focus from the open menu, which asks
        // to close once the new position is set.
        let old = popups(&mut app)[0];
        send_press(
            &mut app,
            source,
            PointerButton::Secondary,
            Vec2::new(30.0, 40.0),
        );
        app.world_mut().flush();
        app.world_mut().trigger(MenuEvent {
            source: old,
            action: MenuAction::CloseAll,
        });
        app.update();
        app.update();

        let popup = popups(&mut app);
        assert_eq!(popup.len(), 1);
        let anchor = app
            .world()
            .get::<bevy::ecs::hierarchy::ChildOf>(popup[0])
            .unwrap()
            .parent();
        let ui = app.world().get::<Node>(anchor).unwrap();
        assert_eq!((ui.left, ui.top), (px(30.0), px(40.0)));
    }

    /// Opens a menu on a fresh source, with nothing but pointer
    /// events and frames, as a run does.
    fn open_one(app: &mut App) -> Entity {
        let source = source(app);
        press(app, source, PointerButton::Secondary, Vec2::ZERO);
        app.update();
        assert_eq!(popups(app).len(), 1);
        source
    }

    #[test]
    fn a_press_on_empty_space_dismisses_the_menu() {
        let mut app = app();
        open_one(&mut app);
        let window = app.world_mut().spawn(Window::default()).id();

        press(&mut app, window, PointerButton::Primary, Vec2::ONE);
        app.update();

        assert!(popups(&mut app).is_empty());
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    #[test]
    fn a_press_on_another_widget_dismisses_the_menu() {
        let mut app = app();
        open_one(&mut app);
        let other = app.world_mut().spawn(Node::default()).id();

        press(&mut app, other, PointerButton::Primary, Vec2::ONE);
        app.update();

        assert!(popups(&mut app).is_empty());
    }

    #[test]
    fn a_press_on_the_source_dismisses_the_menu() {
        let mut app = app();
        let source = open_one(&mut app);

        press(&mut app, source, PointerButton::Primary, Vec2::ONE);
        app.update();

        assert!(popups(&mut app).is_empty());
    }

    #[test]
    fn a_press_inside_the_menu_leaves_it_open() {
        let mut app = app();
        open_one(&mut app);
        let popup = popups(&mut app)[0];
        let row = kids(&app, popup)[0];

        press(&mut app, row, PointerButton::Primary, Vec2::ONE);
        app.update();

        assert_eq!(popups(&mut app).len(), 1);
    }

    #[test]
    fn a_right_press_elsewhere_dismisses_and_on_the_source_moves() {
        let mut app = app();
        let source = open_one(&mut app);
        let other = app.world_mut().spawn(Node::default()).id();
        press(&mut app, other, PointerButton::Secondary, Vec2::ONE);
        app.update();
        assert!(popups(&mut app).is_empty());

        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);
        app.update();
        assert_eq!(popups(&mut app).len(), 1);

        press(
            &mut app,
            source,
            PointerButton::Secondary,
            Vec2::new(30.0, 40.0),
        );
        app.update();
        app.update();

        let popup = popups(&mut app);
        assert_eq!(popup.len(), 1);
        let anchor = app
            .world()
            .get::<bevy::ecs::hierarchy::ChildOf>(popup[0])
            .unwrap()
            .parent();
        let ui = app.world().get::<Node>(anchor).unwrap();
        assert_eq!((ui.left, ui.top), (px(30.0), px(40.0)));
    }

    #[test]
    fn escape_on_a_row_dismisses_the_menu() {
        let mut app = app();
        open_one(&mut app);
        tests::keyboard(&mut app);
        app.update();

        tests::key_down(&mut app, KeyCode::Escape, Key::Escape);
        app.update();

        assert!(popups(&mut app).is_empty());
    }

    #[test]
    fn the_menu_goes_with_its_source() {
        let mut app = app();
        let source = source(&mut app);
        press(&mut app, source, PointerButton::Secondary, Vec2::ZERO);
        assert_eq!(popups(&mut app).len(), 1);

        app.world_mut().entity_mut(source).despawn();
        app.update();
        app.update();

        assert!(popups(&mut app).is_empty());
    }
}
