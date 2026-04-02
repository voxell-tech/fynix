#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use crate::ctx::FynixCtx;
use crate::element::{ElementId, Elements};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};

pub use rectree;

pub mod any_wrapper;
pub mod ctx;
pub mod element;
pub mod resource;
pub mod style;
pub mod type_table;

mod id;

/// Root application context. Owns the element tree, layout state,
/// and style state.
///
/// Obtain a [`FynixCtx`] via [`Self::root_ctx`] to start building
/// the UI.
pub struct Fynix {
    pub elements: Elements,
    pub styles: Styles,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            elements: Elements::new(),
            styles: Styles::new(),
        }
    }

    /// Runs a full layout cycle on the subtree rooted at `id`.
    pub fn layout(&mut self, id: &ElementId) {
        self.elements.layout(id);
    }

    /// Returns a [`FynixCtx`] rooted at the top of the style
    /// hierarchy.
    #[inline]
    pub fn root_ctx<'a, W>(
        &'a mut self,
        world: &'a mut W,
    ) -> FynixCtx<'a, W> {
        self.create_ctx(world, None)
    }

    /// Returns a [`FynixCtx`] starting at the given style scope.
    ///
    /// Use [`Self::root_ctx`] unless you need to resume building from
    /// a previously committed [`StyleId`].
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
