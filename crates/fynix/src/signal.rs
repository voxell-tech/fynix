use core::any::TypeId;
use core::marker::PhantomData;

use hashbrown::hash_map::Entry;
use hashbrown::{HashMap, HashSet};
use sparse_map::{Key, SparseMap};

use crate::ctx::FynixCtx;
use crate::element::{Element, ElementId};
use crate::id::{GenId, IdGenerator};
use crate::signal::meta::SignalMetas;
use crate::type_table::TypeTable;

pub mod meta;

/// Reactive signal store.
///
/// Holds typed signal values and tracks which signals have been
/// mutated since the last call to [`Self::take_dirty`].
pub struct Signals {
    signals: TypeTable<SignalId>,
    metas: SignalMetas,
    scopes: HashMap<SignalId, SparseMap<UntypedScope>>,
    // _scopes: SparseMap<(SignalId, UntypedScope)>,
    dirty: HashSet<SignalId>,
    id_gen: SignalIdGenerator,
}

impl Signals {
    pub fn new() -> Self {
        Self {
            signals: TypeTable::new(),
            metas: SignalMetas::new(),
            scopes: HashMap::new(),
            dirty: HashSet::new(),
            id_gen: IdGenerator::new(),
        }
    }

    /// Creates a new signal with `initial` as its starting value and
    /// returns a typed handle to it.
    pub fn add_signal<T: 'static>(
        &mut self,
        initial: T,
    ) -> SignalHandle<T> {
        let id = self.id_gen.new_id();
        self.signals.insert(id, initial);
        self.metas.init::<T>(id);
        SignalHandle {
            id,
            _marker: PhantomData,
        }
    }

    pub fn remove_signal(&mut self, id: impl Into<SignalId>) -> bool {
        let id = id.into();

        if let Some(meta) = self.metas.get(&id) {
            self.signals.dyn_remove(&meta.signal_id, &id);
            self.scopes.remove(&id);
            self.dirty.remove(&id);

            self.id_gen.recycle(id);
            return true;
        }

        false
    }

    pub fn add_scope<T: 'static, W: 'static>(
        &mut self,
        handle: SignalHandle<T>,
        scope: Scope<T, W>,
    ) -> Key {
        let untyped_scope = scope.untyped();
        match self.scopes.entry(handle.id) {
            Entry::Occupied(mut occupied_entry) => {
                occupied_entry.get_mut().insert(untyped_scope)
            }
            Entry::Vacant(vacant_entry) => vacant_entry
                .insert(SparseMap::new())
                .insert(untyped_scope),
        }
    }

    pub fn remove_scope() {}

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

    // pub fn get_scopes(
    //     &self,
    //     id: &SignalId,
    // ) -> Option<&[UntypedScope]> {
    //     self.scopes.get(id).map(|s| s.as_slice())
    // }

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

// pub trait XTrait {}

// impl XTrait for X {}

// impl<T: XTrait, U: XTrait> XTrait for (T, U) {}
// impl<T: XTrait, U: XTrait, V: XTrait> XTrait for (T, U, V) {}

// fn create_element<W>(ctx: &mut FynixCtx<W>) {
//     let sig_handle = ctx.world.counter_handle;
//     ctx.scope_signals::<(T, U, V)>((sig1, sig2, sig3), |value, ctx| {
//         ctx.add_with::<E>(|e, _| {
//             e.x = value;
//         });
//     });
//     // let bind_handle = ctx.world.counter_handle;
//     // ctx.bind
// }

// TODO: Create derive(ElementSlot) & derive(Element)
pub struct ScopeEl {
    pub id: SignalId,
    pub scope: UntypedScope,
}

// impl Element for ScopeEl {
//     fn new() -> Self
//     where
//         Self: Sized,
//     {
//         todo!()
//     }

//     fn build(
//         &self,
//         id: &ElementId,
//         constraint: rectree::Constraint,
//         nodes: &mut crate::element::ElementNodes,
//     ) -> rectree::Size {
//         todo!()
//     }
// }

/// Generational ID for signal instances.
pub type SignalId = GenId<_SignalMarker>;
pub type SignalIdGenerator = IdGenerator<_SignalMarker>;

#[doc(hidden)]
pub struct _SignalMarker;
