//! How Bevy's types travel under a transition.

use bevy::color::{Color, Oklaba};
use bevy::ecs::resource::Resource;
pub use fynix::{Axes, F32_AXES, OPACITY_AXES};

/// A colour, split into Oklab's lightness, a, b and alpha whatever
/// space it is stored in, so it blends evenly to the eye. It never
/// swings past its target.
pub const COLOR_AXES: Axes<Color> = Axes {
    split: |color| {
        let Oklaba {
            lightness,
            a,
            b,
            alpha,
        } = Oklaba::from(*color);
        [lightness, a, b, alpha]
    },
    join: |[lightness, a, b, alpha]| {
        Oklaba::new(lightness, a, b, alpha).into()
    },
    can_overshoot: false,
};

/// When set, every transition finishes at once.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ReducedMotion(pub bool);
