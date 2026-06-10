/// Identifies a single pointer in the unified pointer model.
///
/// Mouse and touch both lower into pointers here: the mouse is
/// [`PointerId::MOUSE`] (id 0) and each touch contact gets its own
/// id, so everything downstream is pointer-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointerId(pub u64);

impl PointerId {
    /// The mouse pointer.
    pub const MOUSE: Self = Self(0);
}

/// A pressable button on a pointer.
///
/// Touch contacts report [`PointerButton::Primary`], since they have
/// no distinct buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Other(u16),
}

/// A single raw input event from the backend, the only thing each
/// backend must emit.
///
/// `time` is a backend-supplied monotonic timestamp in milliseconds.
/// no_std has no clock, so recognizers rely on this for timing (e.g.
/// double-click intervals).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawInput {
    pub time: u64,
    pub kind: RawInputKind,
}

/// The payload of a [`RawInput`].
///
/// Only the pointer events are modelled so far; keyboard, text, IME,
/// scroll, touch phase, and gamepad variants are planned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RawInputKind {
    /// A pointer moved to `(x, y)` in absolute window space.
    PointerMoved { pointer: PointerId, x: f64, y: f64 },
    /// A pointer button went down at `(x, y)`.
    PointerDown {
        pointer: PointerId,
        button: PointerButton,
        x: f64,
        y: f64,
    },
    /// A pointer button went up at `(x, y)`.
    PointerUp {
        pointer: PointerId,
        button: PointerButton,
        x: f64,
        y: f64,
    },
}
