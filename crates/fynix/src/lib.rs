#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use field_path;
pub use imaging;
use imaging::PaintSink;

use crate::ctx::FynixCtx;
use crate::element::{ElementId, Elements};
use crate::interaction::HandlerFn;
use crate::reactive::binding::Binding;
use crate::reactive::watch::Watch;
use crate::resource::Resources;
use crate::style::{StyleId, Styles};

pub mod composer;
pub mod ctx;
pub mod element;
pub mod init;
pub mod interaction;
pub mod reactive;
pub mod resource;
pub mod style;

pub mod prelude {
    // Intentionally exposing only `NodeContext` and hiding
    // `RectNodes`, `Rectree`, `NodeState`, etc.
    pub use rectree::NodeContext;
    pub use rectree::geom::*;

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

/// Root application context. Owns the element tree, layout state, and
/// style state.
///
/// Obtain a [`FynixCtx`] via [`Self::root_ctx`] to start building the
/// user interface.
pub struct Fynix<W> {
    pub resources: Resources,
    elements: Elements,
    styles: Styles,
    /// Single backend world type. Watches, bindings, and interaction
    /// handlers are stored as components on the element table, so
    /// `W` only appears in the build-time and update APIs.
    _world: core::marker::PhantomData<fn(&mut W)>,
}

impl<W> Fynix<W> {
    pub fn new() -> Self {
        Self {
            resources: Resources::new(),
            elements: Elements::new(),
            styles: Styles::new(),
            _world: core::marker::PhantomData,
        }
    }

    /// Dispatches `interaction` to the handler attached to `id` for
    /// the interaction type `I`, if one exists, letting it mutate
    /// `world`.
    ///
    /// Returns `true` if a handler ran.
    #[inline]
    pub fn dispatch<I: 'static>(
        &mut self,
        id: &ElementId,
        interaction: I,
        world: &mut W,
    ) -> bool
    where
        W: 'static,
    {
        let Some(handler) = self
            .elements
            .table
            .get_component::<HandlerFn<I, W>>(id)
            .copied()
        else {
            return false;
        };
        handler(interaction, world);
        true
    }

    /// Lays out every dirty subtree (see [`Elements::mark_dirty`]),
    /// draining the dirty set.
    #[inline]
    pub fn layout(&mut self) {
        self.elements.layout(&mut self.resources);
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
    /// If the element has a `primary_style`, that style and all its
    /// descendants in the style tree are also removed
    ///
    /// Returns `true` if the element existed
    #[inline]
    pub fn remove_element(&mut self, id: &ElementId) -> bool {
        // Removes the element subtree, cleaning up the styles each
        // removed element owns. The first primary style encountered
        // drops itself and all its descendants in the style tree, so
        // deeper primary styles are left for that subtree removal to
        // handle. Watches, bindings, and interaction handlers ride
        // the element table and are dropped with the element,
        // so only styles need explicit cleanup here.
        let mut has_removed_styles = false;

        self.elements.remove(id, |_id, primary_style| {
            if !has_removed_styles
                && let Some(primary_style) = primary_style
            {
                has_removed_styles =
                    self.styles.remove(&primary_style);
            }
        })
    }

    /// Reconciles the element tree with the current world state,
    /// applying every change-driven update whose source changed.
    ///
    /// Intended to be called by the backend once per frame.
    pub fn sync(&mut self, world: &mut W)
    where
        W: 'static,
    {
        let watches = self
            .elements
            .table
            .components::<Watch<W>>()
            .filter(|(_, watch)| watch.is_changed(world))
            .map(|(id, watch)| (*id, *watch))
            .collect::<Vec<_>>();

        for (id, watch) in watches {
            watch.rebuild(id, self, world);
        }

        let bindings = self
            .elements
            .table
            .components::<Binding<W>>()
            .filter(|(_, binding)| binding.is_changed(world))
            .map(|(id, binding)| (*id, *binding))
            .collect::<Vec<_>>();

        for (id, binding) in bindings {
            binding.build(&id, &mut self.elements, world);
        }
    }

    /// Returns a [`FynixCtx`] rooted at the top of the style
    /// hierarchy.
    #[inline]
    pub fn root_ctx<'f, 'w>(
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
    pub fn create_ctx<'f, 'w>(
        &'f mut self,
        world: &'w mut W,
        parent_style: Option<StyleId>,
    ) -> FynixCtx<'f, 'w, W> {
        FynixCtx::new(self, world, parent_style)
    }
}

impl<W> Default for Fynix<W> {
    fn default() -> Self {
        Self::new()
    }
}
