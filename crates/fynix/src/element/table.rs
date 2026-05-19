use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::element::ElementGroup;
use crate::element::{Element, ElementId};
use crate::type_table::DynTypeMap;
use crate::type_table::TypeMap;
use typeslot::SlotGroup;

/// Slot-indexed element storage, keyed by [`ElementId`].
///
/// Each element type is assigned a unique slot index at startup by
/// [`crate::Fynix::new`].
/// Typed access is then a direct [`Vec`] index.
pub struct ElementTable {
    columns: Vec<Option<DynTypeMap<ElementId>>>,
}

impl ElementTable {
    pub fn new() -> Self {
        let mut columns = Vec::new();
        columns.resize_with(ElementGroup::len(), || None);
        Self { columns }
    }

    /// Inserts `value` under `key`.
    ///
    /// Creates the column on first use. Returns the displaced
    /// value if one was already present.
    pub fn insert<E: Element>(
        &mut self,
        key: ElementId,
        value: E,
    ) -> Option<E> {
        let slot = ElementGroup::slot::<E>();
        // SAFETY: the column at `slot` was created as
        // `TypeMap<ElementId, E>`. TypeSlot guarantees each
        // type gets a unique slot, so no other type shares
        // this column.
        let col = self.columns[slot].get_or_insert_with(|| {
            Box::new(TypeMap::<ElementId, E>::new())
        });
        let map = unsafe { col.downcast_unchecked_mut::<E>() };
        map.insert(key, value)
    }

    /// Returns a reference to the value stored under `key`.
    pub fn get<E: Element>(&self, key: &ElementId) -> Option<&E> {
        let slot = ElementGroup::slot::<E>();
        let col = self.columns[slot].as_ref()?;
        // SAFETY: see [`Self::insert`].
        let map = unsafe { col.downcast_unchecked_ref::<E>() };
        map.get(key)
    }

    /// Returns a mutable reference to the value stored under `key`.
    pub fn get_mut<E: Element>(
        &mut self,
        key: &ElementId,
    ) -> Option<&mut E> {
        let slot = ElementGroup::slot::<E>();
        let col = self.columns[slot].as_mut()?;
        // SAFETY: see [`Self::insert`].
        let map = unsafe { col.downcast_unchecked_mut::<E>() };
        map.get_mut(key)
    }

    /// Removes and returns the value stored under `key`.
    pub fn remove<E: Element>(
        &mut self,
        key: &ElementId,
    ) -> Option<E> {
        let slot = ElementGroup::slot::<E>();
        let col = self.columns[slot].as_mut()?;
        // SAFETY: see [`Self::insert`].
        let map = unsafe { col.downcast_unchecked_mut::<E>() };
        map.remove(key)
    }

    /// Temporarily removes element `E` at `key`, calls `f` with mutable
    /// access to both the value and the remaining table, then reinserts it.
    ///
    /// Returns `None` if `key` is not present for `E`.
    pub fn scope<E: Element, T>(
        &mut self,
        key: &ElementId,
        f: impl FnOnce(&mut E, &mut Self) -> T,
    ) -> Option<T> {
        let mut value = self.remove::<E>(key)?;
        let result = f(&mut value, self);
        self.insert(*key, value);
        Some(result)
    }

    /// Removes `key` from the column at `slot`.
    ///
    /// Returns `true` if the key was present and removed.
    pub fn dyn_remove_by_slot(
        &mut self,
        slot: usize,
        key: &ElementId,
    ) -> bool {
        let Some(Some(col)) = self.columns.get_mut(slot) else {
            return false;
        };
        col.dyn_remove(key)
    }
}

impl Default for ElementTable {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
