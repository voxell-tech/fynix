use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::text::{
    FontSize, FontWeight, LineBreak, TextColor, TextFont, TextLayout,
};
use bevy::ui::widget::Text;
use fynix::element;

use crate::patch::{
    Paint, PatchInk, PatchOpacity, PatchScale, patch,
};
use crate::prop::Prop;
use crate::state::own_when;
use crate::tokens::{TextTokens, Tone};
use crate::transition::{blend_color, blend_f32};
use crate::{Bevy, Styled};

/// A run of text.
#[element(backend = Bevy, theme = TextTokens, prepare = prepare)]
pub struct Label {
    #[elem(patch = PatchText)]
    pub text: Prop<String>,
    /// The theme's body size when unset.
    #[elem(
        default = theme.body_size(),
        patch = PatchTextSize,
        blend = blend_f32
    )]
    pub size: Prop<f32>,
    /// The colour, by role. Body when unset.
    #[elem(
        shown = Color,
        with = |tone, theme| theme.tone(tone),
        patch = PatchInk,
        blend = blend_color
    )]
    pub tone: Prop<Tone>,
    #[elem(patch = PatchBold)]
    pub bold: Prop<bool>,
    /// Whether it breaks onto more lines. It does when unset.
    #[elem(default = true, patch = PatchWrap)]
    pub wrap: Prop<bool>,
    /// How opaque it is, 1.0 when unset.
    #[elem(default = 1.0, patch = PatchOpacity, blend = blend_f32)]
    pub opacity: Prop<f32>,
    /// The factor it is scaled by around its centre after layout,
    /// 1.0 when unset.
    #[elem(default = 1.0, patch = PatchScale, blend = blend_f32)]
    pub scale: Prop<f32>,
}

pub fn label(text: impl Into<Prop<String>>) -> Label {
    Label {
        text: text.into(),
        ..Label::unset()
    }
}

fn prepare(world: &mut World, node: Entity) {
    world.entity_mut(node).insert((
        Text::default(),
        TextFont::default(),
        TextColor::default(),
        TextLayout::default(),
        Paint::default(),
    ));
}

own_when!(Label);

patch!(PatchText, String, |entity, v| {
    if let Some(mut text) = entity.get_mut::<Text>() {
        text.0.clone_from(v);
    }
});
patch!(PatchTextSize, f32, |entity, v| {
    if let Some(mut font) = entity.get_mut::<TextFont>() {
        font.font_size = FontSize::Px(*v);
    }
});
patch!(PatchBold, bool, |entity, v| {
    if let Some(mut font) = entity.get_mut::<TextFont>() {
        font.weight = if *v {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
    }
});
patch!(PatchWrap, bool, |entity, v| {
    if let Some(mut layout) = entity.get_mut::<TextLayout>() {
        layout.linebreak = if *v {
            LineBreak::WordBoundary
        } else {
            LineBreak::NoWrap
        };
    }
});

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::color::Alpha;
    use bevy::math::Vec2;
    use bevy::ui::UiTransform;

    use super::*;
    use crate::tests::{Plain, app};
    use crate::tokens::Motion;
    use crate::{Hovered, ScopedExt, mount};

    fn scale(app: &App, node: Entity) -> Vec2 {
        app.world().get::<UiTransform>(node).unwrap().scale
    }

    fn grow(label: Label, _: &Plain) -> Label {
        label.scale(2.0)
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
}
