#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

pub use field_path;
pub use imaging;
use imaging::PaintSink;
pub use rectree;

use crate::ctx::FynixCtx;
use crate::element::{ElementId, Elements};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};

pub mod composer;
pub mod ctx;
pub mod element;
pub mod init;
pub mod resource;
pub mod style;
pub mod type_table;

pub mod prelude {
    pub use crate::Fynix;
    pub use crate::composer::Composer;
    pub use crate::ctx::FynixCtx;
    pub use crate::element::{
        Element, ElementBuild, ElementChildren, ElementId,
    };
    pub use crate::init::Init;
    pub use crate::style::{Stylable, path};
}

mod id;

/// Root application context. Owns the element tree, layout state,
/// and style state.
///
/// Obtain a [`FynixCtx`] via [`Self::root_ctx`] to start building
/// the UI.
pub struct Fynix {
    // TODO(nixon): Make these private and provide a more elegant API!
    pub elements: Elements,
    pub styles: Styles,
    pub resources: Resources,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            elements: Elements::new(),
            styles: Styles::new(),
            resources: Resources::new(),
        }
    }

    /// Runs a full layout cycle on the subtree rooted at `id`.
    #[inline]
    pub fn layout(&mut self, id: &ElementId) {
        self.elements.layout(id, &mut self.resources);
    }

    /// Renders the subtree rooted at `id` into `sink`.
    ///
    /// Layout must be complete before calling this.
    #[inline]
    pub fn render(&self, id: &ElementId, sink: &mut impl PaintSink) {
        self.elements.render(id, sink);
    }

    /// Removes an element and its associated primary style tree.
    ///
    /// If the element has a `primary_style`, that style and all
    /// its descendants in the style tree are also removed
    ///
    /// Returns `true` if the element existed
    #[inline]
    pub fn remove_element(&mut self, id: &ElementId) -> bool {
        // Removes the element subtree along with their styles.
        if !self.elements.remove(id, &mut self.styles) {
            return false;
        }
        true
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
        parent_style: Option<StyleId>,
    ) -> FynixCtx<'f, 'w, W> {
        FynixCtx::new(self, world, parent_style)
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}
