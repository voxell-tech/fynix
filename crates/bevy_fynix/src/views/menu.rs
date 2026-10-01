//! The surface menus are drawn on, the rows in it, and what the
//! floating widgets built on them share.

use bevy::color::Alpha;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, ResMut};
use bevy::input_focus::tab_navigation::{
    NavAction, TabIndex, TabNavigation,
};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::math::Vec2;
use bevy::ui::{
    AlignItems, FlexDirection, Node, Overflow, OverrideClip,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui_widgets::popover::{Popover, PopoverPlacement};
use bevy::ui_widgets::{
    MenuFocusState, MenuItem as MenuItemBehavior, MenuPopup,
};
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::prop::{Signal, component, each};
use crate::state::State;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens,
};
use crate::views::frame::{Frame, FrameProps};
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

/// A column of `items` on a menu surface, at least `min_width` wide,
/// that `positions` place against its parent and that closes when it
/// loses focus. Its focus state starts as `focus`.
pub(crate) fn popup<T, S>(
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
                ))
                .rules(|cx: &mut Cx<'_, Bevy, T>| {
                    cx.defaults(menu_surface);
                }),
        )
    })
}

/// On a popup that takes the keyboard focus on its first row once it
/// is built.
#[derive(Component)]
pub(crate) struct FocusFirst;

/// Gives the first row of each popup marked [`FocusFirst`] the focus,
/// which is what keeps it open.
pub(crate) fn focus_first(
    popups: Query<Entity, With<FocusFirst>>,
    mut states: Query<&mut MenuFocusState>,
    navigation: TabNavigation,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for popup in &popups {
        commands.entity(popup).remove::<FocusFirst>();
        if let Ok(row) =
            navigation.initialize(popup, NavAction::First)
        {
            focus.set(row, FocusCause::Navigated);
            if let Ok(mut state) = states.get_mut(popup) {
                *state = MenuFocusState::Open;
            }
        }
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
/// the layout, for what floats to hang from.
fn anchor<T, C>(
    source: Entity,
    at: fn(&C) -> Vec2,
) -> AnyView<Bevy, T>
where
    T: SpacingTokens + Send + Sync + 'static,
    C: Component,
{
    AnyView::new(move |cx| {
        cx.build(
            row(())
                .position(PositionType::Absolute)
                .inset(component::<C, _>(source, move |c| {
                    c.map_or(UiRect::all(Val::Auto), |c| {
                        let at = at(c);
                        UiRect::new(
                            px(at.x),
                            Val::Auto,
                            px(at.y),
                            Val::Auto,
                        )
                    })
                }))
                .with(Floating(source)),
        )
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
    cx.at_root(|cx| cx.build(view))
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::resource::Resource;
    use bevy::ui::{
        BackgroundColor, BorderColor, BorderRadius, GlobalZIndex,
    };
    use bevy::ui_widgets::Activate;
    use motiongfx_interp::interpolation::Interpolation;

    use super::*;
    use crate::tests::{self, Plain};
    use crate::transition::BevyMarker;
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

    fn blend(from: Color, to: Color, t: f32) -> Color {
        <Color as Interpolation<BevyMarker>>::interp(&from, &to, t)
    }

    #[test]
    fn a_surface_has_the_panel_look_above_everything() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            frame().rules(menu_surface),
        );

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.position_type, PositionType::Absolute);
        assert_eq!(ui.flex_direction, FlexDirection::Column);
        assert_eq!(ui.padding, UiRect::all(px(4.0)));
        assert_eq!(ui.border, UiRect::all(px(1.0)));
        assert_eq!(ui.border_radius, BorderRadius::all(px(5.0)));
        assert_eq!(ui.overflow, Overflow::clip());
        assert_eq!(fill(&app, node), Color::srgb(0.1, 0.1, 0.1));
        assert_eq!(
            app.world().get::<BorderColor>(node),
            Some(&BorderColor::all(Color::srgb(0.4, 0.4, 0.4)))
        );
        assert_eq!(
            app.world().get::<GlobalZIndex>(node),
            Some(&GlobalZIndex(MENU_Z))
        );
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
    fn the_call_site_beats_the_surface() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            frame().fill(Color::WHITE).rules(menu_surface),
        );

        assert_eq!(fill(&app, node), Color::WHITE);
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
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.width, percent(100.0));
        assert_eq!(ui.min_height, px(20.0));
        assert_eq!(ui.border_radius, BorderRadius::all(px(3.0)));
    }

    #[test]
    fn a_row_lights_up_under_the_pointer_and_fades_back() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), menu_item(label("Cut")));
        let rest = Color::srgb(0.3, 0.3, 0.3).with_alpha(0.0);
        let hover = Color::srgb(0.3, 0.3, 0.3);
        assert_eq!(fill(&app, node), rest);

        app.world_mut().entity_mut(node).insert(Hovered);
        app.update();
        assert_eq!(fill(&app, node), blend(rest, hover, 0.5));
        app.update();
        assert_eq!(fill(&app, node), hover);

        app.world_mut().entity_mut(node).remove::<Hovered>();
        app.update();
        app.update();
        assert_eq!(fill(&app, node), rest);
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
