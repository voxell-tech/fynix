use field_path::field::UntypedField;
use field_path::registry::FieldAccessorRegistry;
use hashbrown::HashMap;

use crate::setter::UntypedSetter;
use crate::style::StyleId;

/// Global registries for field manipulation.
pub struct FieldRegistries {
    // TODO: Remove this from `field_path`.
    pub accessors: FieldAccessorRegistry,
    pub style_setters: HashMap<UntypedField, UntypedSetter<StyleId>>,
}

impl FieldRegistries {
    pub fn new() -> Self {
        Self {
            accessors: FieldAccessorRegistry::default(),
            style_setters: HashMap::new(),
        }
    }
}

impl Default for FieldRegistries {
    fn default() -> Self {
        Self::new()
    }
}
