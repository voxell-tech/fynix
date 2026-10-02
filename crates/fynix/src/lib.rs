#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod backend;
pub mod cx;
mod layer;
pub mod mounted;
pub mod prop;
pub mod rules;
pub mod scoped;
pub mod slot;
pub mod structure;
pub mod transition;
pub mod view;
pub mod visual;

#[cfg(test)]
mod tests;

pub use backend::Backend;
pub use cx::{Cx, Trace};
pub use fynix_macros::element;
pub use lenz;
pub use mounted::{Mounted, Tick};
pub use prop::{Derived, Prop, Signal, derived};
pub use rules::{Condition, RuleArena};
pub use scoped::{Rules, ScopedExt, Transition, When};
pub use slot::{Patch, Slot};
pub use structure::{Each, Keyed, each, keyed};
pub use transition::{Curve, Motion, MotionTokens, Tween};
pub use view::{
    AnyView, Element, Layered, Settable, Styled, View, ViewExt,
    ViewSeq,
};
pub use visual::{Visual, VisualMut};
