//! Reactivity: world-change-driven updates to the element tree.
//!
//! Two flavors share one change-detection signal, the [`ChangedFn`]:
//!
//! - A [`watch`] rebuilds a whole subtree when its source changes.
//! - A [`binding`] writes one field in place when its source changes.
//!
//! Both are stored as per-element components on the element table and
//! flushed together each frame by
//! [`Fynix::sync`](crate::Fynix::sync).

pub mod binding;
pub mod watch;

/// Reports whether a reactive source changed since the last flush.
pub type ChangedFn<W> = fn(&W) -> bool;
