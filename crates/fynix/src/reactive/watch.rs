//! Watches: subtree rebuilds driven by world change detection.
//!
//! A watch is the heavyweight counterpart to a
//! [`Binding`](crate::reactive::binding::Binding). Where a binding
//! writes one field in place, a watch rebuilds a whole subtree when
//! the world state it reads changes. Watches are attached via
//! [`FynixCtx::watch`](crate::ctx::FynixCtx::watch) and flushed each
//! frame by [`Fynix::update_watches`](crate::Fynix::update_watches).

use rectree::{Constraint, NodeContext, Size, Vec2};

use crate::Fynix;
use crate::ctx::FynixCtx;
use crate::element::layout::ElementNodes;
use crate::element::{Element, ElementBuild, ElementId};
use crate::init::Init;
use crate::reactive::ChangedFn;
use crate::style::StyleId;

#[derive(Init, Element)]
pub struct WatchElement {
    #[elem(children)]
    pub(crate) child: Option<ElementId>,
}

impl ElementBuild for WatchElement {
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

pub type BuildFn<W> = fn(&mut FynixCtx<W>) -> Option<ElementId>;

/// A watch, stored as a component on its [`WatchElement`] holder (see
/// [`ElementTable::insert_component`]). The holder's [`ElementId`] is
/// the component key, so the watch carries no id of its own.
///
/// [`ElementTable::insert_component`]: crate::element::table::ElementTable::insert_component
#[derive(Debug)]
pub struct Watch<W> {
    /// Reports whether the watched source changed since the last
    /// flush, requiring a rebuild of [`WatchElement::child`].
    changed_fn: ChangedFn<W>,
    /// Builds the replacement for [`WatchElement::child`].
    build_fn: BuildFn<W>,
    /// Style scope active when the watch was created. Restored on
    /// each rebuild so the subtree is styled like its first build.
    style_id: Option<StyleId>,
}

impl<W> Watch<W> {
    pub(crate) fn new(
        changed_fn: ChangedFn<W>,
        build_fn: BuildFn<W>,
        style_id: Option<StyleId>,
    ) -> Self {
        Self {
            changed_fn,
            build_fn,
            style_id,
        }
    }

    /// Returns `true` if the watched source changed.
    pub fn is_changed(&self, world: &W) -> bool {
        (self.changed_fn)(world)
    }

    /// Rebuilds the subtree under the [`WatchElement`] at `id`: drops
    /// the old child, builds a fresh one under the captured style
    /// scope, re-parents it, and marks the holder dirty.
    ///
    /// A no-op if the holder is gone, e.g. when an ancestor's rebuild
    /// already discarded it earlier in the same flush (the watch was
    /// dropped with it).
    pub fn build(
        &self,
        id: ElementId,
        fynix: &mut Fynix<W>,
        world: &mut W,
    ) {
        let Some(old_child) = fynix
            .elements
            .get_typed_mut::<WatchElement>(&id)
            .map(|elem| elem.child.take())
        else {
            return;
        };

        if let Some(old_child) = old_child {
            fynix.remove_element(&old_child);
        }

        // Rebuild under the watch's captured style scope, then drop
        // any uncommitted style changes so they do not leak.
        let child = {
            let mut ctx = fynix.create_ctx(world, self.style_id);
            (self.build_fn)(&mut ctx)
        };
        fynix.styles.clear_builder();

        if let Some(child_id) = child
            && let Some(node) =
                fynix.elements.table.node_mut(&child_id)
        {
            node.parent_id = Some(id);
        }
        if let Some(elem) =
            fynix.elements.get_typed_mut::<WatchElement>(&id)
        {
            elem.child = child;
        }

        // Mark the rebuilt subtree dirty so it is re-laid-out and
        // re-rendered.
        fynix.elements.mark_dirty(id);
    }
}

impl<W> Copy for Watch<W> {}

impl<W> Clone for Watch<W> {
    fn clone(&self) -> Self {
        *self
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

    /// The watch captures `value` at build time and only rebuilds
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
            ctx.watch(
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
                .get_typed::<WatchElement>(&holder)
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
        fynix.update_watches(&mut world);
        let (child, n) = child_n(&fynix);
        assert_eq!(child, first_child);
        assert_eq!(n, 1);

        // Flip `changed`: subtree rebuilds with the new value.
        world.changed = true;
        fynix.update_watches(&mut world);
        let (_, n) = child_n(&fynix);
        assert_eq!(n, 2);
    }
}
