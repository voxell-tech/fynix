use core::any::TypeId;
use core::marker::PhantomData;

use hashbrown::HashSet;

use crate::ctx::FynixCtx;
use crate::element::ElementId;
use crate::id::{GenId, IdGenerator};
use crate::type_table::TypeTable;

/// Reactive signal store.
///
/// Holds typed signal values and tracks which signals have been
/// mutated since the last call to [`Self::take_dirty`].
pub struct Signals {
    values: TypeTable<SignalId>,
    dirty: HashSet<SignalId>,
    id_gen: SignalIdGenerator,
}

impl Signals {
    pub fn new() -> Self {
        Self {
            values: TypeTable::new(),
            dirty: HashSet::new(),
            id_gen: IdGenerator::new(),
        }
    }

    /// Creates a new signal with `initial` as its starting value and
    /// returns a typed handle to it.
    pub fn create<T: 'static>(
        &mut self,
        initial: T,
    ) -> SignalHandle<T> {
        let id = self.id_gen.new_id();
        self.values.insert(id, initial);
        SignalHandle {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns a reference to the current value of the signal.
    pub fn read<T: 'static>(
        &self,
        handle: SignalHandle<T>,
    ) -> Option<&T> {
        self.values.get::<T>(&handle.id)
    }

    /// Updates the value of the signal and marks it dirty.
    pub fn set<T: 'static>(
        &mut self,
        handle: SignalHandle<T>,
        value: T,
    ) {
        self.values.insert(handle.id, value);
        self.dirty.insert(handle.id);
    }

    /// Drains and returns the set of signals that have been mutated
    /// since the last call to this method.
    pub fn take_dirty(&mut self) -> HashSet<SignalId> {
        core::mem::take(&mut self.dirty)
    }
}

impl Default for Signals {
    fn default() -> Self {
        Self::new()
    }
}

pub struct UntypedScope {
    value_id: TypeId,
    world_id: TypeId,
    scope_fn: *const (),
}

impl UntypedScope {
    pub fn typed<V: 'static, W: 'static>(
        &self,
    ) -> Option<Scope<V, W>> {
        if self.value_id == TypeId::of::<V>()
            && self.world_id == TypeId::of::<W>()
        {
            return Some(unsafe { self.typed_unchecked() });
        }

        None
    }

    /// Recovers the typed [`Scope<V, W>`] without a type check.
    ///
    /// # Safety
    ///
    /// `V` must be the signal value type and `W` must be t= world
    /// type this scope was created for.
    pub unsafe fn typed_unchecked<V: 'static, W: 'static>(
        &self,
    ) -> Scope<V, W> {
        Scope {
            scope_fn: unsafe {
                core::mem::transmute::<*const (), ScopeFn<V, W>>(
                    self.scope_fn,
                )
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Scope<V: 'static, W: 'static> {
    scope_fn: ScopeFn<V, W>,
}

impl<V: 'static, W: 'static> Scope<V, W> {
    pub fn new(scope_fn: ScopeFn<V, W>) -> Self {
        Self { scope_fn }
    }

    pub fn untyped(&self) -> UntypedScope {
        UntypedScope {
            value_id: TypeId::of::<V>(),
            world_id: TypeId::of::<W>(),
            scope_fn: self.scope_fn as *const (),
        }
    }

    pub fn compute_scope(
        &self,
        value: &V,
        ctx: &mut FynixCtx<W>,
    ) -> ElementId {
        (self.scope_fn)(value, ctx)
    }
}

pub type ScopeFn<V, W> = fn(&V, &mut FynixCtx<W>) -> ElementId;

/// A typed handle to a signal value stored in [`Signals`].
///
/// `SignalHandle<T>` is `Copy`, so it can be freely moved into
/// closures or stored on elements without cloning.
pub struct SignalHandle<T> {
    id: SignalId,
    _marker: PhantomData<T>,
}

impl<T> Copy for SignalHandle<T> {}

impl<T> Clone for SignalHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Generational ID for signal instances.
pub type SignalId = GenId<_SignalMarker>;
pub type SignalIdGenerator = IdGenerator<_SignalMarker>;

#[doc(hidden)]
pub struct _SignalMarker;
