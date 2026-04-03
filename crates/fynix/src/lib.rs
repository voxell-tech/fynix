#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

pub use fynix_macros::fynix;
pub use rectree;

use crate::ctx::FynixCtx;
use crate::element::{ElementId, Elements};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};

pub use crate::element::composer::ELEMENT_COMPOSERS;
pub use crate::element::composer::UntypedElementComposer;

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
    elements: Elements,
    styles: Styles,
    resources: Resources,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            elements: Elements::new(),
            styles: Styles::new(),
            resources: Resources::new(),
        }
    }

    pub fn elements(&self) -> &Elements {
        &self.elements
    }

    pub fn styles(&self) -> &Styles {
        &self.styles
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    /// Runs a full layout cycle on the subtree rooted at `id`.
    #[inline]
    pub fn layout(&mut self, id: &ElementId) {
        self.elements.layout(id, &mut self.resources);
    }

    /// Returns a [`FynixCtx`] rooted at the top of the style
    /// hierarchy.
    #[inline]
    pub fn root_ctx<'f, 'w, W>(
        &'f mut self,
        world: &'w mut W,
    ) -> FynixCtx<'f, 'w, W> {
        self.create_ctx(world, None)
    }

    /// Returns a [`FynixCtx`] starting at the given style scope.
    ///
    /// Use [`Self::root_ctx`] unless you need to resume building from
    /// a previously committed [`StyleId`].
    #[inline]
    pub fn create_ctx<'f, 'w, W>(
        &'f mut self,
        world: &'w mut W,
        parent_style_id: Option<StyleId>,
    ) -> FynixCtx<'f, 'w, W> {
        FynixCtx::new(parent_style_id, self, world)
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}
