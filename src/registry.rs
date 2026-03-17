use field_path::field::UntypedField;
use field_path::registry::FieldAccessorRegistry;
use hashbrown::HashMap;

use crate::setter::UntypedSetter;

pub type FieldSetterRegistry<K> =
    HashMap<UntypedField, UntypedSetter<K>>;

pub struct FieldRegistries<K> {
    pub accessors: FieldAccessorRegistry,
    pub setters: FieldSetterRegistry<K>,
}

impl<K> FieldRegistries<K> {
    pub fn new() -> Self {
        Self {
            accessors: FieldAccessorRegistry::default(),
            setters: HashMap::new(),
        }
    }
}

impl<K> Default for FieldRegistries<K> {
    fn default() -> Self {
        Self::new()
    }
}
