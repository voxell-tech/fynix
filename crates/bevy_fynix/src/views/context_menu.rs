//! A menu opened at the pointer by a right-click.

use core::marker::PhantomData;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::Vec2;
use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::ui::{UiScale, px};
use bevy::ui_widgets::popover::{
    PopoverAlign, PopoverPlacement, PopoverSide,
};
use bevy::ui_widgets::{MenuAction, MenuEvent, MenuFocusState};

use crate::prop::component;
use crate::tokens::{SpacingTokens, SurfaceTokens};
use crate::views::menu::{Floating, FocusFirst, float, popup};
use crate::{AnyView, Bevy, Cx, View, ViewSeq};

/// The least width of a context menu.
const MIN_WIDTH: f32 = 120.0;

/// On a node with a context menu while it is open: where it was
/// opened, in logical pixels, and which opening of the node it is.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct OpenAt {
    at: Vec2,
    serial: u32,
}

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
    opened: Query<&OpenAt>,
    mut commands: Commands,
) {
    if press.button != PointerButton::Secondary {
        return;
    }
    press.propagate(false);
    let scale = scale.map_or(1.0, |scale| scale.0);
    let source = press.event_target();
    let serial = opened
        .get(source)
        .map_or(0, |open| open.serial.wrapping_add(1));
    commands.entity(source).insert(OpenAt {
        at: press.pointer_location.position / scale,
        serial,
    });
}

/// Closes the context menu a close request bubbles up to, unless the
/// request comes from a menu the node has since reopened: the press
/// that moves a menu takes the focus from the old one.
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

fn placement(
    side: PopoverSide,
    align: PopoverAlign,
) -> PopoverPlacement {
    PopoverPlacement {
        side,
        align,
        gap: 0.0,
    }
}

impl<T, V, F, S> View<Bevy, T> for ContextMenu<V, F, S>
where
    T: SurfaceTokens + SpacingTokens + Send + Sync + 'static,
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
    T: SurfaceTokens + SpacingTokens + Send + Sync + 'static,
    S: ViewSeq<Bevy, T> + 'static,
{
    use PopoverAlign::{End, Start};
    use PopoverSide::{Bottom, Top};

    AnyView::new(move |cx| {
        let surface = popup(
            rows,
            px(MIN_WIDTH),
            vec![
                placement(Bottom, Start),
                placement(Bottom, End),
                placement(Top, Start),
                placement(Top, End),
            ],
            MenuFocusState::Closed,
        );
        let node = cx.build(surface);
        cx.world
            .entity_mut(node)
            .insert((FocusFirst, OpenedFor(serial)));
        node
    })
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::color::Color;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::ecs::resource::Resource;
    use bevy::input_focus::InputFocus;
    use bevy::picking::backend::HitData;
    use bevy::picking::pointer::{Location, PointerId};
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use bevy::ui::Node;
    use bevy::ui::widget::Text;
    use bevy::ui_widgets::popover::Popover;
    use bevy::ui_widgets::{Activate, MenuPopup};

    use super::*;
    use crate::tokens::{
        Curve, Motion, MotionTokens, TextTokens, Tone,
    };
    use crate::views::{BehaviorExt, label, menu_item};
    use crate::{FynixPlugin, Theme, mount};

    struct Plain;

    impl SpacingTokens for Plain {
        fn gap(&self) -> f32 {
            6.0
        }

        fn row(&self) -> f32 {
            20.0
        }

        fn radius(&self) -> f32 {
            3.0
        }
    }

    impl SurfaceTokens for Plain {
        fn fill(&self) -> Color {
            Color::BLACK
        }

        fn hover(&self) -> Color {
            Color::WHITE
        }

        fn panel(&self) -> Color {
            Color::BLACK
        }
    }

    impl TextTokens for Plain {
        fn tone(&self, _: Tone) -> Color {
            Color::WHITE
        }

        fn body_size(&self) -> f32 {
            14.0
        }

        fn small_size(&self) -> f32 {
            11.0
        }
    }

    impl MotionTokens for Plain {
        fn motion(&self, _: Motion) -> Curve {
            Curve {
                duration: Duration::from_millis(100),
                ease: |t| t,
            }
        }
    }

    #[derive(Resource, Default)]
    struct Deleted(u32);

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain))
        .init_resource::<Deleted>()
        .insert_resource(
            TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ),
        );
        // The first update only starts the clock.
        app.update();
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
        let location = Location {
            target: NormalizedRenderTarget::None {
                width: 800,
                height: 600,
            },
            position: at,
        };
        let hit = HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location,
            Press {
                button,
                hit,
                count: 1,
            },
            on,
        ));
    }

    /// The popups of every menu open.
    fn popups(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<MenuPopup>>()
            .iter(app.world())
            .collect()
    }

    fn kids(app: &App, node: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(node)
            .map(|kids| kids.iter().collect())
            .unwrap_or_default()
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
