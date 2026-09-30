//! What views read from a theme, as small traits a theme implements.
//! A view bounds only the traits its defaults read, so it works under
//! any theme that can answer for them.

use bevy::color::Color;
pub use fynix::{Curve, Motion, MotionTokens};

/// A text colour by role, so a view can ask for "dim" without knowing
/// what dim is in a given theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Body,
    Dim,
    Accent,
}

pub trait TextTokens {
    fn tone(&self, tone: Tone) -> Color;
    fn body_size(&self) -> f32;
    fn small_size(&self) -> f32;
}

pub trait SurfaceTokens {
    fn fill(&self) -> Color;
    fn hover(&self) -> Color;
    fn panel(&self) -> Color;
}

pub trait SpacingTokens {
    fn gap(&self) -> f32;
    fn row(&self) -> f32;
    fn radius(&self) -> f32;
}
