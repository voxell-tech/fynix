//! What views read from a theme, as small traits a theme implements.
//! A view bounds only the traits its defaults read, so it works under
//! any theme that can answer for them.

use bevy::color::{Color, Luminance};
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
    /// Text and icons drawn on a solid accent surface.
    OnAccent,
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

    /// A surface while a pointer button is down on it. The hover
    /// surface, darker, unless a theme says.
    fn pressed(&self) -> Color {
        self.hover().darker(0.1)
    }

    /// Dividers and borders. The hover surface unless a theme says.
    fn hairline(&self) -> Color {
        self.hover()
    }

    /// A selected row's tint. The hover surface unless a theme says.
    fn selection(&self) -> Color {
        self.hover()
    }

    /// A solid accent surface, such as the active segment. The
    /// selection tint unless a theme says.
    fn accent(&self) -> Color {
        self.selection()
    }

    /// The accent surface under the pointer. The accent, lighter,
    /// unless a theme says.
    fn accent_hover(&self) -> Color {
        self.accent().lighter(0.08)
    }

    /// The accent surface while pressed. The accent, darker, unless
    /// a theme says.
    fn accent_pressed(&self) -> Color {
        self.accent().darker(0.1)
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

    /// The least width of a menu's list.
    fn menu_width(&self) -> f32 {
        120.0
    }

    /// A divider's thickness.
    fn divider(&self) -> f32 {
        2.0
    }

    /// The thickness of the strip that grabs a divider, centred on
    /// it. Four times the divider unless a theme says.
    fn divider_grip(&self) -> f32 {
        self.divider() * 4.0
    }

    /// The space at each side of a tab's content.
    fn tab_padding(&self) -> f32 {
        8.0
    }

    /// The space between two tabs.
    fn tab_gap(&self) -> f32 {
        2.0
    }
}
