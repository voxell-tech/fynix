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

    pub fn get<R: Any>(&self) -> Option<&R> {
        let type_id = TypeId::of::<R>();
        self.map
            .get(&type_id)
            .map(|r| r.downcast_ref().expect("Type mismatch!"))
    }

    pub fn get_mut<R: Any>(&mut self) -> Option<&mut R> {
        let type_id = TypeId::of::<R>();
        self.map
            .get_mut(&type_id)
            .map(|r| r.downcast_mut().expect("Type mismatch!"))
    }

    pub fn insert<R: Any>(&mut self, resource: R) -> Option<R> {
        self.map
            .insert(TypeId::of::<R>(), Box::new(resource))
            .map(|r| *r.downcast().expect("Type mismatch!"))
    }

    pub fn remove<R: Any>(&mut self) -> Option<R> {
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

    pub fn contains<R: Any>(&self) -> bool {
        let type_id = TypeId::of::<R>();
        self.contains_type(&type_id)
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}
