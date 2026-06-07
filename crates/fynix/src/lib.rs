#![doc = include_str!("../README.md")]
#![no_std]

extern crate alloc;

pub use field_path;
pub use imaging;
use imaging::PaintSink;

use crate::ctx::FynixCtx;
use crate::element::{ElementId, Elements};
use crate::events::Events;
use crate::interaction::Interactions;
use crate::resource::Resources;
use crate::scope::{ScopeElement, Scopes};
use crate::style::{StyleId, Styles};

pub mod composer;
pub mod ctx;
pub mod element;
pub mod events;
pub mod init;
pub mod interaction;
pub mod resource;
pub mod scope;
pub mod style;
pub mod typing;

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
    pub use crate::events::Events;
    pub use crate::init::Init;
    pub use crate::style::{Stylable, path};
}

mod id;

/// Root application context. Owns the element tree, layout state, and
/// style state.
///
/// Obtain a [`FynixCtx`] via [`Self::root_ctx`] to start building the
/// UI.
pub struct Fynix {
    // TODO(nixon): Make these private and provide a more elegant
    // API!
    pub elements: Elements,
    pub styles: Styles,
    pub resources: Resources,
    pub scopes: Scopes,
    pub events: Events,
    pub interactions: Interactions,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            elements: Elements::new(),
            styles: Styles::new(),
            resources: Resources::new(),
            scopes: Scopes::new(),
            events: Events::new(),
            interactions: Interactions::new(),
        }
    }

    /// Dispatches `interaction` to the handler registered for `id`'s
    /// element type and the interaction type `I`, if one exists.
    ///
    /// Returns `true` if a handler ran. Messages the handler emits
    /// land in [`Self::events`].
    #[inline]
    pub fn dispatch<I: 'static>(
        &mut self,
        id: &ElementId,
        interaction: I,
    ) -> bool {
        self.interactions.dispatch::<I>(
            id,
            interaction,
            &mut self.events,
        )
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
        // Removes the element subtree, cleaning up the styles and
        // scopes each removed element owns. The first primary style
        // encountered drops itself and all its descendants in the
        // style tree, so deeper primary styles are left for that
        // subtree removal to handle.
        let mut has_removed_styles = false;

        self.elements.remove(id, |id, meta| {
            if !has_removed_styles
                && let Some(primary_style) = meta.primary_style
            {
                has_removed_styles =
                    self.styles.remove(&primary_style);
            }

            // Drop any scope and interaction handlers this element
            // owns.
            self.scopes.remove_for_element(id);
            self.interactions.remove(id);
        })
    }

    /// Re-runs every reactive scope of world type `W` whose
    /// `changed_fn` reports a change, rebuilding its subtree in
    /// place.
    ///
    /// Intended to be called by the backend once per frame.
    pub fn update_scopes<W: 'static>(&mut self, world: &mut W) {
        for scope in self.scopes.snapshot_changed::<W>(world) {
            let element_id = scope.element_id();

            // The holder may have been discarded earlier this flush
            // by an ancestor scope's rebuild. If so, the scope was
            // dropped with it, so skip the stale snapshot entry.
            let Some(old_child) = self
                .elements
                .get_typed_mut::<ScopeElement>(&element_id)
                .map(|elem| elem.child.take())
            else {
                continue;
            };

            if let Some(old_child) = old_child {
                self.remove_element(&old_child);
            }

            // Rebuild under the scope's captured style scope, then
            // drop any uncommitted style changes so they do not leak.
            let child = {
                let ctx =
                    FynixCtx::new(self, world, scope.style_id());
                scope.build(ctx)
            };
            self.styles.clear_builder();

            if let Some(child_id) = child
                && let Some(meta) =
                    self.elements.metas.get_mut(&child_id)
            {
                meta.node.parent_id = Some(element_id);
            }
            if let Some(elem) = self
                .elements
                .get_typed_mut::<ScopeElement>(&element_id)
            {
                elem.child = child;
            }

            // Mark the rebuilt subtree dirty so it is re-laid-out
            // and re-rendered.
            self.elements.mark_dirty(element_id);
        }
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
