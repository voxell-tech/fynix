//! A menu opened at the pointer by a right-click.

use core::marker::PhantomData;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Res};
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
/// opened, in logical pixels.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct OpenAt(Vec2);

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
    mut commands: Commands,
) {
    if press.button != PointerButton::Secondary {
        return;
    }
    press.propagate(false);
    let scale = scale.map_or(1.0, |scale| scale.0);
    commands
        .entity(press.event_target())
        .insert(OpenAt(press.pointer_location.position / scale));
}

/// Closes the context menu a close request bubbles up to.
pub(crate) fn dismiss(
    mut event: On<MenuEvent>,
    hosts: bevy::ecs::system::Query<&Floating>,
    mut commands: Commands,
) {
    let Ok(Floating(source)) = hosts.get(event.event_target()) else {
        return;
    };
    if matches!(event.action, MenuAction::CloseAll) {
        commands.entity(*source).remove::<OpenAt>();
        event.propagate(false);
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
        float::<T, OpenAt, OpenAt, (u32, u32)>(
            cx,
            source,
            |open| open.0,
            component::<OpenAt, _>(source, |open| {
                open.map(|open| vec![*open]).unwrap_or_default()
            }),
            |open| (open.0.x.to_bits(), open.0.y.to_bits()),
            move |_| menu(items()),
        );
        source
    }
}

/// The rows on a surface, placed against the point they open at,
/// whichever corner has room.
fn menu<T, S>(rows: S) -> AnyView<Bevy, T>
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
        cx.world.entity_mut(node).insert(FocusFirst);
        node
    })
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::camera::NormalizedRenderTarget;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
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
        app.update();
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

        press(
            &mut app,
            source,
            PointerButton::Secondary,
            Vec2::new(30.0, 40.0),
        );
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
