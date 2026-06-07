use alloc::boxed::Box;
use core::any::TypeId;
use core::hash::{BuildHasher, Hash};

use hashbrown::hash_map::Entry;
use hashbrown::{DefaultHashBuilder, HashMap};
use indexmap::IndexMap;

use crate::typing::any_index_map::DynIndexMap;

/// Heterogeneous, key-addressed table.
///
/// A `TypeTable` stores values of many different types, all reachable
/// by a shared key type `K`. Each value type `V` occupies its own
/// [`IndexMap<K, V, S>`] column, type-erased behind a
/// [`DynIndexMap`]. A lookup hashes the value's [`TypeId`] to find
/// its column, then hashes `K` within that column.
///
/// Unlike [`TypePool`](super::type_pool::TypePool), which hands back
/// an opaque key for every insert, a `TypeTable` is addressed by the
/// caller's own key, so the same `K` reaches every column.
///
/// ## Mental model
///
/// |  key  | `Position` col | `Name` col |
/// |-------|----------------|------------|
/// | `e0`  | (1.0, 2.0)     | "player"   |
/// | `e1`  | (3.0, 4.0)     | -          |
pub struct TypeTable<K, S = DefaultHashBuilder> {
    columns: HashMap<TypeId, DynIndexMap<K, S>>,
    hasher: S,
}

impl<K> TypeTable<K> {
    /// Creates an empty table using the default hasher.
    pub fn new() -> Self {
        Self::with_hasher(DefaultHashBuilder::default())
    }
}

impl<K, S> TypeTable<K, S> {
    /// Creates an empty table seeded with `hasher`.
    ///
    /// Every column clones `hasher` for its own [`IndexMap`].
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            columns: HashMap::new(),
            hasher,
        }
    }
}

impl<K, S> TypeTable<K, S>
where
    K: Eq + Hash + 'static,
    S: BuildHasher,
{
    /// Returns a reference to the value of type `V` at `key`, or
    /// `None` if it is absent.
    pub fn get<V: 'static>(&self, key: &K) -> Option<&V> {
        self.column::<V>()?.get(key)
    }

    /// Returns a mutable reference to the value of type `V` at `key`,
    /// or `None` if it is absent.
    pub fn get_mut<V: 'static>(&mut self, key: &K) -> Option<&mut V> {
        self.column_mut::<V>()?.get_mut(key)
    }

    /// Returns `true` if a value of type `V` is present at `key`.
    pub fn contains<V: 'static>(&self, key: &K) -> bool {
        self.column::<V>().is_some_and(|c| c.contains_key(key))
    }

    /// Removes and returns the value of type `V` at `key`.
    ///
    /// Removal swaps the entry with the last in its column, so it is
    /// *O(1)* but does not preserve column order.
    pub fn remove<V: 'static>(&mut self, key: &K) -> Option<V> {
        self.column_mut::<V>()?.swap_remove(key)
    }

    /// Removes `key` from every column, dropping the whole row.
    ///
    /// Each column is swap-removed independently without knowing its
    /// value type. Returns `true` if any column held `key`.
    pub fn remove_row(&mut self, key: &K) -> bool {
        let mut removed = false;
        for column in self.columns.values_mut() {
            removed |= column.dyn_swap_remove(key);
        }
        removed
    }

    /// Returns `true` if any column holds a value at `key`.
    pub fn contains_row(&self, key: &K) -> bool {
        self.columns.values().any(|c| c.dyn_contains(key))
    }

    /// Returns the number of values stored in the column for `V`.
    pub fn len<V: 'static>(&self) -> usize {
        self.column::<V>().map_or(0, IndexMap::len)
    }

    /// Returns `true` if the column for `V` holds no values.
    pub fn is_empty<V: 'static>(&self) -> bool {
        self.len::<V>() == 0
    }

    /// Iterates `(key, value)` pairs in the column for `V`.
    ///
    /// Yields nothing if no column for `V` has been created yet.
    pub fn iter<V: 'static>(&self) -> impl Iterator<Item = (&K, &V)> {
        self.column::<V>().into_iter().flat_map(IndexMap::iter)
    }

    /// Removes every value in the column for `V`, leaving it empty.
    ///
    /// Returns `true` if a column for `V` exists, or `false` if one
    /// has not been created yet.
    pub fn clear<V: 'static>(&mut self) -> bool {
        if let Some(column) = self.column_mut::<V>() {
            column.clear();
            return true;
        }
        false
    }

    /// Returns the [`IndexMap`] column for `V`, if one exists.
    fn column<V: 'static>(&self) -> Option<&IndexMap<K, V, S>> {
        self.columns.get(&TypeId::of::<V>())?.downcast_ref::<V>()
    }

    /// Returns a mutable [`IndexMap`] column for `V`, if one exists.
    fn column_mut<V: 'static>(
        &mut self,
    ) -> Option<&mut IndexMap<K, V, S>> {
        self.columns
            .get_mut(&TypeId::of::<V>())?
            .downcast_mut::<V>()
    }
}

impl<K, S> TypeTable<K, S>
where
    K: Eq + Hash + 'static,
    S: BuildHasher + Clone + 'static,
{
    /// Inserts `value` under `key` in the column for `V`.
    ///
    /// Returns the previous value at `key`, if one was present.
    pub fn insert<V: 'static>(
        &mut self,
        key: K,
        value: V,
    ) -> Option<V> {
        self.ensure_column::<V>().insert(key, value)
    }

    /// Returns the column for `V`, creating an empty one if needed.
    fn ensure_column<V: 'static>(
        &mut self,
    ) -> &mut IndexMap<K, V, S> {
        let column = match self.columns.entry(TypeId::of::<V>()) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(Box::new(
                IndexMap::<K, V, S>::with_hasher(self.hasher.clone()),
            )),
        };
        // SAFETY: the column for `TypeId::of::<V>()` always stores V.
        unsafe { column.downcast_unchecked_mut::<V>() }
    }
}

impl<K> Default for TypeTable<K> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::TypeTable;

    #[derive(Debug, PartialEq, Clone, Copy)]
    struct Position(f32, f32);

    #[derive(Debug, PartialEq, Clone)]
    struct Name(String);

    #[test]
    fn insert_and_get() {
        let mut table = TypeTable::<u32>::new();
        table.insert(0, Position(1.0, 2.0));
        assert_eq!(
            table.get::<Position>(&0),
            Some(&Position(1.0, 2.0))
        );
    }

    #[test]
    fn heterogeneous_by_shared_key() {
        let mut table = TypeTable::<u32>::new();
        table.insert(7, Position(3.0, 4.0));
        table.insert(7, Name(String::from("player")));
        assert_eq!(
            table.get::<Position>(&7),
            Some(&Position(3.0, 4.0))
        );
        assert_eq!(
            table.get::<Name>(&7),
            Some(&Name(String::from("player")))
        );
    }

    #[test]
    fn insert_returns_previous() {
        let mut table = TypeTable::<u32>::new();
        assert_eq!(table.insert(1, Position(0.0, 0.0)), None);
        assert_eq!(
            table.insert(1, Position(9.0, 9.0)),
            Some(Position(0.0, 0.0))
        );
    }

    #[test]
    fn get_mut_modifies_value() {
        let mut table = TypeTable::<u32>::new();
        table.insert(1, Position(0.0, 0.0));
        table.get_mut::<Position>(&1).unwrap().0 = 5.0;
        assert_eq!(
            table.get::<Position>(&1),
            Some(&Position(5.0, 0.0))
        );
    }

    #[test]
    fn remove_returns_value() {
        let mut table = TypeTable::<u32>::new();
        table.insert(1, Position(5.0, 5.0));
        assert_eq!(
            table.remove::<Position>(&1),
            Some(Position(5.0, 5.0))
        );
        assert!(table.get::<Position>(&1).is_none());
        assert!(table.remove::<Position>(&1).is_none());
    }

    #[test]
    fn remove_swaps_last_into_place() {
        let mut table = TypeTable::<u32>::new();
        table.insert(0, Position(0.0, 0.0));
        table.insert(1, Position(1.0, 1.0));
        table.insert(2, Position(2.0, 2.0));

        // Swap-remove the first entry; the others remain reachable.
        assert_eq!(
            table.remove::<Position>(&0),
            Some(Position(0.0, 0.0))
        );
        assert_eq!(
            table.get::<Position>(&2),
            Some(&Position(2.0, 2.0))
        );
        assert_eq!(
            table.get::<Position>(&1),
            Some(&Position(1.0, 1.0))
        );
        assert_eq!(table.len::<Position>(), 2);
    }

    #[test]
    fn remove_row_drops_every_column() {
        let mut table = TypeTable::<u32>::new();
        table.insert(1, Position(1.0, 1.0));
        table.insert(1, Name(String::from("player")));

        assert!(table.remove_row(&1));
        assert!(table.get::<Position>(&1).is_none());
        assert!(table.get::<Name>(&1).is_none());
        // A second removal finds nothing left.
        assert!(!table.remove_row(&1));
    }

    #[test]
    fn contains_row_reports_any_column() {
        let mut table = TypeTable::<u32>::new();
        assert!(!table.contains_row(&1));
        table.insert(1, Name(String::from("player")));
        assert!(table.contains_row(&1));
        assert!(!table.contains_row(&2));
    }

    #[test]
    fn type_isolation() {
        let mut table = TypeTable::<u32>::new();
        table.insert(0, 50u64);
        assert!(table.get::<u32>(&0).is_none());
        assert_eq!(table.get::<u64>(&0), Some(&50u64));
    }

    #[test]
    fn contains_reflects_presence() {
        let mut table = TypeTable::<u32>::new();
        assert!(!table.contains::<Position>(&0));
        table.insert(0, Position(0.0, 0.0));
        assert!(table.contains::<Position>(&0));
    }

    #[test]
    fn clear_empties_column() {
        let mut table = TypeTable::<u32>::new();
        table.insert(0, Position(0.0, 0.0));
        table.insert(1, Position(1.0, 1.0));

        assert!(table.clear::<Position>());
        assert_eq!(table.len::<Position>(), 0);
        assert!(table.get::<Position>(&0).is_none());
    }

    #[test]
    fn clear_absent_column_returns_false() {
        let mut table = TypeTable::<u32>::new();
        assert!(!table.clear::<Position>());
    }

    #[test]
    fn iter_yields_all_pairs() {
        let mut table = TypeTable::<u32>::new();
        table.insert(0, Position(0.0, 0.0));
        table.insert(1, Position(1.0, 1.0));

        let mut keys = table
            .iter::<Position>()
            .map(|(k, _)| *k)
            .collect::<alloc::vec::Vec<_>>();
        keys.sort();
        assert_eq!(keys, [0, 1]);
    }
}
