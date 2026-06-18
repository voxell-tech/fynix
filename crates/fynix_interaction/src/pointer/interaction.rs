use spatree::kurbo::{Point, Vec2};

/// A press-then-release on the same target within click slop, for any
/// button.
///
/// Button specificity comes from the typed variants [`PrimaryClick`]
/// and [`SecondaryClick`]; the originating button is classified inside
/// the recognizer and never surfaced here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawClick<Id> {
    pub id: Id,
    /// Release position relative to the target's origin.
    pub local: Point,
}

/// A [`RawClick`] whose button is the primary button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimaryClick<Id> {
    pub id: Id,
    pub local: Point,
}

/// A [`RawClick`] whose button is the secondary button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondaryClick<Id> {
    pub id: Id,
    pub local: Point,
}

/// The pointer crossed onto the element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEnter<Id> {
    pub id: Id,
    pub local: Point,
}

/// The pointer crossed off the element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerLeave<Id> {
    pub id: Id,
}

/// The pointer moved while over the element and not dragging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover<Id> {
    pub id: Id,
    pub local: Point,
}

/// A drag began: the pointer moved past the drag threshold while
/// pressed. The pointer is now captured to the press target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragStart<Id> {
    pub id: Id,
    /// Where the press began, in absolute coordinates.
    pub start: Point,
    /// Press position relative to the target's origin.
    pub local: Point,
}

/// A move during an active drag, delivered to the captured target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drag<Id> {
    pub id: Id,
    /// Current position relative to the target's origin (may fall
    /// outside the target while captured).
    pub local: Point,
    /// Movement since the previous drag event.
    pub delta: Vec2,
}

/// A release ending an active drag, delivered to the captured target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragEnd<Id> {
    pub id: Id,
    /// Release position relative to the target's origin.
    pub local: Point,
}
