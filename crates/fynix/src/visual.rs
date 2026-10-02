//! The props every element shares: how opaque it is, and how it is
//! scaled around its centre after layout.

use crate::prop::Prop;
use crate::view::{Layered, Settable, Styled};

/// What rules for every kind of element set: `cx.set::<Visual<W>>`
/// reaches each element that exposes these props, whatever its kind.
/// A rule for the element's own kind beats it.
pub struct Visual<W> {
    /// 1.0, fully opaque, when unset.
    pub opacity: Prop<W, f32>,
    /// 1.0 when unset.
    pub scale: Prop<W, f32>,
}

impl<W> Visual<W> {
    pub fn opacity(
        mut self,
        opacity: impl Into<Prop<W, f32>>,
    ) -> Self {
        self.opacity = opacity.into();
        self
    }

    pub fn scale(mut self, scale: impl Into<Prop<W, f32>>) -> Self {
        self.scale = scale.into();
        self
    }

    /// Which bound props may have changed, as `bits` numbers them:
    /// opacity, then scale. Both checks run.
    pub fn changed_bits(&mut self, world: &W, bits: [u64; 2]) -> u64 {
        let opacity = self.opacity.changed(world);
        let scale = self.scale.changed(world);
        (if opacity { bits[0] } else { 0 })
            | (if scale { bits[1] } else { 0 })
    }
}

impl<W: 'static> Styled for Visual<W> {
    fn unset() -> Self {
        Self {
            opacity: Prop::Unset,
            scale: Prop::Unset,
        }
    }

    fn over(self, below: Self) -> Self {
        Self {
            opacity: self.opacity.or(below.opacity),
            scale: self.scale.or(below.scale),
        }
    }
}

impl<W: 'static> Layered for Visual<W> {
    fn set_mask(&self) -> u64 {
        self.opacity.is_set() as u64
            | (self.scale.is_set() as u64) << 1
    }

    fn swap_props(&mut self, other: &mut Self, mask: u64) {
        let mut this = self.as_mut();
        this.swap_props(other, mask);
    }
}

impl<W> Visual<W> {
    fn as_mut(&mut self) -> VisualMut<'_, W> {
        VisualMut {
            opacity: &mut self.opacity,
            scale: &mut self.scale,
        }
    }
}

/// An element's own [`Visual`] props, lent out so rules for every
/// kind of element can reach them.
pub struct VisualMut<'a, W> {
    pub opacity: &'a mut Prop<W, f32>,
    pub scale: &'a mut Prop<W, f32>,
}

impl<W> VisualMut<'_, W> {
    /// The bits of the props this sets, numbered as [`Visual`]'s.
    pub fn set_mask(&self) -> u64 {
        self.opacity.is_set() as u64
            | (self.scale.is_set() as u64) << 1
    }

    /// Swaps the props whose bits are in `mask` with `other`'s.
    pub fn swap_props(&mut self, other: &mut Visual<W>, mask: u64) {
        if mask & 1 != 0 {
            core::mem::swap(self.opacity, &mut other.opacity);
        }
        if mask & 2 != 0 {
            core::mem::swap(self.scale, &mut other.scale);
        }
    }

    /// Takes each prop left unset from `below`.
    pub fn fill(&mut self, below: Visual<W>) {
        let Visual { opacity, scale } = below;
        let unset = |prop: &mut Prop<W, f32>, below| {
            if prop.is_unset() {
                *prop = below;
            }
        };
        unset(self.opacity, opacity);
        unset(self.scale, scale);
    }
}
