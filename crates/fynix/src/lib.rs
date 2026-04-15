#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use imaging::PaintSink;

use crate::ctx::FynixCtx;
use crate::element::{ElementId, Elements, init_elements};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};

pub use imaging;
pub use rectree;

pub mod ctx;
pub mod element;
pub mod resource;
pub mod style;
pub mod type_table;

mod id;

/// Initializes the Fynix framework.
///
/// Must be called once before any element is added to a
/// [`Fynix`] instance. Safe to call multiple times - only
/// the first call has any effect.
pub fn init() {
    init_elements();
}

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
