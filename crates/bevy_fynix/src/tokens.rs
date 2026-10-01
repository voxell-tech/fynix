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
    /// Secondary or inactive text and icons.
    Dim,
    /// A fainter tier still, for a label beside a brighter one.
    Faint,
    /// What the eye should land on, and what is interactive.
    Accent,
    /// Destructive actions, and errors.
    Critical,
}

pub trait TextTokens {
    fn tone(&self, tone: Tone) -> Color;
    fn body_size(&self) -> f32;
    fn small_size(&self) -> f32;
}

pub trait SurfaceTokens {
    /// A filled control at rest.
    fn fill(&self) -> Color;
    /// What a surface travels to under the pointer.
    fn hover(&self) -> Color;
    /// Panels and popups.
    fn panel(&self) -> Color;

    /// Dividers and borders. The hover surface unless a theme says.
    fn hairline(&self) -> Color {
        self.hover()
    }

    /// A selected row's tint. The hover surface unless a theme says.
    fn selection(&self) -> Color {
        self.hover()
    }
}

pub trait SpacingTokens {
    fn gap(&self) -> f32;
    fn row(&self) -> f32;
    fn radius(&self) -> f32;

    /// The corner radius of a menu surface. The theme's radius unless
    /// a theme says.
    fn menu_radius(&self) -> f32 {
        self.radius()
    }

    /// The space between a menu surface's edge and its rows.
    fn menu_padding(&self) -> f32 {
        4.0
    }

    /// The corner radius of a menu row. The theme's radius unless a
    /// theme says.
    fn menu_item_radius(&self) -> f32 {
        self.radius()
    }

    /// How close a menu may sit to the window's edge.
    fn menu_margin(&self) -> f32 {
        8.0
    }
}
