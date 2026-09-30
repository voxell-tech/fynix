//! A backend-agnostic reactive view tree.
//!
//! Views are structs that own their own props and hold other views
//! whole. Set rules restyle every view of a kind within a scope, a
//! call-site value beats any rule, and whatever is left unset falls
//! back to the theme. State rules hold while a node is in a state.
//! Elements whose props can change stay mounted, and travel to new
//! values over a transition when a rule asks them to.
//!
//! Nothing here names an engine. A [`Backend`] says what a world and a
//! node are, and a backend crate writes the elements.

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
pub mod structure;
pub mod transition;
pub mod view;
pub mod visual;

#[cfg(test)]
mod tests;

pub use backend::Backend;
pub use cx::{Cx, Trace};
pub use lenz;
pub use mounted::{Mounted, Tick};
pub use prop::{Derived, Prop, Signal, derived};
pub use rules::{Condition, RuleArena};
pub use scoped::{Rules, ScopedExt, Transition, When};
pub use structure::{Each, Keyed, each, keyed};
pub use transition::{Curve, Motion, MotionTokens, Tween};
pub use view::{
    AnyView, Element, Layered, Settable, Styled, View, ViewExt,
    ViewSeq,
};
pub use visual::{Visual, VisualMut};
