use alloc::vec::Vec;

use field_path::accessor::func_pointers::{MutFn, MutFnPtr};
use hashbrown::HashMap;
use sparse_map::{Key, SparseMap};

use crate::element::{Element, ElementId, Elements};

/// Store of every reactive bound to the element tree, fixed to the
/// single world type `W`.
///
/// Backed by a concrete [`SparseMap`] keyed by [`BindingId`].
pub struct Bindings<W> {
    bindings: SparseMap<Binding<W>>,
    /// Reverse index from each holder element to its reactive, used
    /// to remove the reactive when the element is removed.
    element_map: HashMap<ElementId, BindingId>,
}

impl<W> Bindings<W> {
    pub fn new() -> Self {
        Self {
            bindings: SparseMap::new(),
            element_map: HashMap::new(),
        }
    }

    pub fn add(&mut self, binding: Binding<W>) -> BindingId {
        let element_id = binding.element_id;
        let binding_id = BindingId(self.bindings.insert(binding));
        self.element_map.insert(element_id, binding_id);

        binding_id
    }

    /// Removes the binding bound to `element_id`, if any.
    ///
    /// A no-op for elements that do not own a binding.
    pub(crate) fn remove_for_element(
        &mut self,
        element_id: &ElementId,
    ) {
        if let Some(binding_id) = self.element_map.remove(element_id)
        {
            self.bindings.remove(&binding_id.0);
        }
    }

    /// Snapshots every binding whose `changed_fn` reports a change
    /// against `world`.
    ///
    /// [`Binding`] is [`Copy`], so the snapshot detaches the changed
    /// bindings from the store, letting the caller borrow the rest
    /// of `Fynix` while it rebuilds each one.
    pub(crate) fn snapshot_changed(
        &self,
        world: &W,
    ) -> Vec<Binding<W>> {
        self.bindings
            .iter()
            .filter(|binding| binding.is_changed(world))
            .copied()
            .collect()
    }
}

impl<W> Default for Bindings<W> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Binding<W> {
    /// Function that determines if something has changed and require
    /// a re-creation of [`ReactiveElement::child`].
    changed_fn: ChangedFn<W>,
    get_fn: GetFnPtr,
    mut_fn: MutFnPtr,
    /// Apply the binding value.
    apply_fn: ApplyFn<W>,
    element_id: ElementId,
}

impl<W> Binding<W> {
    pub(crate) fn new<E: Element, T>(
        changed_fn: ChangedFn<W>,
        get_fn: GetFn<W, T>,
        mut_fn: MutFn<E, T>,
        element_id: ElementId,
    ) -> Self {
        Self {
            changed_fn,
            get_fn: GetFnPtr::new(get_fn),
            mut_fn: MutFnPtr::new(mut_fn),
            apply_fn: apply::<W, E, T>,
            element_id,
        }
    }

    pub fn element_id(&self) -> ElementId {
        self.element_id
    }

    pub fn is_changed(&self, world: &W) -> bool {
        (self.changed_fn)(world)
    }

    pub fn apply(&self, elements: &mut Elements, world: &W) {
        (self.apply_fn)(
            world,
            elements,
            &self.element_id,
            self.get_fn,
            self.mut_fn,
        )
    }
}

impl<W> Copy for Binding<W> {}

impl<W> Clone for Binding<W> {
    fn clone(&self) -> Self {
        *self
    }
}

pub type ChangedFn<W> = fn(&W) -> bool;

pub type ApplyFn<W> = fn(
    world: &W,
    elements: &mut Elements,
    id: &ElementId,
    get_fn: GetFnPtr,
    get_mut: MutFnPtr,
);

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

/// A type-erased mutable field accessor function pointer.
#[derive(Debug, Clone, Copy)]
pub struct GetFnPtr(*const ());

unsafe impl Send for GetFnPtr {}
unsafe impl Sync for GetFnPtr {}

impl GetFnPtr {
    /// Creates a new erased type of [`MutFn<S, T>`].
    pub const fn new<W, T>(f: GetFn<W, T>) -> Self {
        Self(f as *const ())
    }

    /// Re-interprets this pointer as a typed [`MutFn`] without
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

pub type GetFn<W, T> = fn(&W) -> T;

/// Generational ID for reactive instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingId(Key);

impl BindingId {
    /// A sentinel id that will never refer to a live reactive.
    pub const PLACEHOLDER: Self = Self(Key::PLACEHOLDER);
}

impl core::ops::Deref for BindingId {
    type Target = Key;
    fn deref(&self) -> &Key {
        &self.0
    }
}
