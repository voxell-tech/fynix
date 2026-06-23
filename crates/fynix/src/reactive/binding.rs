//! Field bindings: in-place updates of a single element's field,
//! driven by world change detection.
//!
//! A binding is the lightweight counterpart to a
//! [`Watcher`](crate::reactive::watcher::Watcher). Where a watcher
//! rebuilds a whole subtree, a binding reads one value from the world
//! and writes it into one element field, then marks it dirty.
//! Bindings are attached per instance via
//! [`ElementCtx::bind`](crate::ctx::ElementCtx::bind) and flushed
//! each frame by [`Fynix::sync`](crate::Fynix::sync).

use alloc::boxed::Box;

use crate::element::{Element, ElementId, Elements};
use crate::reactive::{Changed, ChangedFn};

/// Shorthand for the world reader a binding accepts: any
/// `Fn(&W) -> T + 'static`.
pub trait GetFn<W, T>: Fn(&W) -> T + 'static {}

impl<W, T, F> GetFn<W, T> for F where F: Fn(&W) -> T + 'static {}

/// Shorthand for the field accessor a binding accepts: any
/// `Fn(&mut E) -> &mut T + 'static`.
pub trait SetFn<E, T>: Fn(&mut E) -> &mut T + 'static {}

impl<E, T, F> SetFn<E, T> for F where F: Fn(&mut E) -> &mut T + 'static
{}

/// Reads the new value from the world and writes it into the target
/// element's field, returning `false` if the element is absent.
type ApplyFn<W> =
    Box<dyn Fn(&ElementId, &mut Elements<W>, &W) -> bool>;

/// A single field binding, stored as a component on the element it
/// writes into (see [`ElementTable::insert_component`]). The target
/// element's [`ElementId`] is the component key, so the binding
/// carries no id of its own.
///
/// It reads a value from the world and writes it into the bound
/// element's field when the source changes. The reader and field
/// accessor are captured in a boxed `apply` closure, so `Binding`
/// stays free of the `E`/`T` parameters without any type erasure.
///
/// [`ElementTable::insert_component`]: crate::element::table::ElementTable::insert_component
pub struct Binding<W: 'static> {
    /// Reports whether the bound source changed since the last apply.
    changed: Changed<W>,
    /// Reads the world and writes the captured field.
    apply: ApplyFn<W>,
}

impl<W> Binding<W> {
    pub(crate) fn new<E: Element, T: 'static>(
        changed: impl ChangedFn<W>,
        get: impl GetFn<W, T>,
        set: impl SetFn<E, T>,
    ) -> Self {
        Self {
            changed: Changed::new(changed),
            apply: Box::new(move |id, elements, world| {
                let Some(element) = elements.get_typed_mut::<E>(id)
                else {
                    return false;
                };
                *set(element) = get(world);
                true
            }),
        }
    }

    /// Returns `true` if the bound source changed.
    pub fn is_changed(&self, world: &W) -> bool {
        self.changed.is_changed(world)
    }

    /// Reads the new value from `world` and writes it into `id`,
    /// marking it dirty.
    pub fn apply(
        &self,
        id: &ElementId,
        elements: &mut Elements<W>,
        world: &W,
    ) {
        if (self.apply)(id, elements, world) {
            elements.mark_dirty(*id);
        }
    }
}

#[cfg(test)]
mod tests {
    use rectree::{Constraint, Size};

    use crate::Fynix;
    use crate::element::layout::ElementNodes;
    use crate::element::{Element, ElementBuild, ElementId};
    use crate::init::Init;

    #[derive(Init, Element)]
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

    /// A binding writes the world value into the field only on a
    /// flush where `changed` reports true, leaving it untouched
    /// otherwise.
    #[test]
    fn writes_field_only_when_changed() {
        let mut world = World {
            changed: false,
            value: 1,
        };
        let mut fynix = Fynix::<World>::new();

        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Counter>()
                .bind(|w| w.changed, |w| w.value, |c| &mut c.n)
                .id()
        };

        let n = |fynix: &Fynix<World>| {
            fynix.elements.get_typed::<Counter>(&id).unwrap().n
        };

        // Binding does not apply on attach; the field keeps its
        // initial value.
        assert_eq!(n(&fynix), 0);

        // Value changes but `changed` is false: no write.
        world.value = 2;
        fynix.sync(&mut world);
        assert_eq!(n(&fynix), 0);

        // Flip `changed`: the field picks up the current value.
        world.changed = true;
        fynix.sync(&mut world);
        assert_eq!(n(&fynix), 2);
    }

    /// A binding writes only into the element it was attached to,
    /// leaving sibling instances of the same type alone.
    #[test]
    fn applies_only_to_bound_instance() {
        let mut world = World {
            changed: true,
            value: 7,
        };
        let mut fynix = Fynix::<World>::new();

        let (bound, plain) = {
            let mut ctx = fynix.root_ctx(&mut world);
            let bound = ctx
                .add::<Counter>()
                .bind(|w| w.changed, |w| w.value, |c| &mut c.n)
                .id();
            let plain = ctx.add::<Counter>().id();
            (bound, plain)
        };

        fynix.sync(&mut world);

        assert_eq!(
            fynix.elements.get_typed::<Counter>(&bound).unwrap().n,
            7
        );
        assert_eq!(
            fynix.elements.get_typed::<Counter>(&plain).unwrap().n,
            0
        );
    }

    /// Removing the element drops its binding with it, so a later
    /// flush is a no-op for that id rather than a stale write.
    #[test]
    fn removing_element_drops_binding() {
        let mut world = World {
            changed: true,
            value: 5,
        };
        let mut fynix = Fynix::<World>::new();

        let id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add::<Counter>()
                .bind(|w| w.changed, |w| w.value, |c| &mut c.n)
                .id()
        };

        assert!(fynix.remove_element(&id));

        // The binding rode the element table and is gone, so this
        // flush finds nothing to apply and does not panic.
        fynix.sync(&mut world);
        assert!(fynix.elements.get_typed::<Counter>(&id).is_none());
    }
}
