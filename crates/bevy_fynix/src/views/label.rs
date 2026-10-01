use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::text::{
    FontSize, FontWeight, LineBreak, TextColor, TextFont, TextLayout,
};
use bevy::ui::widget::Text;
use motiongfx_interp::interpolation::{InterpFn, Interpolation};

use crate::prop::Prop;
use crate::props::props;
use crate::state::own_when;
use crate::tokens::{TextTokens, Tone};
use crate::transition::BevyMarker;
use crate::visual::{faded, scaled, visual_access};
use crate::{Bevy, Element, Styled};

props! {
    /// A run of text.
    pub struct Label {
        text: String,
        /// The theme's body size when unset.
        size: f32,
        /// The colour, by role. Body when unset.
        tone: Tone,
        bold: bool,
        /// Whether it breaks onto more lines. It does when unset.
        wrap: bool,
        /// How opaque it is, 1.0 when unset.
        opacity: f32,
        /// The factor it is scaled by around its centre after layout,
        /// 1.0 when unset.
        scale: f32,
    }
}

pub fn label(text: impl Into<Prop<String>>) -> Label {
    Label {
        text: text.into(),
        ..Label::unset()
    }
}

own_when!(Label);

/// A [`Label`]'s props at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelSnapshot {
    pub text: String,
    pub size: f32,
    pub color: Color,
    pub bold: bool,
    pub wrap: bool,
    pub opacity: f32,
    pub scale: f32,
}

impl Interpolation<BevyMarker> for LabelSnapshot {
    fn interp(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            text: to.text.clone(),
            bold: to.bold,
            size: <f32 as Interpolation<()>>::interp(
                &from.size, &to.size, t,
            ),
            color: <Color as Interpolation<BevyMarker>>::interp(
                &from.color,
                &to.color,
                t,
            ),
            wrap: to.wrap,
            opacity: <f32 as Interpolation<()>>::interp(
                &from.opacity,
                &to.opacity,
                t,
            ),
            scale: <f32 as Interpolation<()>>::interp(
                &from.scale,
                &to.scale,
                t,
            ),
        }
    }
}

impl<T: TextTokens> Element<Bevy, T> for Label {
    type Snapshot = LabelSnapshot;

    fn prepare(world: &mut World, node: Entity) {
        world.entity_mut(node).insert((
            Text::default(),
            TextFont::default(),
            TextColor::default(),
            TextLayout::default(),
        ));
    }

    fn snapshot(&self, world: &World, theme: &T) -> LabelSnapshot {
        let tone = self.tone.get(world).unwrap_or_default();
        LabelSnapshot {
            text: self.text.get(world).unwrap_or_default(),
            size: self.size.get(world).unwrap_or(theme.body_size()),
            color: theme.tone(tone),
            bold: self.bold.get(world).unwrap_or(false),
            wrap: self.wrap.get(world).unwrap_or(true),
            opacity: self.opacity.get(world).unwrap_or(1.0),
            scale: self.scale.get(world).unwrap_or(1.0),
        }
    }

    fn write(
        snapshot: &LabelSnapshot,
        world: &mut World,
        node: Entity,
    ) {
        let linebreak = if snapshot.wrap {
            LineBreak::WordBoundary
        } else {
            LineBreak::NoWrap
        };
        world.entity_mut(node).insert((
            Text::new(snapshot.text.clone()),
            TextFont {
                font_size: FontSize::Px(snapshot.size),
                weight: if snapshot.bold {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                },
                ..Default::default()
            },
            TextColor(faded(snapshot.color, snapshot.opacity)),
            TextLayout {
                linebreak,
                ..Default::default()
            },
            scaled(snapshot.scale),
        ));
    }

    fn is_live(&self) -> bool {
        self.any_bound()
    }

    fn changed(&mut self, world: &World) -> bool {
        self.any_changed(world)
    }

    fn interp() -> Option<InterpFn<LabelSnapshot>> {
        Some(<LabelSnapshot as Interpolation<BevyMarker>>::interp)
    }

    visual_access!();
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use bevy::app::App;
    use bevy::color::Alpha;
    use bevy::math::Vec2;
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use bevy::ui::UiTransform;

    use super::*;
    use crate::tokens::{Curve, Motion, MotionTokens};
    use crate::transition::ReducedMotion;
    use crate::{FynixPlugin, Hovered, ScopedExt, Theme, mount};

    struct Plain;

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

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain))
        .insert_resource(
            TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(50),
            ),
        );
        // The first update only starts the clock.
        app.update();
        app
    }

    fn scale(app: &App, node: Entity) -> Vec2 {
        app.world().get::<UiTransform>(node).unwrap().scale
    }

    fn grow(label: Label, _: &Plain) -> Label {
        label.scale(2.0)
    }

    #[test]
    fn bold_sets_the_font_weight() {
        let mut app = app();
        let plain = mount::<Plain>(app.world_mut(), label("x"));
        let bold =
            mount::<Plain>(app.world_mut(), label("x").bold(true));

        let weight =
            |node| app.world().get::<TextFont>(node).unwrap().weight;
        assert_eq!(weight(plain), FontWeight::NORMAL);
        assert_eq!(weight(bold), FontWeight::BOLD);
    }

    #[test]
    fn an_unset_scale_is_the_identity_transform() {
        let mut app = app();
        let node = mount::<Plain>(app.world_mut(), label("x"));

        assert_eq!(
            app.world().get::<UiTransform>(node),
            Some(&UiTransform::IDENTITY)
        );
    }

    #[test]
    fn a_scale_is_written_to_both_axes_and_nothing_else() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), label("x").scale(1.2));

        assert_eq!(
            app.world().get::<UiTransform>(node),
            Some(&UiTransform::from_scale(Vec2::splat(1.2)))
        );
    }

    #[test]
    fn a_hover_rule_moves_the_scale_over_the_curve_not_the_size() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(grow)
                .transition(Motion::Interact),
        );

        app.world_mut().entity_mut(node).insert(Hovered);
        app.update();
        assert_eq!(
            scale(&app, node),
            Vec2::splat(1.5),
            "50ms of 100ms"
        );
        let font = app.world().get::<TextFont>(node).unwrap();
        assert_eq!(font.font_size, FontSize::Px(14.0));

        app.update();
        assert_eq!(scale(&app, node), Vec2::splat(2.0));
    }

    #[test]
    fn reduced_motion_snaps_the_scale() {
        let mut app = app();
        app.insert_resource(ReducedMotion(true));
        let node = mount::<Plain>(
            app.world_mut(),
            label("x")
                .when::<Hovered, _>(grow)
                .transition(Motion::Interact),
        );

        app.world_mut().entity_mut(node).insert(Hovered);
        app.update();

        assert_eq!(scale(&app, node), Vec2::splat(2.0));
    }

    #[test]
    fn opacity_multiplies_the_text_alpha() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), label("x").opacity(0.25));

        let color = app.world().get::<TextColor>(node).unwrap().0;
        assert_eq!(color, Color::WHITE.with_alpha(0.25));
    }

    #[test]
    fn a_rule_for_every_element_reaches_a_label() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            crate::AnyView::<Bevy, Plain>::new(|cx| {
                cx.set::<crate::Visual>(|v, _| {
                    v.opacity(0.5).scale(3.0)
                });
                cx.set::<Label>(|l, _| l.scale(2.0));
                cx.build(label("x"))
            }),
        );

        let color = app.world().get::<TextColor>(root).unwrap().0;
        assert_eq!(color, Color::WHITE.with_alpha(0.5));
        assert_eq!(
            scale(&app, root),
            Vec2::splat(2.0),
            "a rule for labels beats one for every element"
        );
    }

    #[test]
    fn size_color_opacity_and_scale_blend_while_text_and_wrap_snap() {
        let from = LabelSnapshot {
            text: "a".into(),
            size: 10.0,
            color: Color::BLACK,
            bold: false,
            wrap: true,
            opacity: 0.0,
            scale: 1.0,
        };
        let to = LabelSnapshot {
            text: "b".into(),
            size: 20.0,
            color: Color::WHITE,
            bold: true,
            wrap: false,
            opacity: 1.0,
            scale: 2.0,
        };

        let mid =
            <LabelSnapshot as Interpolation<BevyMarker>>::interp(
                &from, &to, 0.5,
            );

        assert_eq!(mid.text, "b");
        assert_eq!(mid.size, 15.0);
        assert_eq!(mid.opacity, 0.5);
        assert_eq!(mid.scale, 1.5);
        assert_eq!(
            mid.color,
            <Color as Interpolation<BevyMarker>>::interp(
                &from.color,
                &to.color,
                0.5
            )
        );
        assert!(!mid.wrap);
    }
}
