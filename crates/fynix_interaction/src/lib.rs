//! Interaction layer for fynix.
//!
//! Turns a backend's raw input into the semantic interactions element
//! handlers consume. The [`pointer`](mod@crate::pointer) module
//! covers pointer input (clicks, drags, hover); the [`key`] module
//! covers keyboard input. `no_std`.

#![no_std]

pub mod pointer;

pub mod prelude {
    pub use crate::pointer::prelude::*;
}
