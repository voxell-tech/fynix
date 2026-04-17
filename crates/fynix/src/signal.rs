use core::any::TypeId;
use core::marker::PhantomData;

use alloc::vec;
use alloc::vec::Vec;
use hashbrown::{HashMap, HashSet};
use typeslot::SlotGroup;

use crate::ctx::FynixCtx;
use crate::element::ElementId;
use crate::id::{GenId, IdGenerator};
use crate::signal::meta::SignalMetas;
use crate::type_table::TypeTable;
use crate::{World, WorldGroup};

pub mod meta;

/// Reactive signal store.
///
/// Holds typed signal values and tracks which signals have been
/// mutated since the last call to [`Self::take_dirty`].
pub struct Signals {
    signals: TypeTable<SignalId>,
    metas: SignalMetas,
    scopes: Vec<HashMap<SignalId, UntypedScope>>,
    dirty: HashSet<SignalId>,
    id_gen: SignalIdGenerator,
}

impl Signals {
    pub fn new() -> Self {
        Self {
            signals: TypeTable::new(),
            metas: SignalMetas::new(),
            scopes: vec![HashMap::new(); WorldGroup::len()],
            dirty: HashSet::new(),
            id_gen: IdGenerator::new(),
        }
    }

    /// Creates a new signal with `initial` as its starting value and
    /// returns a typed handle to it.
    pub fn add<T: 'static>(&mut self, initial: T) -> SignalHandle<T> {
        let id = self.id_gen.new_id();
        self.signals.insert(id, initial);
        self.metas.init::<T>(id);
        SignalHandle {
            id,
            _marker: PhantomData,
        }
    }

    pub fn remove(&mut self, id: impl Into<SignalId>) -> bool {
        let id = id.into();

        if let Some(meta) = self.metas.get(&id) {
            self.signals.dyn_remove(&meta.signal_id, &id);
            self.dirty.remove(&id);
            for s in self.scopes.iter_mut() {
                s.remove(&id);
            }

            self.id_gen.recycle(id);
            return true;
        }

        false
    }

    pub fn set_scope<T: 'static, W: World>(
        &mut self,
        handle: SignalHandle<T>,
        scope: Scope<T, W>,
    ) {
        let slot = WorldGroup::slot::<W>();
        self.scopes[slot].insert(handle.id, scope.untyped());
    }

    /// Returns a reference to the current value of the signal.
    pub fn read<T: 'static>(
        &self,
        handle: SignalHandle<T>,
    ) -> Option<&T> {
        self.signals.get::<T>(&handle.id)
    }

    /// Updates the value of the signal and marks it dirty.
    pub fn set<T: 'static>(
        &mut self,
        handle: SignalHandle<T>,
        value: T,
    ) {
        self.signals.insert(handle.id, value);
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug)]
pub struct Scope<V, W> {
    scope_fn: ScopeFn<V, W>,
}

impl<V, W> Scope<V, W> {
    pub fn new(scope_fn: ScopeFn<V, W>) -> Self {
        Self { scope_fn }
    }

    pub fn untyped(&self) -> UntypedScope
    where
        V: 'static,
        W: 'static,
    {
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

impl<V, W> Copy for Scope<V, W> {}

impl<V, W> Clone for Scope<V, W> {
    fn clone(&self) -> Self {
        *self
    }
}

pub type ScopeFn<V, W> = fn(&V, &mut FynixCtx<W>) -> ElementId;

/// A typed handle to a signal value stored in [`Signals`].
pub struct SignalHandle<T> {
    id: SignalId,
    _marker: PhantomData<T>,
}

impl<T> From<SignalHandle<T>> for SignalId {
    fn from(value: SignalHandle<T>) -> Self {
        value.id
    }
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
