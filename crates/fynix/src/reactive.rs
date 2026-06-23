//! Reactivity: world-change-driven updates to the element tree.
//!
//! Two flavors share one change-detection signal, the [`ChangedFn`]:
//!
//! - A [`watcher`] rebuilds a whole subtree when its source changes.
//! - A [`binding`] writes one field in place when its source changes.
//!
//! Both are stored as per-element components on the element table and
//! flushed together each frame by
//! [`Fynix::sync`](crate::Fynix::sync).

use alloc::boxed::Box;

pub mod binding;
pub mod watcher;

/// A boxed change-detection predicate: the shared signal both
/// bindings and watchers flush on. May capture the state it compares
/// against.
pub struct Changed<W>(Box<dyn ChangedFn<W>>);

impl<W> Changed<W> {
    /// Boxes `changed` for storage.
    pub fn new(changed: impl ChangedFn<W>) -> Self {
        Self(Box::new(changed))
    }

    /// Returns `true` if the source changed since the last flush.
    pub fn is_changed(&self, world: &W) -> bool {
        (self.0)(world)
    }
}

/// Shorthand for the closure a [`Changed`] accepts: any
/// `Fn(&W) -> bool + 'static`.
///
/// A blanket impl covers every matching closure, so this reads as a
/// trait alias at `impl` sites (e.g.
/// [`bind`](crate::ctx::ElementCtx::bind)).
pub trait ChangedFn<W>: Fn(&W) -> bool + 'static {}

impl<W, F> ChangedFn<W> for F where F: Fn(&W) -> bool + 'static {}
