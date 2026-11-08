//! Blending for Bevy's types, on the core's `Interpolation`.

use bevy::color::{Color, Mix, Oklaba};
use bevy::ecs::resource::Resource;
use motiongfx_interp::interpolation::Interpolation;

/// Marks the interpolations this crate adds for Bevy's types, which
/// the orphan rule keeps from taking the default marker.
pub struct BevyMarker;

/// Blended in Oklab, whatever space either colour is stored in.
impl Interpolation<BevyMarker> for Color {
    fn interp(a: &Self, b: &Self, t: f32) -> Self {
        Oklaba::from(*a).mix(&Oklaba::from(*b), t).into()
    }
}

/// How two colours blend, for `#[elem(blend = ..)]`.
pub fn blend_color(from: &Color, to: &Color, t: f32) -> Color {
    <Color as Interpolation<BevyMarker>>::interp(from, to, t)
}

/// How two numbers blend, for `#[elem(blend = ..)]`.
pub fn blend_f32(from: &f32, to: &f32, t: f32) -> f32 {
    <f32 as Interpolation<()>>::interp(from, to, t)
}

/// When set, every transition finishes at once.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ReducedMotion(pub bool);
