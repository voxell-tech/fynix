#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub use field_path;
pub use imaging;
use imaging::PaintSink;
pub use rectree::RectNodes;

use crate::ctx::FynixCtx;
use crate::element::observer::ObserverFn;
use crate::element::{ElementId, Elements};
use crate::interaction::{Handler, Response};
use crate::reactive::binding::Binding;
use crate::reactive::watcher::{Watcher, WatcherElement};
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
    pub use crate::interaction::{Propagation, Response};
    pub use crate::style::{Stylable, path};
}

mod id;

/// Root application context. Owns the element tree, layout state, and
/// style state.
///
/// Obtain a [`FynixCtx`] via [`Self::root_ctx`] to start building the
/// user interface.
pub struct Fynix<W: 'static> {
    pub resources: Resources,
    elements: Elements<W>,
    styles: Styles,
}

impl<W> Fynix<W> {
    pub fn new() -> Self {
        Self {
            resources: Resources::new(),
            elements: Elements::new(),
            styles: Styles::new(),
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
    ) -> bool {
        let Some(handler) =
            self.elements.table.get_component::<Handler<I, W>>(id)
        else {
            return false;
        };
        handler.call(interaction, &mut Response::new(world));
        true
    }

    /// Dispatches `interaction` up the parent chain from `start`
    /// (inclusive): each ancestor that handles `I` runs, and bubbling
    /// continues only while handlers let it (see
    /// [`Response::propagate`]), stopping at the first that consumes.
    ///
    /// `should_bubble` gates each candidate: the walk stops as soon
    /// as it returns `false`. Pass `|_| true` to always bubble to
    /// the root, or a hit-test gate to stop at the pointer's edge.
    ///
    /// Returns `true` if a handler consumed the interaction. `I` must
    /// be [`Copy`] since it may be delivered to several handlers.
    pub fn dispatch_bubbling<I: 'static + Copy>(
        &mut self,
        start: &ElementId,
        interaction: I,
        world: &mut W,
        should_bubble: impl Fn(&ElementId) -> bool,
    ) -> bool {
        let mut current = Some(*start);
        while let Some(id) = current {
            if !should_bubble(&id) {
                break;
            }
            if let Some(handler) = self
                .elements
                .table
                .get_component::<Handler<I, W>>(&id)
            {
                let mut response = Response::new(&mut *world);
                handler.call(interaction, &mut response);
                if response.consumed() {
                    return true;
                }
            }
            current = self
                .elements
                .table
                .node(&id)
                .and_then(|node| node.parent_id);
        }

        false
    }

    /// The element store, for read-only access such as building a
    /// spatial index over the laid-out tree.
    #[inline]
    pub fn elements(&self) -> &Elements<W> {
        &self.elements
    }

    /// Registers `observer` to run after a component of type `T` is
    /// inserted on any element.
    #[inline]
    pub fn on_insert<T: 'static>(
        &mut self,
        observer: ObserverFn<W>,
    ) -> &mut Self {
        self.elements.table.on_insert::<T>(observer);
        self
    }

    /// Registers `observer` to run after a component of type `T` is
    /// removed on any element.
    #[inline]
    pub fn on_remove<T: 'static>(
        &mut self,
        observer: ObserverFn<W>,
    ) -> &mut Self {
        self.elements.table.on_remove::<T>(observer);
        self
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
        // handle. Watchers, bindings, and interaction handlers ride
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
    pub fn sync(&mut self, world: &mut W) {
        // Watchers and bindings are not `Copy`, so take each changed
        // one out of the table, run it (which is then free to mutate
        // the table), then re-attach it.
        let watcher_ids = self
            .elements
            .table
            .components::<Watcher<W>>()
            .filter(|(_, watcher)| watcher.is_changed(world))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        for id in watcher_ids {
            let Some(watcher) = self
                .elements
                .table
                .remove_component::<Watcher<W>>(&id)
            else {
                continue;
            };
            watcher.rebuild(&id, self, world);
            // An ancestor rebuild may have discarded the holder; only
            // re-attach if it survived.
            if self
                .elements
                .get_typed::<WatcherElement>(&id)
                .is_some()
            {
                self.elements.table.insert_watcher(id, watcher);
            }
        }

        let binding_ids = self
            .elements
            .table
            .components::<Binding<W>>()
            .filter(|(_, binding)| binding.is_changed(world))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();

        for id in binding_ids {
            let Some(binding) = self
                .elements
                .table
                .remove_component::<Binding<W>>(&id)
            else {
                continue;
            };
            binding.apply(&id, &mut self.elements, world);
            self.elements.table.insert_binding(id, binding);
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
