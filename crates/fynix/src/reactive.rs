use alloc::vec::Vec;

use hashbrown::HashMap;
use rectree::{Constraint, NodeContext, Size, Vec2};

use crate::ctx::FynixCtx;
use crate::element::layout::ElementNodes;
use crate::element::{Element, ElementBuild, ElementId};
use crate::init::Init;
use crate::style::StyleId;
use crate::typing::type_pool::{ColumnKey, TypePool};

pub struct Reactives {
    reactives: TypePool,
    /// Reverse index from each holder element to its reactive, used
    /// to remove the reactive when the element is removed.
    by_element: HashMap<ElementId, ReactiveId>,
}

impl Reactives {
    pub fn new() -> Self {
        Self {
            reactives: TypePool::new(),
            by_element: HashMap::new(),
        }
    }

    pub fn add<W: 'static>(
        &mut self,
        reactive: Reactive<W>,
    ) -> ReactiveId {
        let element_id = reactive.element_id;
        let reactive_id = ReactiveId(self.reactives.insert(reactive));
        self.by_element.insert(element_id, reactive_id);
        reactive_id
    }

    /// Removes the reactive bound to `element_id`, if any.
    ///
    /// A no-op for elements that do not own a reactive.
    pub(crate) fn remove_for_element(
        &mut self,
        element_id: &ElementId,
    ) {
        if let Some(reactive_id) = self.by_element.remove(element_id)
        {
            self.reactives.dyn_remove(&reactive_id.0);
        }
    }

    /// Snapshots every reactive of world type `W` whose `changed_fn`
    /// reports a change against `world`.
    ///
    /// [`Reactive`] is [`Copy`], so the snapshot detaches the changed
    /// reactives from the pool, letting the caller borrow the rest of
    /// `Fynix` while it rebuilds each one.
    pub(crate) fn snapshot_changed<W: 'static>(
        &self,
        world: &W,
    ) -> Vec<Reactive<W>> {
        self.reactives
            .iter::<Reactive<W>>()
            .filter(|reactive| reactive.is_changed(world))
            .copied()
            .collect()
    }
}

impl Default for Reactives {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Init, Element)]
pub struct ReactiveElement {
    #[elem(children)]
    pub(crate) child: Option<ElementId>,
}

impl ElementBuild for ReactiveElement {
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
pub struct Reactive<W> {
    /// Function that determines if something has changed and require
    /// a re-creation of [`ReactiveElement::child`].
    changed_fn: ChangedFn<W>,
    /// Builds the element for [`ReactiveElement::child`].
    build_fn: BuildFn<W>,
    /// The element id that holds the [`ReactiveElement`].
    element_id: ElementId,
    /// Style scope active when the reactive was created. Restored on
    /// each rebuild so the subtree is styled like its first build.
    style_id: Option<StyleId>,
}

impl<W> Reactive<W> {
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

impl<W> Copy for Reactive<W> {}

impl<W> Clone for Reactive<W> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Generational ID for reactive instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReactiveId(ColumnKey);

impl ReactiveId {
    /// A sentinel id that will never refer to a live reactive.
    pub const PLACEHOLDER: Self = Self(ColumnKey::PLACEHOLDER);
}

impl core::ops::Deref for ReactiveId {
    type Target = ColumnKey;
    fn deref(&self) -> &ColumnKey {
        &self.0
    }
}
