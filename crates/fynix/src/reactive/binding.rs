//! Field bindings: in-place updates of a single element's field,
//! driven by world change detection.
//!
//! A binding is the lightweight counterpart to a
//! [`Watch`](crate::reactive::watch::Watch). Where a watch rebuilds a
//! whole subtree, a binding reads one value from the world and writes
//! it into one element field, then marks that element dirty. Bindings
//! are attached per instance via
//! [`ElementCtx::bind`](crate::ctx::ElementCtx::bind) and flushed each
//! frame by [`Fynix::update_watches`](crate::Fynix::update_watches).

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
pub struct Binding<W> {
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
    pub fn build(
        &self,
        id: &ElementId,
        elements: &mut Elements,
        world: &W,
    ) {
        (self.apply_fn)(world, elements, id, self.get_fn, self.mut_fn)
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
pub type ApplyFn<W> = fn(
    world: &W,
    elements: &mut Elements,
    id: &ElementId,
    get_fn: GetFnPtr,
    get_mut: MutFnPtr,
);

/// Reads `T` from `world`, writes it into element `E`'s field, and
/// marks the element dirty. A no-op write if the element is absent
/// or of a different type, but it is still marked dirty.
fn apply<W, E: Element, T>(
    world: &W,
    elements: &mut Elements,
    id: &ElementId,
    get_fn: GetFnPtr,
    get_mut: MutFnPtr,
) {
    if let Some(element) = elements.get_typed_mut::<E>(id) {
        let get_fn = unsafe { get_fn.typed_unchecked::<W, T>() };
        let get_mut = unsafe { get_mut.typed_unchecked::<E, T>() };

        let v = get_fn(world);
        *get_mut(element) = v;
    }
    elements.mark_dirty(*id);
}

/// A type-erased [`GetFn`] (the world value reader) stored on a
/// [`Binding`] so it carries no `T` parameter.
#[derive(Debug, Clone, Copy)]
pub struct GetFnPtr(*const ());

unsafe impl Send for GetFnPtr {}
unsafe impl Sync for GetFnPtr {}

impl GetFnPtr {
    /// Erases a [`GetFn<W, T>`].
    pub const fn new<W, T>(f: GetFn<W, T>) -> Self {
        Self(f as *const ())
    }

    /// Re-interprets this pointer as a typed [`GetFn`] without
    /// checking type correctness.
    ///
    /// # Safety
    ///
    /// Undefined behavior if `S` and `T` do not match the types used
    /// when constructing this pointer.
    pub const unsafe fn typed_unchecked<S, T>(&self) -> GetFn<S, T> {
        unsafe {
            core::mem::transmute::<*const (), GetFn<S, T>>(self.0)
        }
    }
}

/// Reads a value of type `T` from the world.
pub type GetFn<W, T> = fn(&W) -> T;
