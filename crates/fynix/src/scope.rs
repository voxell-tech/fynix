use alloc::vec::Vec;

use hashbrown::HashMap;
use rectree::{Constraint, NodeContext, Size, Vec2};

use crate::ctx::FynixCtx;
use crate::element::layout::ElementNodes;
use crate::element::{Element, ElementBuild, ElementId};
use crate::init::Init;
use crate::style::StyleId;
use crate::type_pool::{ColumnKey, TypePool};

pub struct Scopes {
    scopes: TypePool,
    /// Reverse index from each holder element to its scope, used to
    /// remove the scope when the element is removed.
    by_element: HashMap<ElementId, ScopeId>,
}

impl Scopes {
    pub fn new() -> Self {
        Self {
            scopes: TypePool::new(),
            by_element: HashMap::new(),
        }
    }

    pub fn add<W: 'static>(&mut self, scope: Scope<W>) -> ScopeId {
        let element_id = scope.element_id;
        let scope_id = ScopeId(self.scopes.insert(scope));
        self.by_element.insert(element_id, scope_id);
        scope_id
    }

    /// Removes the scope bound to `element_id`, if any.
    ///
    /// A no-op for elements that do not own a scope.
    pub(crate) fn remove_for_element(
        &mut self,
        element_id: &ElementId,
    ) {
        if let Some(scope_id) = self.by_element.remove(element_id) {
            self.scopes.dyn_remove(&scope_id.0);
        }
    }

    /// Snapshots every scope of world type `W` whose `changed_fn`
    /// reports a change against `world`.
    ///
    /// [`Scope`] is [`Copy`], so the snapshot detaches the changed
    /// scopes from the pool, letting the caller borrow the rest of
    /// `Fynix` while it rebuilds each one.
    pub(crate) fn snapshot_changed<W: 'static>(
        &self,
        world: &W,
    ) -> Vec<Scope<W>> {
        self.by_element
            .values()
            .filter_map(|id| self.scopes.get::<Scope<W>>(id).copied())
            .filter(|scope| scope.is_changed(world))
            .collect()
    }
}

impl Default for Scopes {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Init, Element)]
pub struct ScopeElement {
    #[elem(children)]
    pub(crate) child: Option<ElementId>,
}

impl ElementBuild for ScopeElement {
    fn build(
        &self,
        _id: &ElementId,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size {
        // The holder is a pass-through: it takes the size of its
        // child and positions it at the origin.
        let Some(child) = &self.child else {
            return constraint.min;
        };

        let size = nodes.get_size(child);
        nodes.set_translation(child, Vec2::ZERO);
        constraint.constrain(size)
    }
}

pub type ChangedFn<W> = fn(&W) -> bool;

pub type BuildFn<W> = fn(FynixCtx<W>) -> Option<ElementId>;

#[derive(Debug)]
pub struct Scope<W> {
    /// Function that determines if something has changed and require
    /// a re-creation of [`ScopeElement::child`].
    changed_fn: ChangedFn<W>,
    /// Builds the element for [`ScopeElement::child`].
    build_fn: BuildFn<W>,
    /// The element id that holds the [`ScopeElement`].
    element_id: ElementId,
    /// Style scope active when the scope was created. Restored on
    /// each rebuild so the subtree is styled like its first build.
    style_id: Option<StyleId>,
}

impl<W> Scope<W> {
    pub(crate) fn new(
        changed_fn: ChangedFn<W>,
        build_fn: BuildFn<W>,
        element_id: ElementId,
        style_id: Option<StyleId>,
    ) -> Self {
        Self {
            changed_fn,
            build_fn,
            element_id,
            style_id,
        }
    }

    pub(crate) fn element_id(&self) -> ElementId {
        self.element_id
    }

    pub(crate) fn style_id(&self) -> Option<StyleId> {
        self.style_id
    }

    pub fn is_changed(&self, world: &W) -> bool {
        (self.changed_fn)(world)
    }

    pub fn build(&self, ctx: FynixCtx<W>) -> Option<ElementId> {
        (self.build_fn)(ctx)
    }
}

impl<W> Copy for Scope<W> {}

impl<W> Clone for Scope<W> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Generational ID for scope instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(ColumnKey);

impl ScopeId {
    /// A sentinel id that will never refer to a live scope.
    pub const PLACEHOLDER: Self = Self(ColumnKey::PLACEHOLDER);
}

impl core::ops::Deref for ScopeId {
    type Target = ColumnKey;
    fn deref(&self) -> &ColumnKey {
        &self.0
    }
}
