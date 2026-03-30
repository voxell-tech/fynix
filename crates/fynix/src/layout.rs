// TODO: Should this be a u32?
/// A 2D size in resolved pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self::splat(0.0);

    pub const INFINITY: Self = Self::splat(f32::INFINITY);

    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    #[inline]
    pub const fn splat(value: f32) -> Self {
        Self::new(value, value)
    }
}

impl Default for Size {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self::splat(0.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn splat(value: f32) -> Self {
        Self::new(value, value)
    }
}

impl Default for Vec2 {
    fn default() -> Self {
        Self::ZERO
    }
}

/// A min/max size constraint passed down the element tree.
///
/// `max` fields set to [`f32::INFINITY`] indicate an unconstrained
/// axis. Use the constructor helpers [`Self::tight()`],
/// [`Self::loose`], [`Self::unbounded`] rather than constructing
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

    /// Bounded width, unbounded height
    /// (e.g. vertical scroll container).
    pub const fn fixed_width(width: f32) -> Self {
        Self {
            min: Size::ZERO,
            max: Size {
                width,
                height: f32::INFINITY,
            },
        }
    }

    /// Bounded height, unbounded width
    /// (e.g. horizontal scroll container).
    pub const fn fixed_height(height: f32) -> Self {
        Self {
            min: Size::ZERO,
            max: Size {
                width: f32::INFINITY,
                height,
            },
        }
    }

    /// Clamps `size` so it satisfies this constraint.
    pub const fn constrain(&self, size: Size) -> Size {
        Size {
            width: size.width.max(self.min.width).min(self.max.width),
            height: size
                .height
                .max(self.min.height)
                .min(self.max.height),
        }
    }
}

impl Default for Constraint {
    fn default() -> Self {
        Self::unbounded()
    }
}

pub trait Layouter {
    type Id;

    fn get_size(&self, id: &Self::Id) -> Size;

    fn set_position(&mut self, id: &Self::Id, position: Vec2);
}
