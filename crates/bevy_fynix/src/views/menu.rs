//! The surface menus are drawn on, the rows in it, and what the
//! floating widgets built on them share.

use core::marker::PhantomData;
use core::time::Duration;

use bevy::color::Alpha;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::input::ButtonState;
use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::input_focus::tab_navigation::{
    NavAction, TabIndex, TabNavigation,
};
use bevy::input_focus::{FocusCause, FocusedInput, InputFocus};
use bevy::math::Vec2;
use bevy::picking::Pickable;
use bevy::picking::events::{Pointer, Press};
use bevy::ui::{
    AlignItems, FlexDirection, Node, Overflow, OverrideClip,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui_widgets::popover::{
    Popover, PopoverAlign, PopoverPlacement, PopoverSide,
};
use bevy::ui_widgets::{
    MenuAction, MenuEvent, MenuFocusState,
    MenuItem as MenuItemBehavior, MenuPopup,
};
use bevy::window::SystemCursorIcon;
use motiongfx_interp::ease;

use crate::cursor::EntityCursor;
use crate::prop::{Signal, component, each};
use crate::shortcut::{MENU, Scope};
use crate::state::State;
use crate::tokens::{
    Curve, Motion, MotionTokens, SpacingTokens, SurfaceTokens,
    TextTokens, Tone,
};
use crate::views::frame::{Frame, FrameProps};
use crate::views::label::label;
use crate::views::stack::{column, row};
use crate::{
    AnyView, Bevy, Cx, Hovered, ScopedExt, Styled, View, ViewSeq,
};

/// Where a menu sits in the window's stack, above the interface.
pub const MENU_Z: i32 = 1000;

/// Where a tooltip sits in the window's stack, above menus.
pub const TOOLTIP_Z: i32 = 1100;

/// The width of a menu surface's border.
const HAIRLINE: f32 = 1.0;

/// The rules making a frame a menu surface: panel fill, hairline
/// border, menu radius and padding, and a place above the interface.
/// They reach the root of the view they are set for alone.
pub fn menu_surface<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + 'static,
{
    cx.root(|cx| {
        cx.set::<Frame>(|frame, theme: &T| {
            frame
                .direction(FlexDirection::Column)
                .gap(0.0)
                .position(PositionType::Absolute)
                .padding(UiRect::all(px(theme.menu_padding())))
                .fill(theme.panel())
                .border(HAIRLINE)
                .border_color(theme.hairline())
                .radius(theme.menu_radius())
                .overflow(Overflow::clip())
                .z(Some(MENU_Z))
        });
    });
}

/// One row of a menu: `content` in a frame that lights up under the
/// pointer and can be activated, by pointer or keyboard.
pub struct MenuItem<C> {
    pub frame: Frame,
    pub content: C,
}

pub fn menu_item<C>(content: C) -> MenuItem<C> {
    MenuItem {
        frame: Frame::unset(),
        content,
    }
}

/// A menu row's defaults, for its root frame alone.
fn item_defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    cx.set::<Frame>(|frame, theme: &T| {
        frame
            .width(percent(100.0))
            .min_height(px(theme.row()))
            .padding(UiRect::axes(px(theme.gap()), px(0.0)))
            .align(AlignItems::Center)
            .radius(theme.menu_item_radius())
            // The hover colour, clear, so the fill fades in rather
            // than passing through black.
            .fill(theme.hover().with_alpha(0.0))
    });
    cx.when::<State<Hovered>>(|cx| {
        cx.set::<Frame>(|frame, theme: &T| frame.fill(theme.hover()));
    });
    cx.transition(Motion::Interact);
}

impl<C> FrameProps for MenuItem<C> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T, C> View<Bevy, T> for MenuItem<C>
where
    T: SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    C: View<Bevy, T>,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        cx.scope(|cx| {
            cx.defaults(|cx| cx.root(item_defaults));
            let node = cx.build(self.frame);
            cx.world.entity_mut(node).insert((
                MenuItemBehavior,
                // Focus is what keeps a popup open, so a row takes
                // it.
                TabIndex(0),
                EntityCursor(SystemCursorIcon::Pointer),
            ));
            cx.under(node, |cx| cx.build(self.content));
            node
        })
    }
}

/// A menu row that opens the rows `items` returns as a menu of their
/// own beside it, while the pointer is on the row or on them.
pub struct Submenu<C, F, S> {
    pub frame: Frame,
    pub content: C,
    items: F,
    rows: PhantomData<fn() -> S>,
}

/// A [`Submenu`] row showing `content`. `items` runs each time its
/// menu opens.
pub fn submenu<C, F, S>(content: C, items: F) -> Submenu<C, F, S>
where
    F: Fn() -> S + Send + Sync + 'static,
{
    Submenu {
        frame: Frame::unset(),
        content,
        items,
        rows: PhantomData,
    }
}

impl<C, F, S> FrameProps for Submenu<C, F, S> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

/// On the node a popup is built and dropped under, laid over the
/// node the popup hangs from.
#[derive(Component)]
pub(crate) struct Slot;

/// A node laid over its parent, for a `keyed` or an `each` to build
/// a popup under: the popup is placed against it as against the
/// parent.
pub(crate) fn slot<T>() -> impl View<Bevy, T> + 'static
where
    T: SpacingTokens + Send + Sync + 'static,
{
    column(())
        .position(PositionType::Absolute)
        .inset(UiRect::all(Val::ZERO))
        .with((Slot, Pickable::IGNORE))
}

impl<T, C, F, S> View<Bevy, T> for Submenu<C, F, S>
where
    T: TextTokens
        + SurfaceTokens
        + SpacingTokens
        + MotionTokens
        + Send
        + Sync
        + 'static,
    C: View<Bevy, T>,
    F: Fn() -> S + Send + Sync + 'static,
    S: ViewSeq<Bevy, T> + 'static,
{
    fn build(self, cx: &mut Cx<'_, Bevy, T>) -> Entity {
        let min_width = px(cx.theme().menu_width());
        // Past the padding of the menu the row is in, and as far
        // again clear of its edge.
        let gap = cx.theme().menu_padding() * 2.0;
        let items = self.items;
        cx.scope(|cx| {
            cx.defaults(|cx| cx.root(item_defaults));
            let node = cx.build(self.frame);
            cx.world
                .entity_mut(node)
                .insert(EntityCursor(SystemCursorIcon::Pointer));
            cx.under(node, |cx| {
                cx.build(self.content);
                cx.build(row(()).grow(1.0));
                cx.build(label(">").tone(Tone::Dim));
            });
            let popup = move |_: &()| {
                let rows = items();
                AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
                    // Closed, so the focus stays in the menu the row
                    // is in: it opens and shuts with the row's hover.
                    let node = cx.build(menu_popup(
                        rows,
                        min_width,
                        beside(gap),
                        MenuFocusState::Closed,
                    ));
                    // A strip over the gap on either side, so the
                    // pointer keeps the row hovered on its way
                    // across.
                    cx.under(node, |cx| {
                        for (left, right) in [
                            (px(-gap), Val::Auto),
                            (Val::Auto, px(-gap)),
                        ] {
                            cx.build(
                                column(())
                                    .position(PositionType::Absolute)
                                    .inset(UiRect {
                                        left,
                                        right,
                                        top: Val::ZERO,
                                        bottom: Val::ZERO,
                                    })
                                    .width(px(gap))
                                    .with(OverrideClip),
                            );
                        }
                    });
                    node
                })
            };
            // Under the row, so the focus stays within the menu the
            // row is in.
            cx.under(node, |cx| {
                cx.build(
                    each(
                        component::<Hovered, _>(node, |hovered| {
                            hovered
                                .map(|_| ())
                                .into_iter()
                                .collect::<Vec<_>>()
                        }),
                        |_| (),
                        popup,
                    )
                    .within(slot::<T>()),
                );
            });
            node
        })
    }
}

/// Right of the anchor, then left of it, lined up with its top, then
/// its bottom, `gap` away from it.
fn beside(gap: f32) -> Vec<PopoverPlacement> {
    use PopoverAlign::{End, Start};
    use PopoverSide::{Left, Right};

    [(Right, Start), (Left, Start), (Right, End), (Left, End)]
        .into_iter()
        .map(|(side, align)| PopoverPlacement { side, align, gap })
        .collect()
}

/// A column of `items` on a menu surface, at least `min_width` wide,
/// that `positions` place against its parent and that closes when it
/// loses focus. Its focus state starts as `focus`.
pub(crate) fn menu_popup<T, S>(
    items: S,
    min_width: Val,
    positions: Vec<PopoverPlacement>,
    focus: MenuFocusState,
) -> AnyView<Bevy, T>
where
    T: SurfaceTokens + SpacingTokens + Send + Sync + 'static,
    S: ViewSeq<Bevy, T> + 'static,
{
    AnyView::new(move |cx: &mut Cx<'_, Bevy, T>| {
        let window_margin = cx.theme().menu_margin();
        cx.build(
            column(items)
                .min_width(min_width)
                .with((
                    MenuPopup::default(),
                    focus,
                    Popover {
                        positions,
                        window_margin,
                    },
                    OverrideClip,
                    Scope(MENU),
                ))
                .rules(|cx: &mut Cx<'_, Bevy, T>| {
                    cx.defaults(menu_surface);
                }),
        )
    })
}

/// On a popup that takes the keyboard focus once it is built, on its
/// first row or its last.
#[derive(Component)]
pub(crate) struct FocusOn(pub(crate) NavAction);

/// Gives a row of each popup marked [`FocusOn`] the focus, which is
/// what keeps it open.
pub(crate) fn focus_first(
    popups: Query<(Entity, &FocusOn)>,
    mut states: Query<&mut MenuFocusState>,
    navigation: TabNavigation,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for (popup, on) in &popups {
        commands.entity(popup).remove::<FocusOn>();
        if let Ok(row) = navigation.initialize(popup, on.0) {
            focus.set(row, FocusCause::Navigated);
            if let Ok(mut state) = states.get_mut(popup) {
                *state = MenuFocusState::Open;
            }
        }
    }
}

/// Closes every open popup a press lands outside of, and takes the
/// focus out of it. A press on the node a popup hangs from, such as
/// a dropdown's button, is left to that node.
///
/// `bevy_ui_widgets` closes a popup when the focus leaves it, but
/// nothing moves the focus on a press: that is the tab navigation
/// plugin's `click_to_focus`, which an app that does not draw with
/// Bevy's feathers does not have.
pub(crate) fn close_on_outside_press(
    press: On<Pointer<Press>>,
    popups: Query<(Entity, &MenuFocusState), With<MenuPopup>>,
    parents: Query<&ChildOf>,
    slots: Query<(), With<Slot>>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    if press.entity != press.original_event_target() {
        return;
    }
    let within = |node: Entity, ancestor: Entity| {
        node == ancestor
            || parents.iter_ancestors(node).any(|up| up == ancestor)
    };
    for (popup, state) in &popups {
        if matches!(state, MenuFocusState::Closed) {
            continue;
        }
        // Past the slot it was built under, which only stands in for
        // the node it hangs from.
        let owner = parents
            .iter_ancestors(popup)
            .find(|&up| !slots.contains(up))
            .unwrap_or(popup);
        if within(press.entity, owner) {
            continue;
        }
        if focus.get().is_some_and(|held| within(held, popup)) {
            focus.clear();
        }
        commands.entity(popup).insert(MenuFocusState::Closed);
        commands.trigger(MenuEvent {
            source: popup,
            action: MenuAction::CloseAll,
        });
    }
}

/// Closes the popup of a row that has the focus when Escape goes
/// down, and gives the focus back to its owner. `bevy_ui_widgets`
/// only reads Escape when the popup itself has the focus, which a
/// popup with rows never does.
pub(crate) fn close_on_escape(
    event: On<FocusedInput<KeyboardInput>>,
    rows: Query<(), With<MenuItemBehavior>>,
    mut commands: Commands,
) {
    let key = &event.input;
    if key.key_code != KeyCode::Escape
        || key.state != ButtonState::Pressed
        || key.repeat
        || !rows.contains(event.focused_entity)
    {
        return;
    }
    for action in [MenuAction::FocusRoot, MenuAction::CloseAll] {
        commands.trigger(MenuEvent {
            source: event.focused_entity,
            action,
        });
    }
}

/// On a node built for a source node, which it follows to the grave.
#[derive(Component)]
pub(crate) struct Floating(pub Entity);

/// Despawns every [`Floating`] node whose source is gone.
pub(crate) fn despawn_orphans(
    floating: Query<(Entity, &Floating)>,
    nodes: Query<(), With<Node>>,
    mut commands: Commands,
) {
    for (node, Floating(source)) in &floating {
        if !nodes.contains(*source) {
            commands.entity(node).despawn();
        }
    }
}

/// A node at the point the component `C` of `source` names, out of
/// the layout, for what floats to hang from. It stays at the last
/// point once `C` is gone, so what still fades out under it does not
/// jump.
fn anchor<T, C>(
    source: Entity,
    at: fn(&C) -> Vec2,
) -> AnyView<Bevy, T>
where
    T: SpacingTokens + Send + Sync + 'static,
    C: Component,
{
    AnyView::new(move |cx| {
        let node = cx.build(
            row(())
                .position(PositionType::Absolute)
                .with(Floating(source)),
        );
        cx.effect(
            node,
            component::<C, _>(source, move |c| c.map(at)).into(),
            |world, node, point| {
                let Some(point) = point else {
                    return;
                };
                if let Some(mut ui) = world.get_mut::<Node>(node) {
                    ui.left = px(point.x);
                    ui.top = px(point.y);
                }
            },
        );
        node
    })
}

/// One view per item of `items`, under an anchor at the point
/// `source`'s `C` names, which hangs at the root of the window
/// instead of where it was built. The anchor is returned.
pub(crate) fn float<T, C, I, K>(
    cx: &mut Cx<'_, Bevy, T>,
    source: Entity,
    at: fn(&C) -> Vec2,
    items: Signal<Vec<I>>,
    key: fn(&I) -> K,
    build: impl Fn(&I) -> AnyView<Bevy, T> + Send + Sync + 'static,
) -> Entity
where
    T: SpacingTokens + Send + Sync + 'static,
    C: Component,
    I: 'static,
    K: PartialEq + Clone + Send + Sync + 'static,
{
    let view =
        each(items, key, build).within(anchor::<T, C>(source, at));
    cx.at_root(|cx| {
        cx.scope(|cx| {
            // Still under the rules of where it was built, whose
            // transition would ease it in from an unset look.
            cx.transition_over(Curve {
                duration: Duration::ZERO,
                ease: ease::linear,
            });
            cx.build(view)
        })
    })
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::resource::Resource;
    use bevy::ui::{BackgroundColor, GlobalZIndex};
    use bevy::ui_widgets::Activate;

    use super::*;
    use crate::tests::{self, Plain};
    use crate::views::{BehaviorExt, frame, label};
    use crate::{ScopedExt, mount};

    fn app() -> App {
        tests::app_with(Plain {
            panel: Color::srgb(0.1, 0.1, 0.1),
            hairline: Some(Color::srgb(0.4, 0.4, 0.4)),
            menu_radius: Some(5.0),
            ..Plain::default()
        })
    }

    fn fill(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
    }

    #[test]
    fn a_surface_rule_leaves_what_is_inside_alone() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            column((frame(),)).rules(menu_surface),
        );

        let inner = app.world().get::<Children>(node).unwrap()[0];
        assert_eq!(fill(&app, inner), Color::NONE);
        assert!(app.world().get::<GlobalZIndex>(inner).is_none());
    }

    #[test]
    fn a_submenu_is_built_only_while_its_row_is_hovered() {
        let mut app = app();
        let row = mount::<Plain>(
            app.world_mut(),
            submenu(label("Wrap in"), || (menu_item(label("All")),)),
        );
        app.update();
        let popups = |app: &mut App| {
            app.world_mut()
                .query_filtered::<(), With<Popover>>()
                .iter(app.world())
                .count()
        };
        assert_eq!(popups(&mut app), 0);

        tests::hover(&mut app, row, true);
        app.update();
        assert_eq!(popups(&mut app), 1);

        tests::hover(&mut app, row, false);
        // Long enough to have faded out.
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(popups(&mut app), 0);
    }

    #[test]
    fn a_row_is_a_focusable_menu_item_with_a_pointer_cursor() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), menu_item(label("Cut")));

        assert!(app.world().get::<MenuItemBehavior>(node).is_some());
        assert_eq!(
            app.world().get::<TabIndex>(node),
            Some(&TabIndex(0))
        );
        assert_eq!(
            app.world().get::<EntityCursor>(node),
            Some(&EntityCursor(SystemCursorIcon::Pointer))
        );
    }

    #[derive(Resource, Default)]
    struct Picked(u32);

    #[test]
    fn activating_a_row_runs_its_handler() {
        let mut app = app();
        app.init_resource::<Picked>();
        let node = mount::<Plain>(
            app.world_mut(),
            menu_item(label("Cut")).on_activate(|world| {
                world.resource_mut::<Picked>().0 += 1;
            }),
        );

        app.world_mut().trigger(Activate { entity: node });
        app.update();

        assert_eq!(app.world().resource::<Picked>().0, 1);
    }
}
