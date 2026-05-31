use crate::element::{Element, ElementId};
use crate::type_table::{ColumnId, TypeTable};

/// Slot-indexed element storage, keyed by [`ElementId`].
///
/// Each element type is assigned a runtime [`ColumnId`] on first use.
/// Typed access resolves to a direct [`Vec`] index after the initial
/// [`TypeId`] lookup.
pub struct ElementTable {
    inner: TypeTable<ElementId>,
}

impl ElementTable {
    pub fn new() -> Self {
        Self {
            inner: TypeTable::new(),
        }
    }

    /// Ensures the column for `E` exists and returns its [`ColumnId`].
    ///
    /// The id is stable for the lifetime of this table and can be
    /// stored in per-element metadata for slot-based dispatch.
    pub fn ensure_column<E: Element>(&mut self) -> ColumnId {
        self.inner.ensure_column::<E>()
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
        self.inner.insert(key, value)
    }

    /// Returns a reference to the value stored under `key`.
    pub fn get<E: Element>(&self, key: &ElementId) -> Option<&E> {
        self.inner.get(key)
    }

    /// Returns a mutable reference to the value stored under `key`.
    pub fn get_mut<E: Element>(
        &mut self,
        key: &ElementId,
    ) -> Option<&mut E> {
        self.inner.get_mut(key)
    }

    /// Removes and returns the value stored under `key`.
    pub fn remove<E: Element>(
        &mut self,
        key: &ElementId,
    ) -> Option<E> {
        self.inner.remove(key)
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
    pub fn dyn_remove_by_column(
        &mut self,
        col: ColumnId,
        key: &ElementId,
    ) -> bool {
        self.inner.dyn_remove_by_column(col, key)
    }
}

impl Default for ElementTable {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
