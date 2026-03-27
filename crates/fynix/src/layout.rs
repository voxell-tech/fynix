// TODO: Should this be a u32?
/// A 2D size in resolved pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const INFINITY: Self = Self {
        width: f32::INFINITY,
        height: f32::INFINITY,
    };
}

/// A min/max size constraint passed down the element tree.
///
/// `max` fields set to [`f32::INFINITY`] indicate an
/// unconstrained axis. Use the constructor helpers
/// ([`tight`](Constraint::tight), [`loose`](Constraint::loose),
/// [`unbounded`](Constraint::unbounded)) rather than constructing
/// directly where possible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraint {
    pub min: Size,
    pub max: Size,
}

impl Constraint {
    /// Forces the child to be exactly `size`.
    pub const fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    /// Child may choose any size from zero up to `max`.
    pub const fn loose(max: Size) -> Self {
        Self {
            min: Size::ZERO,
            max,
        }
    }

    /// No bounds on either axis.
    pub const fn unbounded() -> Self {
        Self {
            min: Size::ZERO,
            max: Size::INFINITY,
        }
    }

    /// Clamps `size` so it satisfies this constraint.
    pub fn constrain(&self, size: Size) -> Size {
        Size {
            width: size.width.max(self.min.width).min(self.max.width),
            height: size
                .height
                .max(self.min.height)
                .min(self.max.height),
        }
    }
}
