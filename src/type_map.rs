use core::any::TypeId;
use core::hash::Hash;

use alloc::boxed::Box;
use hashbrown::HashMap;
use sparse_map::{Key, SparseMap};

pub struct TypeMaps<K> {
    type_maps: HashMap<TypeId, Box<dyn AnyTypeMap<K>>>,
}

impl<K> TypeMaps<K> {
    pub fn new() -> Self {
        Self {
            type_maps: HashMap::new(),
        }
    }
}

impl<K> TypeMaps<K>
where
    K: Hash + Eq + 'static,
{
    pub fn insert<T: 'static>(
        &mut self,
        id: K,
        value: T,
    ) -> Option<T> {
        let type_id = TypeId::of::<T>();
        let m = unsafe {
            self.type_maps
                .entry(type_id)
                .or_insert_with(|| Box::new(TypeMap::<K, T>::new()))
                // SAFETY: Type garuanteed on creation.
                .downcast_unchecked_mut()
        };

        m.insert(id, value)
    }

    pub fn get<T: 'static>(&self, id: &K) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        self.type_maps
            .get(&type_id)
            .and_then(|m| m.downcast_ref())
            .and_then(|m| m.get(id))
    }

    pub fn remove<T: 'static>(&mut self, id: &K) -> Option<T> {
        let type_id = TypeId::of::<T>();
        self.type_maps
            .get_mut(&type_id)
            .and_then(|m| m.downcast_mut())
            .and_then(|m| m.remove(id))
    }
}

impl<K> Default for TypeMaps<K> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TypeMap<K, T> {
    values: SparseMap<T>,
    map: HashMap<K, Key>,
}

impl<K, T> TypeMap<K, T> {
    pub fn new() -> Self {
        Self {
            values: SparseMap::new(),
            map: HashMap::new(),
        }
    }
}

impl<K, T> TypeMap<K, T>
where
    K: Hash + Eq,
{
    pub fn insert(&mut self, id: K, value: T) -> Option<T> {
        let mut previous = None;
        if let Some(key) = self.map.get(&id) {
            previous = self.values.remove(key);
        }

        let key = self.values.insert(value);
        self.map.insert(id, key);

        previous
    }

    pub fn get(&self, id: &K) -> Option<&T> {
        self.map.get(id).and_then(|k| self.values.get(k))
    }

    pub fn remove(&mut self, id: &K) -> Option<T> {
        self.map.get(id).and_then(|k| self.values.remove(k))
    }
}

impl<K, T> Default for TypeMap<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

trait AnyTypeMap<K> {
    fn element_type_id(&self) -> TypeId;
}

impl<K, T: 'static> AnyTypeMap<K> for TypeMap<K, T> {
    fn element_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

impl<K> dyn AnyTypeMap<K> {
    #[inline]
    pub fn element_is<T: 'static>(&self) -> bool {
        // Compare both `TypeId`s on equality.
        self.element_type_id() == TypeId::of::<T>()
    }

    #[inline]
    pub fn downcast_ref<T: 'static>(&self) -> Option<&TypeMap<K, T>> {
        if self.element_is::<T>() {
            // SAFETY: Just checked whether we are pointing to the
            // correct type, and we can rely on that check for
            // memory safety because `AnyStyleMap` is only ever
            // privately implemented for `StyleMap<T>`.
            unsafe { Some(self.downcast_unchecked_ref()) }
        } else {
            None
        }
    }

    #[inline]
    pub fn downcast_mut<T: 'static>(
        &mut self,
    ) -> Option<&mut TypeMap<K, T>> {
        if self.element_is::<T>() {
            // SAFETY: Just checked whether we are pointing to the
            // correct type, and we can rely on that check for
            // memory safety because `AnyStyleMap` is only ever
            // privately implemented for `StyleMap<T>`.
            unsafe { Some(self.downcast_unchecked_mut()) }
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// The contained value must be of type [`SparseMap<T>`].
    /// Calling this method with the incorrect type is
    /// *undefined behavior*.
    #[inline]
    pub unsafe fn downcast_unchecked_ref<T: 'static>(
        &self,
    ) -> &TypeMap<K, T> {
        debug_assert!(self.element_is::<T>());
        // SAFETY: caller guarantees that T is the correct type
        unsafe { &*(self as *const Self as *const TypeMap<K, T>) }
    }

    /// # Safety
    ///
    /// The contained value must be of type [`SparseMap<T>`].
    /// Calling this method with the incorrect type is
    /// *undefined behavior*.
    #[inline]
    pub unsafe fn downcast_unchecked_mut<T: 'static>(
        &mut self,
    ) -> &mut TypeMap<K, T> {
        debug_assert!(self.element_is::<T>());
        // SAFETY: caller guarantees that T is the correct type
        unsafe { &mut *(self as *mut Self as *mut TypeMap<K, T>) }
    }
}

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
        let mut maps = TypeMaps::<u32>::new();
        let id = 1;

        maps.insert(id, Velocity(10.5));

        let retrieved = maps.get::<Velocity>(&id);
        assert_eq!(retrieved, Some(&Velocity(10.5)));
    }

    #[test]
    fn test_heterogeneous_storage() {
        let mut maps = TypeMaps::<u32>::new();
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
        let mut maps = TypeMaps::<u32>::new();
        let id = 7;

        maps.insert(id, 100);
        // Inserting again should return the old value
        let old_value = maps.insert(id, 200);

        assert_eq!(old_value, Some(100));
        assert_eq!(maps.get(&id), Some(&200));
    }

    #[test]
    fn test_remove_logic() {
        let mut maps = TypeMaps::<u32>::new();
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
    fn test_type_isolation() {
        let mut maps = TypeMaps::<u32>::new();
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
        let mut maps = TypeMaps::<CustomKey>::new();
        let key = CustomKey(99);

        maps.insert(key.clone(), "Hello World");

        assert_eq!(
            maps.get::<&'static str>(&key),
            Some(&"Hello World")
        );
    }

    #[test]
    fn test_multiple_keys_one_type() {
        let mut maps = TypeMaps::<u32>::new();

        maps.insert(1, Velocity(1.0));
        maps.insert(2, Velocity(2.0));

        assert_eq!(maps.get::<Velocity>(&1), Some(&Velocity(1.0)));
        assert_eq!(maps.get::<Velocity>(&2), Some(&Velocity(2.0)));
    }

    #[test]
    fn test_empty_map_get() {
        let maps: TypeMaps<u32> = TypeMaps::new();
        assert!(maps.get::<Velocity>(&0).is_none());
    }
}
