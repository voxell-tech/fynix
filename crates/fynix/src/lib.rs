#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};

use crate::ctx::BuildCtx;
use crate::element::Elements;
use crate::style::{StyleId, Styles};

pub mod any_wrapper;
pub mod ctx;
pub mod element;
pub mod style;
pub mod type_table;

mod id;

/// Root application context. Owns the layout tree, element storage, and style state.
///
/// Obtain a [`BuildCtx`] via [`root_ctx`](Fynix::root_ctx) to start building the UI.
pub struct Fynix {
    pub tree: Rectree,
    pub elements: Elements,
    pub styles: Styles,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            tree: Rectree::new(),
            elements: Elements::new(),
            styles: Styles::new(),
        }
    }

    /// Returns a [`BuildCtx`] rooted at the top of the style hierarchy.
    #[inline]
    pub fn root_ctx(&mut self) -> BuildCtx<'_> {
        self.create_ctx(None)
    }

    /// Returns a [`BuildCtx`] starting at the given style scope.
    ///
    /// Use [`root_ctx`](Fynix::root_ctx) unless you need to resume building
    /// from a previously committed [`StyleId`].
    #[inline]
    pub fn create_ctx(
        &mut self,
        parent_style_id: Option<StyleId>,
    ) -> BuildCtx<'_> {
        BuildCtx::new(
            parent_style_id,
            &mut self.elements,
            &mut self.styles,
        )
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutWorld for Fynix {
    fn get_solver(&self, _id: &NodeId) -> &dyn LayoutSolver {
        todo!()
    }
}

/// Generic context pairing [`Fynix`] with an external world `W`.
///
/// Intended for integrations (e.g. a Bevy world) that need access to both
/// the fynix state and their own data simultaneously.
pub struct FynixCtx<'a, W> {
    pub fynix: &'a mut Fynix,
    pub world: &'a mut W,
}
