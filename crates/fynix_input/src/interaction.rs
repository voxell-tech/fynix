use crate::raw_input::{PointerButton, PointerId};

/// A pointer pressed and released over the same element.
///
/// Delivered to the element under the release via `fynix`'s dispatch,
/// so a handler attaches with `ctx.add::<E>().on::<Click>(..)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Click {
    pub pointer: PointerId,
    pub button: PointerButton,
    /// Release position relative to the clicked element's origin.
    pub local_x: f64,
    pub local_y: f64,
}

/// A pointer moved onto an element it was not over last frame.
///
/// Emitted once when the topmost hit-tested element under the pointer
/// changes, paired with a [`PointerLeave`] for the element left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEnter {
    pub pointer: PointerId,
}

/// A pointer moved off an element it was over last frame.
///
/// The counterpart to [`PointerEnter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerLeave {
    pub pointer: PointerId,
}
