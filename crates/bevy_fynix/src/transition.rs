//! How Bevy's types travel under a transition.

use bevy::color::{Color, Oklaba};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::ecs::world::World;
use fynix::Timing;
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

/// How the changes the next update sees travel, in place of the
/// transitions and presences that would move them, so the cause of a
/// change decides: a drag sets [`Timing::INSTANT`] as it writes, and
/// what follows the value keeps up with the pointer. Set it with the
/// change. It is cleared after every update.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ChangeTiming(pub Option<Timing>);

impl ChangeTiming {
    /// Puts the changes the next update sees in place at once.
    pub fn snap(world: &mut World) {
        world.resource_mut::<Self>().0 = Some(Timing::INSTANT);
    }
}

/// Lets go of the timing the changes of the update just run took.
pub(crate) fn clear_change_timing(mut timing: ResMut<ChangeTiming>) {
    timing.0 = None;
}
