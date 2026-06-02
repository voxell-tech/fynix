use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;

use hashbrown::HashMap;
use hashbrown::hash_map::Entry;
use sparse_map::{Key, SparseMap};

/// Opaque index into a [`TypePool`]'s column [`Vec`].
///
/// Stable for the lifetime of the table. Encoded inside every
/// [`ColumnKey`] so lookups never touch the hash map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnId(usize);

impl ColumnId {
    /// A sentinel id that will never refer to a live column.
    pub const PLACEHOLDER: Self = Self(usize::MAX);

    pub fn index(self) -> usize {
        self.0
    }
}

/// Handle returned by [`TypePool::insert`].
///
/// Encodes both the column and the position within it. Pass it back
/// to [`TypePool::get`], [`TypePool::get_mut`], or
/// [`TypePool::remove`] to reach the value with no hash lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnKey {
    col: ColumnId,
    key: Key,
}

impl ColumnKey {
    /// A sentinel key that will never refer to a live value.
    pub const PLACEHOLDER: Self = Self {
        col: ColumnId::PLACEHOLDER,
        key: Key::PLACEHOLDER,
    };

    /// Returns the [`ColumnId`] encoded in this key.
    pub fn col_id(self) -> ColumnId {
        self.col
    }
}

mod any_sparse_map {
    use core::any::TypeId;

    use sparse_map::{Key, SparseMap};

    trait Seal {}
    impl<T: 'static> Seal for SparseMap<T> {}

    #[expect(private_bounds)]
    pub trait AnySparseMap: Seal {
        fn element_type_id(&self) -> TypeId;

        /// Removes the value at `key`.
        ///
        /// Returns `true` if an entry was present and removed.
        fn dyn_remove(&mut self, key: &Key) -> bool;
    }

    impl<T: 'static> AnySparseMap for SparseMap<T> {
        fn element_type_id(&self) -> TypeId {
            TypeId::of::<T>()
        }

        fn dyn_remove(&mut self, key: &Key) -> bool {
            self.remove(key).is_some()
        }
    }

    impl dyn AnySparseMap {
        #[inline]
        pub fn element_is<T: 'static>(&self) -> bool {
            self.element_type_id() == TypeId::of::<T>()
        }

        #[inline]
        pub fn downcast_ref<T: 'static>(
            &self,
        ) -> Option<&SparseMap<T>> {
            if self.element_is::<T>() {
                unsafe { Some(self.downcast_unchecked_ref()) }
            } else {
                None
            }
        }

        #[inline]
        pub fn downcast_mut<T: 'static>(
            &mut self,
        ) -> Option<&mut SparseMap<T>> {
            if self.element_is::<T>() {
                unsafe { Some(self.downcast_unchecked_mut()) }
            } else {
                None
            }
        }

        /// # Safety
        ///
        /// Calling this with the wrong type is *undefined behavior*.
        #[inline]
        pub unsafe fn downcast_unchecked_ref<T: 'static>(
            &self,
        ) -> &SparseMap<T> {
            debug_assert!(self.element_is::<T>());
            unsafe { &*(self as *const Self as *const SparseMap<T>) }
        }

        /// # Safety
        ///
        /// Calling this with the wrong type is *undefined behavior*.
        #[inline]
        pub unsafe fn downcast_unchecked_mut<T: 'static>(
            &mut self,
        ) -> &mut SparseMap<T> {
            debug_assert!(self.element_is::<T>());
            unsafe { &mut *(self as *mut Self as *mut SparseMap<T>) }
        }
    }
}

type DynSparseMap = Box<dyn any_sparse_map::AnySparseMap>;

/// Heterogeneous append-only store.
///
/// Each call to [`TypePool::insert`] allocates a new slot and
/// returns a [`ColumnKey`] encoding the column and position.
/// Subsequent lookups use the key directly with no hash-map
/// overhead.
///
/// ## Mental model
///
/// | [`ColumnKey`] | `f32` col | `String` col |
/// |---------------|-----------|--------------|
/// | {col:0,..}    | 3.14      | -            |
/// | {col:1,..}    | -         | "hello"      |
/// | {col:0,..}    | 2.71      | -            |
pub struct TypePool {
    column_ids: HashMap<TypeId, ColumnId>,
    columns: Vec<DynSparseMap>,
}

impl TypePool {
    /// Creates an empty [`TypePool`].
    pub fn new() -> Self {
        Self {
            column_ids: HashMap::new(),
            columns: Vec::new(),
        }
    }

    pub fn ensure_column<T: 'static>(&mut self) -> ColumnId {
        match self.column_ids.entry(TypeId::of::<T>()) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let col = ColumnId(self.columns.len());
                e.insert(col);
                self.columns.push(Box::new(SparseMap::<T>::new()));
                col
            }
        }
    }

    /// Inserts `value` and returns a [`ColumnKey`] identifying it.
    pub fn insert<T: 'static>(&mut self, value: T) -> ColumnKey {
        self.insert_with_key(
            #[inline(always)]
            |_| value,
        )
    }

    /// Like [`Self::insert`], but calls `create` with the
    /// [`ColumnKey`] before the value is placed in storage.
    pub fn insert_with_key<T: 'static>(
        &mut self,
        create: impl FnOnce(ColumnKey) -> T,
    ) -> ColumnKey {
        let col = self.ensure_column::<T>();
        // SAFETY: `col` was just assigned for T by ensure_column.
        let column = unsafe {
            self.columns[col.index()].downcast_unchecked_mut::<T>()
        };
        let key = column
            .insert_with_key(|_, key| create(ColumnKey { col, key }));
        ColumnKey { col, key }
    }

    /// Returns a reference to the value at `col_key`, or `None`
    /// if it has been removed.
    pub fn get<T: 'static>(&self, col_key: &ColumnKey) -> Option<&T> {
        self.columns
            .get(col_key.col.index())?
            .downcast_ref::<T>()?
            .get(&col_key.key)
    }

    /// Returns a mutable reference to the value at `col_key`,
    /// or `None` if it has been removed.
    pub fn get_mut<T: 'static>(
        &mut self,
        col_key: &ColumnKey,
    ) -> Option<&mut T> {
        self.columns
            .get_mut(col_key.col.index())?
            .downcast_mut::<T>()?
            .get_mut(&col_key.key)
    }

    /// Removes and returns the value at `col_key`, or `None` if
    /// already removed.
    pub fn remove<T: 'static>(
        &mut self,
        col_key: &ColumnKey,
    ) -> Option<T> {
        self.columns
            .get_mut(col_key.col.index())?
            .downcast_mut::<T>()?
            .remove(&col_key.key)
    }

    /// Provides `(&mut T, &mut TypePool)` simultaneously.
    ///
    /// The value at `col_key` is temporarily taken out of its slot
    /// for the duration of `f`, then restored to the same slot.
    /// The [`ColumnKey`] remains valid after the call.
    ///
    /// Returns `None` if `col_key` is absent.
    pub fn scope<T: 'static, R>(
        &mut self,
        col_key: &ColumnKey,
        f: impl FnOnce(&mut T, &mut Self) -> R,
    ) -> Option<R> {
        let mut value = {
            self.columns
                .get_mut(col_key.col.index())?
                .downcast_mut::<T>()?
                .take(&col_key.key)?
        };
        let result = f(&mut value, self);
        if let Some(col) = self
            .columns
            .get_mut(col_key.col.index())
            .and_then(|c| c.downcast_mut::<T>())
        {
            col.restore(&col_key.key, value);
        }
        Some(result)
    }

    /// Removes the value at `col_key` without knowing its type.
    ///
    /// Returns `true` if an entry was present and removed.
    pub fn dyn_remove(&mut self, col_key: &ColumnKey) -> bool {
        match self.columns.get_mut(col_key.col.index()) {
            Some(col) => col.dyn_remove(&col_key.key),
            None => false,
        }
    }
}

impl Default for TypePool {
    fn default() -> Self {
        Self::new()
    }
}
