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

use field_path::accessor::func_pointers::{MutFn, MutFnPtr};

use crate::element::{Element, ElementId, Elements};
use crate::reactive::ChangedFn;

/// A single field binding, stored as a component on the element it
/// writes into (see [`ElementTable::insert_component`]). The target
/// element's [`ElementId`] is the component key, so the binding
/// carries no id of its own.
///
/// It reads `T` from the world and writes it into element `E`'s field
/// when the source changes. `get_fn` and `mut_fn` are type-erased to
/// keep `Binding` free of the `E`/`T` parameters; `apply_fn` is the
/// monomorphized writer that restores them and performs the write.
///
/// [`ElementTable::insert_component`]: crate::element::table::ElementTable::insert_component
#[derive(Debug)]
pub struct Binding<W: 'static> {
    /// Reports whether the bound source changed since the last
    /// apply.
    changed_fn: ChangedFn<W>,
    /// Type-erased `fn(&W) -> T` reading the new value.
    get_fn: GetFnPtr,
    /// Type-erased `fn(&mut E) -> &mut T` to the target field.
    mut_fn: MutFnPtr,
    /// Monomorphized writer that re-types and applies the value.
    apply_fn: ApplyFn<W>,
}

impl<W> Binding<W> {
    pub(crate) fn new<E: Element, T>(
        changed_fn: ChangedFn<W>,
        get_fn: GetFn<W, T>,
        mut_fn: MutFn<E, T>,
    ) -> Self {
        Self {
            changed_fn,
            get_fn: GetFnPtr::new(get_fn),
            mut_fn: MutFnPtr::new(mut_fn),
            apply_fn: apply::<W, E, T>,
        }
    }

    /// Returns `true` if the bound source changed.
    pub fn is_changed(&self, world: &W) -> bool {
        (self.changed_fn)(world)
    }

    /// Reads the new value from `world` and writes it into `id`,
    /// marking it dirty.
    pub fn apply(
        &self,
        id: &ElementId,
        elements: &mut Elements<W>,
        world: &W,
    ) {
        let success = (self.apply_fn)(
            world,
            elements,
            id,
            self.get_fn,
            self.mut_fn,
        );

        if success {
            elements.mark_dirty(*id);
        }
    }
}

impl<W> Copy for Binding<W> {}

impl<W> Clone for Binding<W> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Type-erased writer stored on a [`Binding`]. Monomorphized per
/// `(W, E, T)`, it restores the erased accessors and performs the
/// field write.
type ApplyFn<W> = fn(
    world: &W,
    elements: &mut Elements<W>,
    id: &ElementId,
    get_fn: GetFnPtr,
    get_mut: MutFnPtr,
) -> bool;

/// Reads `T` from `world`, writes it into element `E`'s field.
///
/// Returns `false` if element is absent.
fn apply<W, E: Element, T>(
    world: &W,
    elements: &mut Elements<W>,
    id: &ElementId,
    get_fn: GetFnPtr,
    get_mut: MutFnPtr,
) -> bool {
    if let Some(element) = elements.get_typed_mut::<E>(id) {
        let get_fn = unsafe { get_fn.typed_unchecked::<W, T>() };
        let get_mut = unsafe { get_mut.typed_unchecked::<E, T>() };

        let v = get_fn(world);
        *get_mut(element) = v;

        return true;
    }
    false
}

/// A type-erased [`GetFn`] (the world value reader) stored on a
/// [`Binding`] so it carries no `T` parameter.
#[derive(Debug, Clone, Copy)]
struct GetFnPtr(*const ());

unsafe impl Send for GetFnPtr {}
unsafe impl Sync for GetFnPtr {}

impl GetFnPtr {
    /// Erases a [`GetFn<W, T>`].
    const fn new<W, T>(f: GetFn<W, T>) -> Self {
        Self(f as *const ())
    }

    /// Re-interprets this pointer as a typed [`GetFn`] without
    /// checking type correctness.
    ///
    /// # Safety
    ///
    /// Undefined behavior if `S` and `T` do not match the types used
    /// when constructing this pointer.
    const unsafe fn typed_unchecked<S, T>(&self) -> GetFn<S, T> {
        unsafe {
            core::mem::transmute::<*const (), GetFn<S, T>>(self.0)
        }
    }
}

/// Reads a value of type `T` from the world.
pub type GetFn<W, T> = fn(&W) -> T;

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
