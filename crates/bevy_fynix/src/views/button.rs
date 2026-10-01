//! A clickable [`Frame`] around one content view.
//!
//! Like a [`Stack`](super::Stack), it holds its frame and forwards the
//! frame's builder methods. Everything else is rules: its defaults
//! (fill, radius, centred content, a hover fill and a transition) are
//! defaults for its root frame alone, so an app's `set::<Frame>` and a
//! call site's `.when::<Hovered, _>(..)` both beat them, and none of
//! them reach the content.

use bevy::ecs::entity::Entity;
use bevy::ui::{AlignItems, FlexDirection, JustifyContent};
use bevy::ui_widgets::Button as ButtonBehavior;
use bevy::window::SystemCursorIcon;

use crate::cursor::EntityCursor;
use crate::prop::Prop;
use crate::state::State;
use crate::tokens::{
    Motion, MotionTokens, SpacingTokens, SurfaceTokens,
};
use crate::views::frame::{Frame, forward_all_frame_props};
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

impl<C> Button<C> {
    pub fn direction(
        mut self,
        direction: impl Into<Prop<FlexDirection>>,
    ) -> Self {
        self.frame = self.frame.direction(direction);
        self
    }

    forward_all_frame_props!();
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
    use core::time::Duration;

    use bevy::app::{App, PreUpdate};
    use bevy::color::Color;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::lifecycle::Remove;
    use bevy::ecs::observer::On;
    use bevy::ecs::resource::Resource;
    use bevy::ecs::system::ResMut;
    use bevy::picking::backend::HitData;
    use bevy::picking::hover::{HoverMap, update_is_hovered};
    use bevy::picking::pointer::PointerId;
    use bevy::text::{FontSize, TextFont};
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use bevy::ui::Val;
    use bevy::ui::widget::Text;
    use bevy::ui::{BackgroundColor, BorderRadius, Node, px};

    use super::*;
    use crate::tokens::{Curve, TextTokens, Tone};
    use crate::transition::{BevyMarker, ReducedMotion};
    use crate::views::{Label, label};
    use crate::{AnyView, FynixPlugin, StateExt, Theme, mount};
    use motiongfx_interp::interpolation::Interpolation;

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
            Color::srgb(0.2, 0.2, 0.2)
        }

        fn hover(&self) -> Color {
            Color::srgb(0.3, 0.3, 0.3)
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

    const REST: Color = Color::srgb(0.2, 0.2, 0.2);
    const HOVER: Color = Color::srgb(0.3, 0.3, 0.3);

    /// How many times [`Hovered`] was taken off a node.
    #[derive(Resource, Default)]
    struct Releases(usize);

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain))
        .insert_resource(TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(50),
        ))
        .init_resource::<Releases>()
        .add_systems(PreUpdate, update_is_hovered)
        .add_observer(
            |_: On<Remove, Hovered>,
             mut releases: ResMut<Releases>| {
                releases.0 += 1;
            },
        );
        // The first update only starts the clock.
        app.update();
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
