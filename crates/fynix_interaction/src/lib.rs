//! Interaction layer for fynix.
//!
//! Turns a backend's raw input into the semantic interactions element
//! handlers consume.

#![no_std]

pub mod pointer;

pub mod prelude {
    pub use crate::pointer::prelude::*;
}
