use core::hash::Hash;

use alloc::boxed::Box;
use alloc::vec::Vec;
use field_path::field::UntypedField;
use hashbrown::{HashMap, HashSet};

pub struct FieldMap<K> {
    pub index_map: HashMap<K, Span>,
    pub fields: Box<[UntypedField]>,
}

pub struct FieldMapBuilder<K> {
    pub field_map: HashMap<K, HashSet<UntypedField>>,
}

impl<K> FieldMapBuilder<K> {
    pub fn new() -> Self {
        Self {
            field_map: HashMap::new(),
        }
    }
}

impl<K> FieldMapBuilder<K>
where
    K: Hash + Eq,
{
    pub fn insert(&mut self, key: K, field: UntypedField) {
        self.field_map.entry(key).or_default().insert(field);
    }

    pub fn remove(&mut self, key: &K, field: &UntypedField) {
        if let Some(fields) = self.field_map.get_mut(key) {
            fields.remove(field);
        }
    }

    pub fn compile(self) -> FieldMap<K> {
        let mut index_map = HashMap::new();
        let mut all_fields = Vec::new();

        for (key, fields) in self.field_map.into_iter() {
            let start = all_fields.len();
            all_fields.extend(fields);
            let end = all_fields.len();
            index_map.insert(key, Span { start, end });
        }

        FieldMap {
            index_map,
            fields: all_fields.into_boxed_slice(),
        }
    }
}

impl<K> Default for FieldMapBuilder<K> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
