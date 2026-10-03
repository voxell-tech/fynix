use bevy::asset::Handle;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::ui::widget::ImageNode;
use fynix::element;

use crate::patch::{
    Paint, PatchInk, PatchOpacity, PatchRotation, PatchScale,
    PatchSquare, PatchTint, patch,
};
use crate::prop::Prop;
use crate::state::own_when;
use crate::tokens::{TextTokens, Tone};
use crate::transition::{blend_color, blend_f32};
use crate::{Bevy, Styled};

/// A square image tinted by a text tone.
#[element(backend = Bevy, theme = TextTokens, prepare = prepare)]
pub struct Icon {
    #[elem(patch = PatchImage)]
    pub image: Prop<Handle<Image>>,
    /// The length of each side. The theme's body size when unset.
    #[elem(
        default = theme.body_size(),
        patch = PatchSquare,
        blend = blend_f32
    )]
    pub size: Prop<f32>,
    /// The tint, by role. Body when unset.
    #[elem(
        shown = Color,
        with = |tone, theme| theme.tone(tone),
        patch = PatchInk,
        blend = blend_color
    )]
    pub tone: Prop<Tone>,
    /// A colour that replaces the tone's while set.
    #[elem(patch = PatchTint)]
    pub tint: Prop<Option<Color>>,
    /// Clockwise, in degrees.
    #[elem(patch = PatchRotation, blend = blend_f32)]
    pub rotation: Prop<f32>,
    /// How opaque it is, 1.0 when unset.
    #[elem(default = 1.0, patch = PatchOpacity, blend = blend_f32)]
    pub opacity: Prop<f32>,
    /// The factor it is scaled by around its centre after layout,
    /// 1.0 when unset.
    #[elem(default = 1.0, patch = PatchScale, blend = blend_f32)]
    pub scale: Prop<f32>,
}

pub fn icon(image: impl Into<Prop<Handle<Image>>>) -> Icon {
    Icon {
        image: image.into(),
        ..Icon::unset()
    }
}

fn prepare(world: &mut World, node: Entity) {
    world
        .entity_mut(node)
        .insert((ImageNode::default(), Paint::default()));
}

own_when!(Icon);

patch!(PatchImage, Handle<Image>, |entity, v| {
    if let Some(mut image) = entity.get_mut::<ImageNode>() {
        image.image = v.clone();
    }
});

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::math::Rot2;
    use bevy::ui::{Node, UiTransform, Val};

    use super::*;
    use crate::mount;
    use crate::tests::{Plain, app};

    fn image_node(app: &App, node: Entity) -> &ImageNode {
        app.world().get::<ImageNode>(node).expect("an icon")
    }

    #[test]
    fn props_are_written_and_the_tone_picks_the_colour() {
        let mut app = app();
        let handle = Handle::<Image>::default();
        let node = mount::<Plain>(
            app.world_mut(),
            icon(handle.clone()).size(20.0).tone(Tone::Accent),
        );

        assert_eq!(image_node(&app, node).image, handle);
        assert_eq!(
            image_node(&app, node).color,
            Color::srgb(1.0, 0.5, 0.0)
        );
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.width, Val::Px(20.0));
    }

    #[test]
    fn a_tint_replaces_the_tone_colour() {
        let mut app = app();
        let tint = Color::srgb(0.2, 0.4, 0.6);
        let node = mount::<Plain>(
            app.world_mut(),
            icon(Handle::default())
                .tone(Tone::Accent)
                .tint(Some(tint)),
        );

        assert_eq!(image_node(&app, node).color, tint);
    }

    #[test]
    fn a_rotation_is_written_to_the_transform_in_degrees() {
        let mut app = app();
        let node = mount::<Plain>(
            app.world_mut(),
            icon(Handle::default()).rotation(90.0),
        );

        assert_eq!(
            app.world().get::<UiTransform>(node).unwrap().rotation,
            Rot2::degrees(90.0)
        );
    }
}
