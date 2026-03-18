use field_path::registry::FieldAccessorRegistry;
use hashbrown::HashMap;

use crate::rule_set::FieldRuleRegistry;

pub struct FieldRegistries {
    pub accessors: FieldAccessorRegistry,
    pub rules: FieldRuleRegistry,
}

impl FieldRegistries {
    pub fn new() -> Self {
        Self {
            accessors: FieldAccessorRegistry::default(),
            rules: HashMap::new(),
        }
    }
}

impl Default for FieldRegistries {
    fn default() -> Self {
        Self::new()
    }
}
