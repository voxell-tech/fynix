//! The props every element shares, in Bevy: opacity multiplies the
//! alpha of what an element draws, and scale is its
//! [`UiTransform`](bevy::ui::UiTransform), which its children inherit.

use bevy::color::{Alpha, Color};
use bevy::ecs::world::World;

/// The props every element shares, for rules that reach every kind of
/// element: `cx.set::<Visual>(|v, _| v.opacity(0.0))`.
pub type Visual = fynix::Visual<World>;

/// `color` drawn at `opacity`.
pub(crate) fn faded(color: Color, opacity: f32) -> Color {
    color.with_alpha(color.alpha() * opacity)
}
