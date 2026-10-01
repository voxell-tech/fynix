use bevy::asset::Handle;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::math::Rot2;
use bevy::ui::widget::ImageNode;
use bevy::ui::{Node, UiTransform, px};

use motiongfx_interp::interpolation::{InterpFn, Interpolation};

use crate::leave::Collapsing;
use crate::prop::Prop;
use crate::props::props;
use crate::state::own_when;
use crate::tokens::{TextTokens, Tone};
use crate::transition::BevyMarker;
use crate::visual::{faded, scaled, visual_access};
use crate::{Bevy, Element, Styled};

props! {
    /// A square image tinted by a text tone.
    pub struct Icon {
        image: Handle<Image>,
        /// The length of each side. The theme's body size when unset.
        size: f32,
        /// The tint, by role. Body when unset.
        tone: Tone,
        /// Clockwise, in degrees.
        rotation: f32,
        /// How opaque it is, 1.0 when unset.
        opacity: f32,
        /// The factor it is scaled by around its centre after layout,
        /// 1.0 when unset.
        scale: f32,
    }
}

pub fn icon(image: impl Into<Prop<Handle<Image>>>) -> Icon {
    Icon {
        image: image.into(),
        ..Icon::unset()
    }
}

own_when!(Icon);

/// An [`Icon`]'s props at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct IconSnapshot {
    pub image: Handle<Image>,
    pub size: f32,
    pub color: Color,
    pub rotation: f32,
    pub opacity: f32,
    pub scale: f32,
}

/// Everything but the image blends, and the image takes the target.
impl Interpolation<BevyMarker> for IconSnapshot {
    fn interp(from: &Self, to: &Self, t: f32) -> Self {
        let float = |from: &f32, to: &f32| {
            <f32 as Interpolation<()>>::interp(from, to, t)
        };
        Self {
            image: to.image.clone(),
            size: float(&from.size, &to.size),
            color: <Color as Interpolation<BevyMarker>>::interp(
                &from.color,
                &to.color,
                t,
            ),
            rotation: float(&from.rotation, &to.rotation),
            opacity: float(&from.opacity, &to.opacity),
            scale: float(&from.scale, &to.scale),
        }
    }
}

impl<T: TextTokens> Element<Bevy, T> for Icon {
    type Snapshot = IconSnapshot;

    fn prepare(world: &mut World, node: Entity) {
        world.entity_mut(node).insert(ImageNode::default());
    }

    fn snapshot(&self, world: &World, theme: &T) -> IconSnapshot {
        let tone = self.tone.get(world).unwrap_or_default();
        IconSnapshot {
            image: self.image.get(world).unwrap_or_default(),
            size: self.size.get(world).unwrap_or(theme.body_size()),
            color: theme.tone(tone),
            rotation: self.rotation.get(world).unwrap_or(0.0),
            opacity: self.opacity.get(world).unwrap_or(1.0),
            scale: self.scale.get(world).unwrap_or(1.0),
        }
    }

    fn write(
        snapshot: &IconSnapshot,
        world: &mut World,
        node: Entity,
    ) {
        let mut entity = world.entity_mut(node);
        // A collapsing node's size is the collapse's to write.
        let sized = !entity.contains::<Collapsing>();
        if let Some(mut ui) = entity.get_mut::<Node>()
            && sized
        {
            ui.width = px(snapshot.size);
            ui.height = px(snapshot.size);
        }
        entity.insert((
            ImageNode::new(snapshot.image.clone())
                .with_color(faded(snapshot.color, snapshot.opacity)),
            UiTransform {
                rotation: Rot2::degrees(snapshot.rotation),
                ..scaled(snapshot.scale)
            },
        ));
    }

    fn is_live(&self) -> bool {
        self.any_bound()
    }

    fn changed(&mut self, world: &World) -> bool {
        self.any_changed(world)
    }

    fn interp() -> Option<InterpFn<IconSnapshot>> {
        Some(<IconSnapshot as Interpolation<BevyMarker>>::interp)
    }

    visual_access!();
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::relationship::RelationshipTarget;
    use bevy::time::TimePlugin;
    use bevy::ui::Val;

    use super::*;
    use crate::{AnyView, FynixPlugin, Theme, mount};

    struct Plain;

    impl TextTokens for Plain {
        fn tone(&self, tone: Tone) -> Color {
            match tone {
                Tone::Body => Color::WHITE,
                Tone::Dim | Tone::Faint => Color::srgb(0.5, 0.5, 0.5),
                Tone::Accent | Tone::Critical => {
                    Color::srgb(1.0, 0.5, 0.0)
                }
            }
        }

        fn body_size(&self) -> f32 {
            14.0
        }

        fn small_size(&self) -> f32 {
            11.0
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            FynixPlugin::<Plain>::default(),
        ))
        .insert_resource(Theme(Plain));
        app
    }

    fn image_node(app: &App, node: Entity) -> &ImageNode {
        app.world().get::<ImageNode>(node).expect("an icon")
    }

    #[test]
    fn unset_props_fall_back_to_the_theme() {
        let mut app = app();
        let node =
            mount::<Plain>(app.world_mut(), icon(Handle::default()));

        assert_eq!(image_node(&app, node).color, Color::WHITE);
        let ui = app.world().get::<Node>(node).unwrap();
        assert_eq!(ui.width, Val::Px(14.0));
        assert_eq!(ui.height, Val::Px(14.0));
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
    fn a_set_rule_beats_the_theme_and_the_call_site_beats_it() {
        let mut app = app();
        let root = mount::<Plain>(
            app.world_mut(),
            AnyView::<Bevy, Plain>::new(|cx| {
                let root = cx.spawn();
                cx.under(root, |cx| {
                    cx.set::<Icon>(|i, _| {
                        i.size(30.0).tone(Tone::Dim)
                    });
                    cx.build(icon(Handle::default()));
                    cx.build(icon(Handle::default()).size(8.0));
                });
                root
            }),
        );
        let kids = app
            .world()
            .get::<Children>(root)
            .unwrap()
            .iter()
            .collect::<Vec<_>>();

        let width =
            |node| app.world().get::<Node>(node).unwrap().width;
        assert_eq!(width(kids[0]), Val::Px(30.0));
        assert_eq!(width(kids[1]), Val::Px(8.0));
        assert_eq!(
            image_node(&app, kids[1]).color,
            Color::srgb(0.5, 0.5, 0.5)
        );
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
