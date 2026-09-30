//! The props every element shares, in Bevy: opacity multiplies the
//! alpha of what an element draws, and scale is its [`UiTransform`],
//! which its children inherit.

use bevy::color::{Alpha, Color};
use bevy::ecs::world::World;
use bevy::math::Vec2;
use bevy::ui::UiTransform;

/// The props every element shares, for rules that reach every kind of
/// element: `cx.set::<Visual>(|v, _| v.opacity(0.0))`.
pub type Visual = fynix::Visual<World>;

/// `color` drawn at `opacity`.
pub(crate) fn faded(color: Color, opacity: f32) -> Color {
    color.with_alpha(color.alpha() * opacity)
}

/// The transform of a node scaled by `scale` around its centre.
pub(crate) fn scaled(scale: f32) -> UiTransform {
    UiTransform::from_scale(Vec2::splat(scale))
}

/// Builder methods for the [`Visual`] props of an element with
/// `opacity` and `scale` fields, and its `visual` accessor for
/// [`Element`](crate::Element).
macro_rules! visual_props {
    () => {
        /// How opaque it is, 1.0 when unset.
        pub fn opacity(
            mut self,
            opacity: impl Into<Prop<f32>>,
        ) -> Self {
            self.opacity = opacity.into();
            self
        }

        /// The factor it is scaled by around its centre after layout,
        /// 1.0 when unset. Its children are scaled with it.
        pub fn scale(mut self, scale: impl Into<Prop<f32>>) -> Self {
            self.scale = scale.into();
            self
        }
    };
}

/// The `visual` method of [`Element`](crate::Element) for an element
/// with `opacity` and `scale` fields.
macro_rules! visual_access {
    () => {
        fn visual(
            &mut self,
        ) -> Option<fynix::VisualMut<'_, bevy::ecs::world::World>> {
            Some(fynix::VisualMut {
                opacity: &mut self.opacity,
                scale: &mut self.scale,
            })
        }
    };
}

pub(crate) use {visual_access, visual_props};
