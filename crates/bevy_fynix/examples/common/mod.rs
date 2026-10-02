//! The theme the examples share.

use core::time::Duration;

use bevy::color::Color;
use bevy_fynix::tokens::{
    Curve, Motion, MotionTokens, SpacingTokens, SurfaceTokens,
    TextTokens, Tone,
};

/// The examples' theme. Nothing in the views names it: they ask for
/// whatever token traits they need, and this implements them.
pub struct Monokai;

fn hex(rgb: u32) -> Color {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color::srgb_u8(r, g, b)
}

impl TextTokens for Monokai {
    fn tone(&self, tone: Tone) -> Color {
        match tone {
            Tone::Body => hex(0xFCFCFA),
            Tone::Dim => hex(0x939293),
            Tone::Faint => hex(0x727072),
            Tone::Accent => hex(0xFFD866),
            Tone::OnAccent => hex(0x2D2A2E),
            Tone::Critical => hex(0xFF6188),
        }
    }

    fn body_size(&self) -> f32 {
        14.0
    }

    fn small_size(&self) -> f32 {
        11.0
    }
}

impl SurfaceTokens for Monokai {
    fn fill(&self) -> Color {
        hex(0x403E41)
    }

    fn hover(&self) -> Color {
        hex(0x5B595C)
    }

    fn panel(&self) -> Color {
        hex(0x221F22)
    }

    fn accent(&self) -> Color {
        hex(0xFFD866)
    }
}

impl SpacingTokens for Monokai {
    fn gap(&self) -> f32 {
        8.0
    }

    fn row(&self) -> f32 {
        24.0
    }

    fn radius(&self) -> f32 {
        4.0
    }
}

impl MotionTokens for Monokai {
    fn motion(&self, motion: Motion) -> Curve {
        let millis = match motion {
            Motion::Interact => 180,
            Motion::Expand => 280,
        };
        Curve {
            duration: Duration::from_millis(millis),
            // Ease out: quick to answer, gentle to land.
            ease: |t| 1.0 - (1.0 - t) * (1.0 - t),
        }
    }
}
