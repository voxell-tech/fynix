use spatree::kurbo::Point;

/// A primary-button press and release over the same element.
///
/// Delivered to the element under the release via `fynix`'s dispatch,
/// so a handler attaches with `ctx.add::<E>().on::<Click>(..)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Click {
    /// Release position relative to the clicked element's origin.
    pub local: Point,
}

/// A secondary-button press and release over the same element.
///
/// The role of a button is decided by the backend through
/// [`ClassifyButton`](crate::pointer::ClassifyButton); this fires
/// when that role is [`ButtonRole::Secondary`].
///
/// [`ButtonRole::Secondary`]: crate::pointer::ButtonRole::Secondary
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondaryClick {
    /// Release position relative to the clicked element's origin.
    pub local: Point,
}

/// A pointer moved onto an element it was not over last frame.
///
/// Emitted once when the topmost hit-tested element under the pointer
/// changes, paired with a [`PointerLeave`] for the element left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEnter;

/// A pointer moved off an element it was over last frame.
///
/// The counterpart to [`PointerEnter`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerLeave;
