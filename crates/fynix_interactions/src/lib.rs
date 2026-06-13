//! Interaction layer for fynix.
//!
//! Turns semantic pointer actions into the interaction types element
//! handlers consume. A backend translates its native input into calls
//! on [`PointerRecognizer`], which resolves them against a
//! [`HitTest`] and delivers matched interactions through `fynix`'s
//! dispatch.

#![no_std]

extern crate alloc;

use fynix::element::ElementId;
use fynix::interaction::Interactions;

pub mod interaction;
pub mod pointer;

pub use fynix_hit_test::{Hit, HitTest};
pub use interaction::{
    Click, PointerEnter, PointerLeave, SecondaryClick,
};
pub use pointer::{ButtonRole, ClassifyButton, PointerRecognizer};

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
        || interactions.contains::<SecondaryClick>(id)
        || interactions.contains::<PointerEnter>(id)
        || interactions.contains::<PointerLeave>(id)
}
