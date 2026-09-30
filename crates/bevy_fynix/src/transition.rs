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

/// When set, every transition finishes at once.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ReducedMotion(pub bool);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_and_colors_blend() {
        assert_eq!(
            <f32 as Interpolation<()>>::interp(&10.0, &20.0, 0.25),
            12.5
        );
        let black = Color::BLACK;
        let white = Color::srgb(1.0, 1.0, 1.0);

        let end = <Color as Interpolation<BevyMarker>>::interp(
            &black, &white, 1.0,
        )
        .to_srgba();
        assert!((end.red - 1.0).abs() < 1e-4);
        let mid = <Color as Interpolation<BevyMarker>>::interp(
            &black, &white, 0.5,
        )
        .to_srgba();
        assert!(mid.red > 0.0 && mid.red < 1.0);
        assert!((mid.red - mid.green).abs() < 1e-4);
    }
}
