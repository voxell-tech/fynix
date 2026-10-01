//! A clickable [`Frame`] around one content view.
//!
//! Like a [`Stack`](super::Stack), it holds its frame and takes the
//! frame's props through [`FrameProps`]. Everything else is rules:
//! its defaults
//! (fill, radius, centred content, a hover fill and a transition) are
//! defaults for its root frame alone, so an app's `set::<Frame>` and
//! a call site's `.when::<Hovered, _>(..)` both beat them, and none
//! of them reach the content.

use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ui::{AlignItems, JustifyContent, UiRect, percent, px};
use bevy::ui_widgets::Button as ButtonBehavior;
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::state::State;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens, Tone,
};
use crate::views::frame::{Frame, FrameProps};
use crate::views::{Icon, Label};
use crate::{Bevy, Cx, Hovered, Styled, View};

pub struct Button<C> {
    pub frame: Frame,
    pub content: C,
}

pub fn button<C>(content: C) -> Button<C> {
    Button {
        frame: Frame::unset(),
        content,
    }
}

/// A button's defaults, for its root frame alone.
fn defaults<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SurfaceTokens + SpacingTokens + MotionTokens + 'static,
{
    cx.set::<Frame>(|frame, theme: &T| {
        frame
            .fill(theme.fill())
            .radius(theme.radius())
            .justify(JustifyContent::Center)
            .align(AlignItems::Center)
    });
    cx.when::<State<Hovered>>(|cx| {
        cx.set::<Frame>(|frame, theme: &T| frame.fill(theme.hover()));
    });
    cx.transition(Motion::Interact);
}

/// The padding along a strip of buttons, twice the theme's gap.
fn strip_pad<T: SpacingTokens>(theme: &T) -> f32 {
    theme.gap() * 2.0
}

/// A button with no surface until the pointer is on it, padded, with
/// the theme's radius. Apply with `.rules(ghost)`.
pub fn ghost<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SpacingTokens + 'static,
{
    cx.root(|cx| {
        cx.set::<Frame>(|frame, theme: &T| {
            frame
                .fill(Color::NONE)
                .padding(UiRect::axes(
                    px(strip_pad(theme)),
                    px(theme.gap()),
                ))
                .radius(theme.radius())
        });
    });
}

/// A button with no surface at all, resting or hovered. Every
/// [`Label`] and [`Icon`] in it turns [`Tone::Accent`] while the
/// pointer is on it. Apply with `.rules(tint)`.
pub fn tint<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: MotionTokens + 'static,
{
    cx.root(|cx| {
        cx.set::<Frame>(|frame, _| frame.fill(Color::NONE));
        cx.when::<State<Hovered>>(|cx| {
            cx.set::<Frame>(|frame, _| frame.fill(Color::NONE));
        });
    });
    cx.when::<State<Hovered>>(|cx| {
        cx.set::<Label>(|label, _| label.tone(Tone::Accent));
        cx.set::<Icon>(|icon, _| icon.tone(Tone::Accent));
    });
    cx.transition(Motion::Interact);
}

/// A button for a menu bar: no resting surface, the height of its
/// parent, square corners and horizontal padding, lighting up under
/// the pointer. Apply with `.rules(menu_bar)`.
pub fn menu_bar<T>(cx: &mut Cx<'_, Bevy, T>)
where
    T: SpacingTokens + 'static,
{
    cx.root(|cx| {
        cx.set::<Frame>(|frame, theme: &T| {
            frame
                .fill(Color::NONE)
                .height(percent(100.0))
                .radius(0.0)
                .padding(UiRect::axes(px(strip_pad(theme)), px(0.0)))
        });
    });
}

/// A button for a row of exclusive options: a full row high, square,
/// and growing to share the row. It is filled with the theme's
/// accent, and does not change under the pointer, while `active`.
/// Apply with `.rules(segment(active))`.
pub fn segment<T>(
    active: bool,
) -> impl Fn(&mut Cx<'_, Bevy, T>) + Send + Sync + 'static
where
    T: SurfaceTokens + SpacingTokens + 'static,
{
    move |cx: &mut Cx<'_, Bevy, T>| {
        cx.root(|cx| {
            cx.set::<Frame>(move |frame, theme: &T| {
                let frame = frame
                    .height(px(theme.row()))
                    .radius(0.0)
                    .grow(1.0);
                if active {
                    frame.fill(theme.accent())
                } else {
                    frame
                }
            });
            if active {
                cx.when::<State<Hovered>>(|cx| {
                    cx.set::<Frame>(|frame, theme: &T| {
                        frame.fill(theme.accent())
                    });
                });
            }
        });
    }
}

impl<C> FrameProps for Button<C> {
    fn frame_mut(&mut self) -> &mut Frame {
        &mut self.frame
    }
}

impl<T, C> View<Bevy, T> for Button<C>
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
            cx.defaults(|cx| cx.root(defaults));
            let node = cx.build(self.frame);
            cx.world.entity_mut(node).insert((
                ButtonBehavior,
                EntityCursor(SystemCursorIcon::Pointer),
            ));
            cx.under(node, |cx| cx.build(self.content));
            node
        })
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::{App, PreUpdate};
    use bevy::asset::Handle;
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::lifecycle::Remove;
    use bevy::ecs::observer::On;
    use bevy::ecs::resource::Resource;
    use bevy::ecs::system::ResMut;
    use bevy::picking::backend::HitData;
    use bevy::picking::hover::{HoverMap, update_is_hovered};
    use bevy::picking::pointer::PointerId;
    use bevy::text::{FontSize, TextColor, TextFont};
    use bevy::ui::widget::{ImageNode, Text};
    use bevy::ui::{BackgroundColor, BorderRadius, Node, Val};
    use motiongfx_interp::interpolation::Interpolation;

    use super::*;
    use crate::testing::{self, Plain};
    use crate::transition::{BevyMarker, ReducedMotion};
    use crate::views::{icon, label, row};
    use crate::{AnyView, ScopedExt, StateExt, mount};

    const REST: Color = Color::srgb(0.2, 0.2, 0.2);
    const HOVER: Color = Color::srgb(0.3, 0.3, 0.3);
    const ACCENT: Color = Color::srgb(0.9, 0.5, 0.1);

    /// How many times [`Hovered`] was taken off a node.
    #[derive(Resource, Default)]
    struct Releases(usize);

    fn app() -> App {
        let mut app = testing::app_with(Plain {
            accent: Some(ACCENT),
            accent_tone: Some(ACCENT),
            ..Plain::default()
        });
        app.init_resource::<Releases>()
            .add_systems(PreUpdate, update_is_hovered)
            .add_observer(
                |_: On<Remove, Hovered>,
                 mut releases: ResMut<Releases>| {
                    releases.0 += 1;
                },
            );
        app
    }

    fn fill(app: &App, node: Entity) -> Color {
        app.world().get::<BackgroundColor>(node).unwrap().0
    }

    fn blend(from: Color, to: Color, t: f32) -> Color {
        <Color as Interpolation<BevyMarker>>::interp(&from, &to, t)
    }

    fn hover(app: &mut App, node: Entity, on: bool) {
        let mut node = app.world_mut().entity_mut(node);
        if on {
            node.insert(Hovered);
        } else {
            node.remove::<Hovered>();
        }
    }

    /// Puts the mouse over `entity` alone, then runs an update.
    fn point_at(app: &mut App, entity: Option<Entity>) {
        let mut map = HoverMap::default();
        if let Some(entity) = entity {
            let hit =
                HitData::new(Entity::PLACEHOLDER, 0.0, None, None);
            map.0
                .entry(PointerId::Mouse)
                .or_default()
                .insert(entity, hit);
        }
        app.insert_resource(map);
        app.update();
    }

    fn hovered(app: &App, node: Entity) -> bool {
        app.world().get::<Hovered>(node).is_some()
    }

    #[test]
    fn a_button_defaults_from_the_theme_and_holds_its_content() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), button(label("Save")));

        assert!(app.world().get::<ButtonBehavior>(node).is_some());
        assert_eq!(fill(&app, node), Color::srgb(0.2, 0.2, 0.2));
        assert_eq!(
            app.world().get::<Node>(node).unwrap().border_radius,
            BorderRadius::all(Val::Px(3.0))
        );
        let content = app.world().get::<Children>(node).unwrap()[0];
        assert_eq!(
            app.world().get::<Text>(content).unwrap().0,
            "Save"
        );
    }

    #[test]
    fn the_call_site_beats_the_default() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).fill(Color::WHITE),
        );

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn hovering_moves_the_fill_to_the_hover_colour_and_back() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));

        hover(&mut app, node, true);
        app.update();
        assert_eq!(
            fill(&app, node),
            blend(REST, HOVER, 0.5),
            "50ms of 100ms"
        );
        app.update();
        assert_eq!(fill(&app, node), HOVER);

        hover(&mut app, node, false);
        app.update();
        assert_eq!(fill(&app, node), blend(HOVER, REST, 0.5));
        app.update();
        assert_eq!(fill(&app, node), REST);
    }

    #[test]
    fn a_call_site_state_rule_beats_the_default_hover_fill() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).when::<Hovered, _>(
                |cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Frame>(|f, _| f.fill(Color::WHITE));
                },
            ),
        );

        hover(&mut app, node, true);
        app.update();
        app.update();

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn a_state_rule_on_the_button_reaches_its_content() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).when::<Hovered, _>(
                |cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Label>(|l, _| l.size(20.0));
                },
            ),
        );
        let content = app.world().get::<Children>(node).unwrap()[0];
        let size = |app: &App| {
            app.world().get::<TextFont>(content).unwrap().font_size
        };
        assert_eq!(size(&app), FontSize::Px(14.0));

        hover(&mut app, node, true);
        app.update();

        assert_eq!(size(&app), FontSize::Px(20.0));
    }

    #[test]
    fn an_app_rule_restyles_every_button() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            AnyView::<Bevy, Plain>::new(|cx| {
                cx.set::<Frame>(|f, _| f.fill(Color::WHITE));
                cx.build(button(label("x")))
            }),
        );

        assert_eq!(fill(&app, root), Color::WHITE);
    }

    #[test]
    fn a_call_site_fill_is_the_resting_fill() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).fill(Color::BLACK),
        );
        assert_eq!(fill(&app, node), Color::BLACK);

        hover(&mut app, node, true);
        app.update();
        app.update();
        assert_eq!(fill(&app, node), HOVER);

        hover(&mut app, node, false);
        app.update();
        app.update();
        assert_eq!(fill(&app, node), Color::BLACK);
    }

    #[test]
    fn a_ghost_button_lights_up_too() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).fill(Color::NONE),
        );

        hover(&mut app, node, true);
        app.update();
        app.update();

        assert_eq!(fill(&app, node), HOVER);
    }

    #[test]
    fn a_hovered_child_keeps_the_button_hovered() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));
        let child = app.world().get::<Children>(node).unwrap()[0];

        point_at(&mut app, Some(child));
        assert!(hovered(&app, node));

        // From the label to the padding and back, the way the
        // pointer crosses the button.
        point_at(&mut app, Some(node));
        assert!(hovered(&app, node));
        point_at(&mut app, Some(child));
        assert!(hovered(&app, node));
        assert_eq!(app.world().resource::<Releases>().0, 0);
        assert_eq!(fill(&app, node), HOVER);

        point_at(&mut app, None);
        assert!(!hovered(&app, node));
        assert_eq!(app.world().resource::<Releases>().0, 1);
    }

    #[test]
    fn reduced_motion_snaps_the_fill() {
        let mut app = app();
        app.insert_resource(ReducedMotion(true));
        let node =
            mount::<Plain>(app.world_mut(), button(label("x")));

        hover(&mut app, node, true);
        app.update();
        assert_eq!(fill(&app, node), HOVER);

        hover(&mut app, node, false);
        app.update();
        assert_eq!(fill(&app, node), REST);
    }

    /// Hovers `node`, or lets go, and runs the transition out.
    fn settle(app: &mut App, node: Entity, on: bool) {
        hover(app, node, on);
        app.update();
        app.update();
    }

    fn ui(app: &App, node: Entity) -> &Node {
        app.world().get::<Node>(node).unwrap()
    }

    #[test]
    fn a_ghost_button_has_no_surface_until_hovered() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).rules(ghost),
        );

        assert_eq!(fill(&app, node), Color::NONE);
        assert_eq!(
            ui(&app, node).padding,
            UiRect::axes(px(12.0), px(6.0))
        );
        assert_eq!(
            ui(&app, node).border_radius,
            BorderRadius::all(px(3.0))
        );

        settle(&mut app, node, true);
        assert_eq!(fill(&app, node), HOVER);
        settle(&mut app, node, false);
        assert_eq!(fill(&app, node), Color::NONE);
    }

    #[test]
    fn a_tint_button_has_no_surface_and_turns_its_parts_accent() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(row((icon(Handle::default()), label("x"))))
                .rules(tint),
        );
        let content = app.world().get::<Children>(node).unwrap()[0];
        let [mark, text] =
            app.world().get::<Children>(content).unwrap()[..]
        else {
            panic!("an icon and a label");
        };
        let colours = |app: &App| {
            (
                app.world().get::<ImageNode>(mark).unwrap().color,
                app.world().get::<TextColor>(text).unwrap().0,
            )
        };
        assert_eq!(fill(&app, node), Color::NONE);
        assert_eq!(colours(&app), (Color::WHITE, Color::WHITE));

        settle(&mut app, node, true);
        assert_eq!(fill(&app, node), Color::NONE);
        assert_eq!(colours(&app), (ACCENT, ACCENT));
        assert_eq!(
            ui(&app, content).padding,
            UiRect::DEFAULT,
            "the row is not a button"
        );

        settle(&mut app, node, false);
        assert_eq!(colours(&app), (Color::WHITE, Color::WHITE));
    }

    #[test]
    fn a_menu_bar_button_is_full_height_square_and_lights_up() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("File")).rules(menu_bar),
        );

        assert_eq!(fill(&app, node), Color::NONE);
        let ui = ui(&app, node);
        assert_eq!(ui.height, percent(100.0));
        assert_eq!(ui.border_radius, BorderRadius::all(px(0.0)));
        assert_eq!(ui.padding, UiRect::axes(px(12.0), px(0.0)));

        settle(&mut app, node, true);
        assert_eq!(fill(&app, node), HOVER);
    }

    #[test]
    fn an_inactive_segment_is_a_square_row_that_lights_up() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("a")).rules(segment(false)),
        );

        let ui = ui(&app, node);
        assert_eq!(ui.height, px(20.0));
        assert_eq!(ui.flex_grow, 1.0);
        assert_eq!(ui.border_radius, BorderRadius::all(px(0.0)));
        assert_eq!(fill(&app, node), REST);

        settle(&mut app, node, true);
        assert_eq!(fill(&app, node), HOVER);
    }

    #[test]
    fn an_active_segment_is_accent_and_stays_so_under_the_pointer() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("a")).rules(segment(true)),
        );
        assert_eq!(fill(&app, node), ACCENT);

        settle(&mut app, node, true);
        assert_eq!(fill(&app, node), ACCENT);
        settle(&mut app, node, false);
        assert_eq!(fill(&app, node), ACCENT);
    }

    #[test]
    fn a_call_site_prop_beats_a_bundle() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x")).fill(Color::WHITE).rules(ghost),
        );
        let strip = mount::<Plain>(
            app.world_mut(),
            button(label("x"))
                .height(px(50.0))
                .fill(Color::BLACK)
                .rules(segment(true)),
        );

        assert_eq!(fill(&app, node), Color::WHITE);
        assert_eq!(fill(&app, strip), Color::BLACK);
        assert_eq!(ui(&app, strip).height, px(50.0));
        assert_eq!(ui(&app, strip).flex_grow, 1.0, "the rest stays");
    }

    #[test]
    fn a_call_site_state_rule_beats_a_bundles() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(label("x"))
                .rules(segment(true))
                .when::<Hovered, _>(|cx: &mut Cx<Bevy, Plain>| {
                    cx.set::<Frame>(|f, _| f.fill(Color::WHITE));
                }),
        );

        settle(&mut app, node, true);

        assert_eq!(fill(&app, node), Color::WHITE);
    }

    #[test]
    fn a_bundle_does_not_reach_a_frame_in_the_content() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            button(row((label("x"),))).rules(menu_bar),
        );
        let content = app.world().get::<Children>(node).unwrap()[0];

        assert_eq!(ui(&app, content).height, Val::Auto);
        assert_eq!(fill(&app, content), Color::NONE);
    }

    #[test]
    fn a_set_rule_on_frame_does_not_reach_the_content() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            AnyView::<Bevy, Plain>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.set::<Frame>(|f, _| f.width(px(10.0)));
                    cx.set::<Label>(|l, _| l.size(20.0));
                    cx.build(button(label("x")));
                });
                root
            }),
        );
        let node = app.world().get::<Children>(root).unwrap()[0];

        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.width, Val::Px(10.0));
        let content = app.world().get::<Children>(node).unwrap()[0];
        assert_eq!(
            app.world().get::<TextFont>(content).unwrap().font_size,
            FontSize::Px(20.0)
        );
    }
}
