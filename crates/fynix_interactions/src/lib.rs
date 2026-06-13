//! Input layer for fynix.
//!
//! Turns raw platform input into the semantic interaction types that
//! element handlers consume. The backend emits [`RawInput`], the
//! stateful recognizers in this crate resolve it against a
//! [`HitTest`], and matched interactions are delivered through
//! `fynix`'s existing dispatch.

#![no_std]

extern crate alloc;

use fynix::element::ElementId;
use fynix::interaction::Interactions;

pub mod hit_test;
pub mod interaction;
pub mod pointer;
pub mod raw_input;
pub mod recognizer;

pub use hit_test::{Hit, HitTest};
pub use interaction::{Click, PointerEnter, PointerLeave};
pub use raw_input::{
    PointerButton, PointerId, RawInput, RawInputKind,
};
pub use recognizer::PointerRecognizer;

/// Returns `true` if `id` handles any hit-tested interaction, and so
/// must be indexed by [`HitTest::build`].
///
/// Most of a scene is static, so only elements with a pointer handler
/// belong in the spatree. This is the default `include` predicate:
/// pass it (closed over the [`Interactions`]) to [`HitTest::build`].
/// Extend the list as new hit-tested interactions are added.
pub fn is_hit_target(
    interactions: &Interactions,
    id: &ElementId,
) -> bool {
    interactions.contains::<Click>(id)
        || interactions.contains::<PointerEnter>(id)
        || interactions.contains::<PointerLeave>(id)
}
