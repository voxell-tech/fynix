use alloc::vec::Vec;

use hashbrown::HashMap;
use rectree::{Constraint, NodeContext, Size, Vec2};
use sparse_map::{Key, SparseMap};

use crate::ctx::FynixCtx;
use crate::element::layout::ElementNodes;
use crate::element::{Element, ElementBuild, ElementId};
use crate::init::Init;
use crate::style::StyleId;

/// Store of every reactive bound to the element tree, fixed to the
/// single world type `W`.
///
/// Backed by a concrete [`SparseMap`] keyed by [`ReactiveId`].
pub struct Reactives<W> {
    reactives: SparseMap<Reactive<W>>,
    /// Reverse index from each holder element to its reactive, used
    /// to remove the reactive when the element is removed.
    element_map: HashMap<ElementId, ReactiveId>,
}

impl<W> Reactives<W> {
    pub fn new() -> Self {
        Self {
            reactives: SparseMap::new(),
            element_map: HashMap::new(),
        }
    }

    pub fn add(&mut self, reactive: Reactive<W>) -> ReactiveId {
        let element_id = reactive.element_id;
        let reactive_id = ReactiveId(self.reactives.insert(reactive));
        self.element_map.insert(element_id, reactive_id);
        reactive_id
    }

    /// Removes the reactive bound to `element_id`, if any.
    ///
    /// A no-op for elements that do not own a reactive.
    pub(crate) fn remove_for_element(
        &mut self,
        element_id: &ElementId,
    ) {
        if let Some(reactive_id) = self.element_map.remove(element_id)
        {
            self.reactives.remove(&reactive_id.0);
        }
    }

    /// Snapshots every reactive whose `changed_fn` reports a change
    /// against `world`.
    ///
    /// [`Reactive`] is [`Copy`], so the snapshot detaches the changed
    /// reactives from the store, letting the caller borrow the rest
    /// of `Fynix` while it rebuilds each one.
    pub(crate) fn snapshot_changed(
        &self,
        world: &W,
    ) -> Vec<Reactive<W>> {
        self.reactives
            .iter()
            .filter(|reactive| reactive.is_changed(world))
            .copied()
            .collect()
    }
}

impl<W> Default for Reactives<W> {
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

pub type BuildFn<W> = fn(&mut FynixCtx<W>) -> Option<ElementId>;

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

    pub fn build(&self, ctx: &mut FynixCtx<W>) -> Option<ElementId> {
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
pub struct ReactiveId(Key);

impl ReactiveId {
    /// A sentinel id that will never refer to a live reactive.
    pub const PLACEHOLDER: Self = Self(Key::PLACEHOLDER);
}

impl core::ops::Deref for ReactiveId {
    type Target = Key;
    fn deref(&self) -> &Key {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Fynix;
    use crate::element::ElementBuild;
    use crate::element::layout::ElementNodes;

    #[derive(Init, Element, Clone)]
    struct Counter {
        n: u32,
    }

    impl ElementBuild for Counter {
        fn build(
            &self,
            _id: &ElementId,
            constraint: Constraint,
            _nodes: &mut ElementNodes,
        ) -> Size {
            constraint.min
        }
    }

    #[derive(Default)]
    struct World {
        changed: bool,
        value: u32,
    }

    /// The reactive captures `value` at build time and only rebuilds
    /// when `changed` reports true, picking up the new value then.
    #[test]
    fn rebuilds_only_when_changed() {
        let mut world = World {
            changed: false,
            value: 1,
        };
        let mut fynix = Fynix::<World>::new();

        let holder = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.reactive(
                |w| w.changed,
                |ctx| {
                    let value = ctx.world.value;
                    Some(
                        ctx.add_with::<Counter>(|c, _| c.n = value)
                            .id(),
                    )
                },
            )
            .id()
        };

        let child_n = |fynix: &Fynix<World>| {
            let child = fynix
                .elements
                .get_typed::<ReactiveElement>(&holder)
                .unwrap()
                .child
                .unwrap();
            (
                child,
                fynix
                    .elements
                    .get_typed::<Counter>(&child)
                    .unwrap()
                    .n,
            )
        };

        // Initial build captured value 1.
        let (first_child, n) = child_n(&fynix);
        assert_eq!(n, 1);

        // Value changes but `changed` is false: no rebuild.
        world.value = 2;
        fynix.update_reactives(&mut world);
        let (child, n) = child_n(&fynix);
        assert_eq!(child, first_child);
        assert_eq!(n, 1);

        // Flip `changed`: subtree rebuilds with the new value.
        world.changed = true;
        fynix.update_reactives(&mut world);
        let (_, n) = child_n(&fynix);
        assert_eq!(n, 2);
    }
}
