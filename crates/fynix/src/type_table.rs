use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::hash::Hash;

use hashbrown::HashMap;
use hashbrown::hash_map::Entry;
use sparse_map::{Key, SparseMap};

/// Heterogeneous table mapping keys of type `K` to typed values.
///
/// Each key can be associated with at most one value per concrete
/// type `T`. Internally, one [`TypeMap<K, T>`] column is allocated
/// the first time a value of type `T` is inserted.
///
/// ## Mental model
///
/// | key | `f32` | `u32` | `i32` |
/// |-----|-------|-------|-------|
/// | k1  | -     | 10    | -10   |
/// | k2  | -     | -     | -24   |
/// | k3  | 3.14  | -     | -     |
///
/// Columns are stored in a [`Vec`] and indexed by a column number.
/// The `slots` map translates [`TypeId`] to a column index once on
/// first use; subsequent accesses go straight to the [`Vec`] by
/// index.
pub struct TypeTable<K> {
    columns_map: HashMap<TypeId, ColumnId>,
    columns: Vec<DynTypeMap<K>>,
}

impl<K> TypeTable<K> {
    /// Creates an empty [`TypeTable`].
    pub fn new() -> Self {
        Self {
            columns_map: HashMap::new(),
            columns: Vec::new(),
        }
    }

    /// Returns the [`ColumnId`] for `T`, or `None` if no value of
    /// type `T` has ever been inserted.
    ///
    /// The returned id is stable for the lifetime of this table and
    /// can be stored to bypass the [`TypeId`] lookup on hot paths.
    pub fn type_column<T: 'static>(&self) -> Option<ColumnId> {
        self.columns_map.get(&TypeId::of::<T>()).copied()
    }
}

impl<K> TypeTable<K>
where
    K: Hash + Eq + 'static,
{
    /// Ensures the column for `T` exists and returns its
    /// [`ColumnId`].
    ///
    /// Like [`Self::type_column`] but creates the column on first
    /// call rather than returning `None`. The returned id is stable
    /// for the lifetime of this table.
    pub fn ensure_column<T: 'static>(&mut self) -> ColumnId {
        match self.columns_map.entry(TypeId::of::<T>()) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let col = ColumnId(self.columns.len());
                e.insert(col);
                self.columns.push(Box::new(TypeMap::<K, T>::new()));
                col
            }
        }
    }

    /// Inserts `value` of type `T` under `key`.
    ///
    /// Creates the column for `T` on first use.
    /// Returns the displaced value if one was already present.
    pub fn insert<T: 'static>(
        &mut self,
        key: K,
        value: T,
    ) -> Option<T> {
        let col = self.ensure_column::<T>();
        // SAFETY: col_id was just assigned for T by ensure_column.
        let map = unsafe {
            self.columns[col.index()].downcast_unchecked_mut::<T>()
        };
        map.insert(key, value)
    }

    /// Returns a reference to the `T`-typed value stored under `key`,
    /// or `None` if no such entry exists.
    pub fn get<T: 'static>(&self, key: &K) -> Option<&T> {
        let col = self.columns_map.get(&TypeId::of::<T>())?;
        // SAFETY: col_id was assigned for T.
        let map = unsafe {
            self.columns[col.index()].downcast_unchecked_ref::<T>()
        };
        map.get(key)
    }

    /// Returns a mutable reference to the `T`-typed value stored
    /// under `key`, or `None` if no such entry exists.
    pub fn get_mut<T: 'static>(&mut self, key: &K) -> Option<&mut T> {
        let col = self.columns_map.get(&TypeId::of::<T>())?;
        // SAFETY: col_id was assigned for T.
        let map = unsafe {
            self.columns[col.index()].downcast_unchecked_mut::<T>()
        };
        map.get_mut(key)
    }

    /// Returns a reference to the `T`-typed value stored under `key`
    /// using a pre-resolved [`ColumnId`].
    ///
    /// Returns `None` if `col` is out of bounds, the column holds a
    /// different type, or `key` is absent.
    pub fn get_by_column<T: 'static>(
        &self,
        col: ColumnId,
        key: &K,
    ) -> Option<&T> {
        self.columns.get(col.index())?.downcast_ref::<T>()?.get(key)
    }

    /// Returns a mutable reference to the `T`-typed value stored
    /// under `key` using a pre-resolved [`ColumnId`].
    ///
    /// Returns `None` if `col` is out of bounds, the column holds a
    /// different type, or `key` is absent.
    pub fn get_mut_by_column<T: 'static>(
        &mut self,
        col: ColumnId,
        key: &K,
    ) -> Option<&mut T> {
        self.columns
            .get_mut(col.index())?
            .downcast_mut::<T>()?
            .get_mut(key)
    }

    /// Removes and returns the `T`-typed value stored under `key`, or
    /// `None` if none exists.
    pub fn remove<T: 'static>(&mut self, key: &K) -> Option<T> {
        let col = self.columns_map.get(&TypeId::of::<T>())?;
        // SAFETY: col_id was assigned for T.
        let map = unsafe {
            self.columns[col.index()].downcast_unchecked_mut::<T>()
        };
        map.remove(key)
    }

    /// Removes `key` from the column identified by `type_id`, without
    /// knowing the value type at compile time.
    ///
    /// Returns `true` if the column existed and the key was present
    /// in it.
    pub fn dyn_remove(&mut self, type_id: &TypeId, key: &K) -> bool {
        if let Some(col) = self.columns_map.get(type_id) {
            return self.columns[col.index()].dyn_remove(key);
        }
        false
    }

    /// Removes `key` from the column at `slot` without knowing the
    /// value type at compile time.
    ///
    /// Returns `true` if the key was present and removed.
    pub fn dyn_remove_by_column(
        &mut self,
        col: ColumnId,
        key: &K,
    ) -> bool {
        match self.columns.get_mut(col.index()) {
            Some(col) => col.dyn_remove(key),
            None => false,
        }
    }

    /// Temporarily removes the `T`-typed value at `key`, calls `f`
    /// with mutable access to both the value and the remaining table,
    /// then reinserts it.
    ///
    /// Returns `None` if `key` is not present for `T`.
    pub fn scope<T: 'static, R>(
        &mut self,
        key: &K,
        f: impl FnOnce(&mut T, &mut Self) -> R,
    ) -> Option<R>
    where
        K: Copy,
    {
        let mut value = self.remove::<T>(key)?;
        let result = f(&mut value, self);
        self.insert(*key, value);
        Some(result)
    }

    /// Removes `key` from every type column.
    ///
    /// Returns `true` if at least one column contained an entry for
    /// `key`.
    pub fn remove_all(&mut self, key: &K) -> bool {
        let mut has_removed = false;
        for col in &mut self.columns {
            has_removed |= col.dyn_remove(key);
        }
        has_removed
    }
}

impl<K> Default for TypeTable<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Opaque index into a [`TypeTable`]'s column [`Vec`].
///
/// Obtained from [`TypeTable::type_column`] and passed to
/// [`TypeTable::get_by_column`] / [`TypeTable::get_mut_by_column`] to
/// skip the [`TypeId`] → slot [`HashMap`] lookup on hot paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnId(usize);

impl ColumnId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// Typed column inside a [`TypeTable`]: maps keys of type `K` to
/// values of type `T`, backed by a [`SparseMap`] for cache-friendly
/// dense storage.
pub struct TypeMap<K, T> {
    values: SparseMap<T>,
    map: HashMap<K, Key>,
}

impl<K, T> TypeMap<K, T> {
    /// Creates an empty [`TypeMap`].
    pub fn new() -> Self {
        Self {
            values: SparseMap::new(),
            map: HashMap::new(),
        }
    }
}

impl<K, T> Default for TypeMap<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, T> TypeMap<K, T>
where
    K: Hash + Eq,
{
    /// Inserts `value` under `key`.
    ///
    /// Returns the displaced value if one was already present.
    pub fn insert(&mut self, key: K, value: T) -> Option<T> {
        let mut previous = None;
        if let Some(sparse_key) = self.map.get(&key) {
            previous = self.values.remove(sparse_key);
        }

        let sparse_key = self.values.insert(value);
        self.map.insert(key, sparse_key);

        previous
    }

    /// Returns a reference to the value stored under `key`, or `None`
    /// if absent.
    pub fn get(&self, key: &K) -> Option<&T> {
        self.map.get(key).and_then(|k| self.values.get(k))
    }

    /// Returns a mutable reference to the value stored under `key`,
    /// or `None` if absent.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut T> {
        self.map.get(key).and_then(|k| self.values.get_mut(k))
    }

    /// Removes and returns the value stored under `key`, or `None` if
    /// absent.
    pub fn remove(&mut self, key: &K) -> Option<T> {
        self.map.remove(key).and_then(|k| self.values.remove(&k))
    }
}

mod any_type_map {
    use core::any::TypeId;

    use super::*;

    /// Private trait to prevent other types from implementing the
    /// [`AnyTypeMap`] trait.
    trait Seal {}
    impl<K, T: 'static> Seal for TypeMap<K, T> {}

    #[expect(private_bounds)]
    pub trait AnyTypeMap<K>: Seal {
        fn element_type_id(&self) -> TypeId;

        /// Removes `key` from the map.
        ///
        /// Returns `true` if an entry was present and removed.
        fn dyn_remove(&mut self, key: &K) -> bool
        where
            K: Hash + Eq;
    }

    impl<K, T: 'static> AnyTypeMap<K> for TypeMap<K, T> {
        fn element_type_id(&self) -> TypeId {
            TypeId::of::<T>()
        }

        fn dyn_remove(&mut self, id: &K) -> bool
        where
            K: Hash + Eq,
        {
            self.remove(id).is_some()
        }
    }

    impl<K> dyn AnyTypeMap<K> {
        #[inline]
        pub fn element_is<T: 'static>(&self) -> bool {
            self.element_type_id() == TypeId::of::<T>()
        }

        #[allow(unused)]
        #[inline]
        pub fn downcast_ref<T: 'static>(
            &self,
        ) -> Option<&TypeMap<K, T>> {
            if self.element_is::<T>() {
                unsafe { Some(self.downcast_unchecked_ref()) }
            } else {
                None
            }
        }

        #[allow(unused)]
        #[inline]
        pub fn downcast_mut<T: 'static>(
            &mut self,
        ) -> Option<&mut TypeMap<K, T>> {
            if self.element_is::<T>() {
                unsafe { Some(self.downcast_unchecked_mut()) }
            } else {
                None
            }
        }

        /// # Safety
        ///
        /// Calling this method with the incorrect type is
        /// *undefined behavior*.
        #[inline]
        pub unsafe fn downcast_unchecked_ref<T: 'static>(
            &self,
        ) -> &TypeMap<K, T> {
            debug_assert!(self.element_is::<T>());
            unsafe { &*(self as *const Self as *const TypeMap<K, T>) }
        }

        /// # Safety
        ///
        /// Calling this method with the incorrect type is
        /// *undefined behavior*.
        #[inline]
        pub unsafe fn downcast_unchecked_mut<T: 'static>(
            &mut self,
        ) -> &mut TypeMap<K, T> {
            debug_assert!(self.element_is::<T>());
            unsafe { &mut *(self as *mut Self as *mut TypeMap<K, T>) }
        }
    }
}

pub type DynTypeMap<K> = Box<dyn any_type_map::AnyTypeMap<K>>;

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::*;

    // Helper types for testing heterogeneous storage
    #[derive(Debug, PartialEq, Clone, Copy)]
    struct Velocity(f32);

    #[derive(Debug, PartialEq, Clone)]
    struct Name(String);

    #[derive(Debug, PartialEq, Eq, Hash, Clone)]
    struct CustomKey(u32);

    #[test]
    fn test_basic_insert_and_get() {
        let mut maps = TypeTable::<u32>::new();
        let id = 1;

        maps.insert(id, Velocity(10.5));

        let retrieved = maps.get::<Velocity>(&id);
        assert_eq!(retrieved, Some(&Velocity(10.5)));
    }

    #[test]
    fn test_heterogeneous_storage() {
        let mut maps = TypeTable::<u32>::new();
        let id = 42;

        // Store different types for the same key
        maps.insert(id, Velocity(20.0));
        maps.insert(id, Name("Entity_1".into()));

        // Ensure both exist and are distinct
        assert_eq!(maps.get::<Velocity>(&id), Some(&Velocity(20.0)));
        assert_eq!(
            maps.get::<Name>(&id),
            Some(&Name("Entity_1".into()))
        );
    }

    #[test]
    fn test_overwrite_behavior() {
        let mut maps = TypeTable::<u32>::new();
        let id = 7;

        maps.insert(id, 100);
        // Inserting again should return the old value
        let old_value = maps.insert(id, 200);

        assert_eq!(old_value, Some(100));
        assert_eq!(maps.get(&id), Some(&200));
    }

    #[test]
    fn test_remove_logic() {
        let mut maps = TypeTable::<u32>::new();
        let id = 10;

        maps.insert(id, Velocity(5.0));

        let removed = maps.remove::<Velocity>(&id);
        assert_eq!(removed, Some(Velocity(5.0)));

        // Verify it's gone
        assert!(maps.get::<Velocity>(&id).is_none());
        // Verify removing again returns None
        assert!(maps.remove::<Velocity>(&id).is_none());
    }

    #[test]
    fn test_remove_from_internal_map() {
        let mut map = TypeMap::<u32, Velocity>::new();
        let id = 10;

        // Adding only one value
        map.insert(id, Velocity(5.0));
        // Removing that value
        assert_eq!(map.remove(&id), Some(Velocity(5.0)));

        // Verifying that both internal maps are empty
        assert!(map.values.is_empty());
        assert!(map.map.is_empty());
    }

    #[test]
    fn test_type_isolation() {
        let mut maps = TypeTable::<u32>::new();
        let id = 1;

        maps.insert(id, 50u64);

        // Querying for the wrong type should not return `None`.
        assert!(maps.get::<u32>(&id).is_none());
        assert!(maps.get::<i64>(&id).is_none());
        assert_eq!(maps.get::<u64>(&id), Some(&50u64));
    }

    #[test]
    fn test_generic_key_support() {
        // Test with a custom key type.
        let mut maps = TypeTable::<CustomKey>::new();
        let key = CustomKey(99);

        maps.insert(key.clone(), "Hello World");

        assert_eq!(
            maps.get::<&'static str>(&key),
            Some(&"Hello World")
        );
    }

    #[test]
    fn test_multiple_keys_one_type() {
        let mut maps = TypeTable::<u32>::new();

        maps.insert(1, Velocity(1.0));
        maps.insert(2, Velocity(2.0));

        assert_eq!(maps.get::<Velocity>(&1), Some(&Velocity(1.0)));
        assert_eq!(maps.get::<Velocity>(&2), Some(&Velocity(2.0)));
    }

    #[test]
    fn test_empty_map_get() {
        let maps: TypeTable<u32> = TypeTable::new();
        assert!(maps.get::<Velocity>(&0).is_none());
    }
}
