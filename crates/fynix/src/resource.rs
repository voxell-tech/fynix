use core::any::{Any, TypeId};

use alloc::boxed::Box;
use hashbrown::HashMap;

// TODO(nixon): Use `TypeSlot` for this?
//
// Implication:
// A derive is needed for all resource that needs to be registable.

pub struct Resources {
    map: HashMap<TypeId, Box<dyn Any>>,
}

impl Resources {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    // TODO(nixon): Add insert_or, insert_or_default, scope

    pub fn get<R: 'static>(&self) -> Option<&R> {
        let type_id = TypeId::of::<R>();
        self.map
            .get(&type_id)
            .map(|r| r.downcast_ref().expect("Type mismatch!"))
    }

    pub fn get_mut<R: 'static>(&mut self) -> Option<&mut R> {
        let type_id = TypeId::of::<R>();
        self.map
            .get_mut(&type_id)
            .map(|r| r.downcast_mut().expect("Type mismatch!"))
    }

    pub fn insert<R: 'static>(&mut self, resource: R) -> Option<R> {
        self.map
            .insert(TypeId::of::<R>(), Box::new(resource))
            .map(|r| *r.downcast().expect("Type mismatch!"))
    }

    pub fn remove<R: 'static>(&mut self) -> Option<R> {
        let type_id = TypeId::of::<R>();
        self.map
            .remove(&type_id)
            .map(|r| *r.downcast().expect("Type mismatch!"))
    }

    pub fn remove_dyn(&mut self, type_id: &TypeId) -> bool {
        self.map.remove(type_id).is_some()
    }

    pub fn contains_type(&self, type_id: &TypeId) -> bool {
        self.map.contains_key(type_id)
    }

    pub fn contains<R: 'static>(&self) -> bool {
        let type_id = TypeId::of::<R>();
        self.contains_type(&type_id)
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut r = Resources::new();
        r.insert(42u32);
        assert_eq!(r.get::<u32>(), Some(&42));
    }

    #[test]
    fn get_returns_none_when_absent() {
        let r = Resources::new();
        assert_eq!(r.get::<u32>(), None);
    }

    #[test]
    fn insert_returns_previous_value() {
        let mut r = Resources::new();
        assert_eq!(r.insert(1u32), None);
        assert_eq!(r.insert(2u32), Some(1));
        assert_eq!(r.get::<u32>(), Some(&2));
    }

    #[test]
    fn get_mut_allows_mutation() {
        let mut r = Resources::new();
        r.insert(10u32);
        *r.get_mut::<u32>().unwrap() += 5;
        assert_eq!(r.get::<u32>(), Some(&15));
    }

    #[test]
    fn get_mut_returns_none_when_absent() {
        let mut r = Resources::new();
        assert!(r.get_mut::<u32>().is_none());
    }

    #[test]
    fn remove_returns_value_and_empties() {
        let mut r = Resources::new();
        r.insert(99u32);
        assert_eq!(r.remove::<u32>(), Some(99));
        assert_eq!(r.get::<u32>(), None);
    }

    #[test]
    fn remove_returns_none_when_absent() {
        let mut r = Resources::new();
        assert_eq!(r.remove::<u32>(), None);
    }

    #[test]
    fn remove_dyn_returns_true_and_false() {
        let mut r = Resources::new();
        r.insert(1u32);
        let tid = TypeId::of::<u32>();
        assert!(r.remove_dyn(&tid));
        assert!(!r.remove_dyn(&tid));
    }

    #[test]
    fn contains_and_contains_type() {
        let mut r = Resources::new();
        assert!(!r.contains::<u32>());
        r.insert(0u32);
        assert!(r.contains::<u32>());
        assert!(r.contains_type(&TypeId::of::<u32>()));
        r.remove::<u32>();
        assert!(!r.contains::<u32>());
    }

    #[test]
    fn different_types_are_independent() {
        let mut r = Resources::new();
        r.insert(1u32);
        r.insert(2u64);
        assert_eq!(r.get::<u32>(), Some(&1));
        assert_eq!(r.get::<u64>(), Some(&2));
        r.remove::<u32>();
        assert_eq!(r.get::<u32>(), None);
        assert_eq!(r.get::<u64>(), Some(&2));
    }

    #[test]
    fn default_is_empty() {
        let r = Resources::default();
        assert!(!r.contains::<u32>());
    }
}
