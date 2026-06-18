use hashbrown::HashMap;
use typarena::ColumnId;

use crate::element::ElementId;
use crate::element::table::ElementTable;

/// Runs after a component is inserted or removed on an element,
/// receiving the table and that element's id so it can derive further
/// components.
pub type ObserverFn<W> = fn(&mut ElementTable<W>, ElementId);

/// Per-column insert/remove observers for an [`ElementTable`], keyed
/// by [`ColumnId`].
///
/// One observer per column per event; registering again replaces the
/// previous one.
pub struct Observers<W: 'static> {
    on_insert: HashMap<ColumnId, ObserverFn<W>>,
    on_remove: HashMap<ColumnId, ObserverFn<W>>,
}

impl<W> Observers<W> {
    pub fn new() -> Self {
        Self {
            on_insert: HashMap::new(),
            on_remove: HashMap::new(),
        }
    }

    /// Registers (or replaces) the insert observer for `col`.
    pub fn set_on_insert(
        &mut self,
        col: ColumnId,
        observer: ObserverFn<W>,
    ) {
        self.on_insert.insert(col, observer);
    }

    /// Registers (or replaces) the remove observer for `col`.
    pub fn set_on_remove(
        &mut self,
        col: ColumnId,
        observer: ObserverFn<W>,
    ) {
        self.on_remove.insert(col, observer);
    }

    /// The insert observer registered for `col`, if any.
    pub fn get_on_insert(
        &self,
        col: ColumnId,
    ) -> Option<ObserverFn<W>> {
        self.on_insert.get(&col).copied()
    }

    /// The remove observer registered for `col`, if any.
    pub fn get_on_remove(
        &self,
        col: ColumnId,
    ) -> Option<ObserverFn<W>> {
        self.on_remove.get(&col).copied()
    }
}

impl<W> Default for Observers<W> {
    fn default() -> Self {
        Self::new()
    }
}
