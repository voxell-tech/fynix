#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

// use rectree::Rectree;

use crate::ctx::FynixCtx;
use crate::element::Elements;
use crate::style::{StyleId, Styles};

pub mod any_wrapper;
pub mod ctx;
pub mod element;
pub mod layout;
pub mod style;
pub mod type_table;

mod id;

/// Root application context. Owns the layout tree, element storage, and style state.
///
/// Obtain a [`BuildCtx`] via [`root_ctx`](Fynix::root_ctx) to start building the UI.
pub struct Fynix {
    // pub tree: Rectree,
    pub elements: Elements,
    pub styles: Styles,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            // tree: Rectree::new(),
            elements: Elements::new(),
            styles: Styles::new(),
        }
    }

    /// Returns a [`BuildCtx`] rooted at the top of the style hierarchy.
    #[inline]
    pub fn root_ctx<'a, W>(
        &'a mut self,
        world: &'a mut W,
    ) -> FynixCtx<'a, W> {
        self.create_ctx(world, None)
    }

    /// Returns a [`BuildCtx`] starting at the given style scope.
    ///
    /// Use [`root_ctx`](Fynix::root_ctx) unless you need to resume building
    /// from a previously committed [`StyleId`].
    #[inline]
    pub fn create_ctx<'a, W>(
        &'a mut self,
        world: &'a mut W,
        parent_style_id: Option<StyleId>,
    ) -> FynixCtx<'a, W> {
        FynixCtx::new(parent_style_id, self, world)
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}

// impl LayoutWorld for Fynix {
//     fn constraint(
//         &self,
//         id: &NodeId,
//         parent: rectree::layout::Constraint,
//     ) -> rectree::layout::Constraint {
//         todo!()
//     }

//     fn build(
//         &self,
//         id: &NodeId,
//         node: &rectree::RectNode,
//         tree: &Rectree,
//         pos: &mut rectree::layout::Positioner,
//     ) -> rectree::kurbo::Size {
//         todo!()
//     }
// }
