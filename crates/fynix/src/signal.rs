use core::marker::PhantomData;

use alloc::vec::Vec;
use field_path::accessor::UntypedAccessor;
use field_path::field_accessor::FieldAccessor;
use hashbrown::{HashMap, HashSet};

use crate::element::{Element, ElementId, Elements};
use crate::id::{GenId, IdGenerator};
use crate::type_table::TypeTable;

/// Reactive signal store.
///
/// Holds typed signal values and their element field bindings.
///
/// Call [`Self::flush`] once per frame to apply pending changes.
/// It returns the set of elements whose fields were written, for
/// the backend to schedule re-layout and repainting.
pub struct Signals {
    values: TypeTable<SignalId>,
    bindings: HashMap<SignalId, Vec<UntypedBinding>>,
    dirty: HashSet<SignalId>,
    id_gen: SignalIdGenerator,
}

impl Signals {
    pub fn new() -> Self {
        Self {
            values: TypeTable::new(),
            bindings: HashMap::new(),
            dirty: HashSet::new(),
            id_gen: IdGenerator::new(),
        }
    }

    /// Creates a new signal with `initial` as its starting value and
    /// returns a typed handle to it.
    pub fn create<T: Clone + 'static>(
        &mut self,
        initial: T,
    ) -> Signal<T> {
        let id = self.id_gen.new_id();
        self.values.insert(id, initial);
        Signal {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns a reference to the current value of `signal`.
    pub fn get<T: 'static>(&self, signal: Signal<T>) -> Option<&T> {
        self.values.get::<T>(&signal.id)
    }

    /// Updates the value of `signal` and marks it dirty for
    /// the next flush.
    pub fn set<T: Clone + 'static>(
        &mut self,
        signal: Signal<T>,
        value: T,
    ) {
        self.values.insert(signal.id, value);
        self.dirty.insert(signal.id);
    }

    /// Binds `signal` to the field described by `accessor` on
    /// `element_id`. When the signal changes, the new value
    /// is written directly into that field during flush.
    pub fn bind<E, T>(
        &mut self,
        element_id: ElementId,
        signal: Signal<T>,
        accessor: FieldAccessor<E, T>,
    ) where
        E: Element,
        T: Clone + 'static,
    {
        self.bindings.entry(signal.id).or_default().push(
            UntypedBinding {
                element_id,
                signal_id: signal.id,
                accessor: accessor.accessor.untyped(),
                apply_binding_fn: apply_binding::<E, T>,
            },
        );
    }

    /// Applies all pending signal changes to bound element fields
    /// and returns the set of elements that were written, for the
    /// backend to schedule re-layout and repainting.
    pub fn flush(
        &mut self,
        elements: &mut Elements,
    ) -> HashSet<ElementId> {
        let mut dirty_elements = HashSet::new();
        let dirty: Vec<SignalId> = self.dirty.drain().collect();
        for signal_id in dirty {
            let Some(bindings) = self.bindings.get(&signal_id) else {
                continue;
            };
            for binding in bindings {
                (binding.apply_binding_fn)(
                    &binding.element_id,
                    &binding.signal_id,
                    &binding.accessor,
                    elements,
                    &self.values,
                );
                dirty_elements.insert(binding.element_id);
            }
        }

        dirty_elements
    }
}

impl Default for Signals {
    fn default() -> Self {
        Self::new()
    }
}

/// A typed handle to a signal value stored in [`Signals`].
///
/// `Signal<T>` is `Copy`, so it can be freely moved into
/// closures or stored on elements without cloning.
pub struct Signal<T> {
    id: SignalId,
    _marker: PhantomData<T>,
}

impl<T> Copy for Signal<T> {}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Type-erased apply function for a signal binding.
///
/// Called during [`Signals::flush`] to write the current
/// signal value into the bound element field.
pub type ApplyBindingFn = fn(
    &ElementId,
    &SignalId,
    &UntypedAccessor,
    &mut Elements,
    &TypeTable<SignalId>,
);

/// A type-erased signal-to-field binding.
struct UntypedBinding {
    element_id: ElementId,
    signal_id: SignalId,
    accessor: UntypedAccessor,
    apply_binding_fn: ApplyBindingFn,
}

/// Monomorphized apply - reads the signal value and writes it
/// directly into the bound element field.
fn apply_binding<E: Element, T: Clone + 'static>(
    element_id: &ElementId,
    signal_id: &SignalId,
    accessor: &UntypedAccessor,
    elements: &mut Elements,
    values: &TypeTable<SignalId>,
) {
    if let Some(value) = values.get::<T>(signal_id)
        && let Some(element) =
            elements.elements.get_mut::<E>(element_id)
        && let Some(acc) = accessor.typed::<E, T>()
    {
        *acc.get_mut(element) = value.clone();
    }
}

/// Generational ID for signal instances.
pub type SignalId = GenId<_SignalMarker>;
pub type SignalIdGenerator = IdGenerator<_SignalMarker>;

#[doc(hidden)]
pub struct _SignalMarker;
