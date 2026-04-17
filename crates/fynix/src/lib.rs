#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicU8, Ordering};

pub use field_path;
pub use imaging;
use imaging::PaintSink;
pub use rectree;
pub use typeslot;
use typeslot::{SlotGroup, TypeSlot};

use crate::ctx::FynixCtx;
use crate::element::{ElementGroup, ElementId, Elements};
use crate::resource::Resources;
use crate::signal::Signals;
use crate::style::{StyleId, Styles};

pub mod composer;
pub mod ctx;
pub mod element;
pub mod init;
pub mod resource;
pub mod signal;
pub mod style;
pub mod type_table;

pub mod prelude {
    pub use crate::Fynix;
    pub use crate::composer::Composer;
    pub use crate::ctx::FynixCtx;
    pub use crate::element::{
        Element, ElementBuild, ElementChildren, ElementId,
        ElementTemplate,
    };
    pub use crate::init::Init;
    pub use crate::style::{Stylable, path};
}

mod id;

#[derive(SlotGroup)]
pub struct WorldGroup;

pub trait World: TypeSlot<WorldGroup> {}

impl<T: TypeSlot<WorldGroup>> World for T {}

/// Initializes the Fynix framework.
///
/// Must be called before any element is added to a [`Fynix`]
/// instance. Safe to call more than once - subsequent calls
/// block until initialization is complete.
fn init() {
    // 0 = uninit, 1 = initializing, 2 = done.
    static STATE: AtomicU8 = AtomicU8::new(0);

    match STATE.compare_exchange(
        0,
        1,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => {
            ElementGroup::init();
            STATE.store(2, Ordering::Release);
        }
        Err(_) => {
            while STATE.load(Ordering::Acquire) != 2 {
                core::hint::spin_loop();
            }
        }
    }
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
    pub signals: Signals,
}

impl Fynix {
    pub fn new() -> Self {
        init();
        Self {
            elements: Elements::new(),
            styles: Styles::new(),
            resources: Resources::new(),
            signals: Signals::new(),
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
