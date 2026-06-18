//! Pointer interaction: turns a backend's pointer input into the
//! semantic interactions element handlers consume (clicks, drags,
//! hover).
//!
//! The backend feeds input to an [`Interactor`] through its
//! `down`/`up`/`moved`/`cancel` methods; the interactor resolves the
//! element under the pointer via `fynix_hit_test`, runs a per-pointer
//! [`PointerRecognizer`], and dispatches the resulting interactions
//! through `fynix`.
//!
//! [`Interactor`]: interactor::Interactor
//! [`PointerRecognizer`]: recognizer::PointerRecognizer

use fynix::Fynix;
use fynix::element::table::ElementTable;
use fynix::interaction::Handler;
use fynix::prelude::ElementId;

pub mod identity;
pub mod interaction;
pub mod interactor;
pub mod recognizer;

pub mod prelude {
    pub use super::identity::{
        ButtonRole, ClassifyButton, PointerId,
    };
    pub use super::interaction::{
        Drag, DragEnd, DragStart, Hover, PointerEnter, PointerLeave,
        PrimaryClick, RawClick, SecondaryClick,
    };
    pub use super::interactor::Interactor;
    pub use super::recognizer::{Config, PointerRecognizer};
}

/// Marks an element that carries one of the crate's pointer handlers,
/// inserted by the observers [`init`] registers. The hit-test indexes
/// only elements carrying it, so most of the scene stays out of the
/// spatree.
pub struct HitTarget;

/// Registers, on `fynix`, an insert observer for each pointer handler
/// type so that attaching any of them marks the element with
/// [`HitTarget`]. Call once, before building the tree, for each
/// `(Device, Pointer)` the backend uses.
pub fn init<D: 'static, P: 'static, W: 'static>(
    fynix: &mut Fynix<W>,
) {
    use identity::PointerId;
    use interaction::*;

    /// The observer that marks an element as a hit target when one of
    /// the crate's pointer handlers is attached.
    fn mark<W: 'static>(table: &mut ElementTable<W>, id: ElementId) {
        table.insert_component(id, HitTarget);
    }

    type Id<Device, Pointer> = PointerId<Device, Pointer>;
    fynix
        .on_insert::<Handler<RawClick<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<PrimaryClick<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<SecondaryClick<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<PointerEnter<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<PointerLeave<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<Hover<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<DragStart<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<Drag<Id<D, P>>, W>>(mark)
        .on_insert::<Handler<DragEnd<Id<D, P>>, W>>(mark);
}
