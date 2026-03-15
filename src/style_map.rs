use core::any::TypeId;
use core::hash::Hash;

use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use field_path::registry::FieldAccessorRegistry;
use hashbrown::HashMap;
use rectree::layout::DepthNode;

use crate::field_map::{FieldMap, FieldMapBuilder};
use crate::style::{Style, UntypedStyle};
use crate::type_map::TypeTable;

#[derive(Debug, Copy, Clone)]
pub struct StyleId {
    /// The index of the last instantiated element in local context.
    pub local_rank: usize,
    pub depth_node: DepthNode,
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct ValueKey<K> {
    pub key: K,
    pub field: UntypedField,
}

pub struct StyleContext<K> {
    pub registries: FieldAccessorRegistry,
    pub styles: HashMap<UntypedField, UntypedStyle<K>>,
    pub values: TypeTable<ValueKey<K>>,
}

pub struct StyleMap<K> {
    pub context: StyleContext<K>,
    pub field_maps: HashMap<K, FieldMap<TypeId>>,
}

impl<K> StyleMap<K>
where
    K: Clone + Hash + Eq + 'static,
{
    pub fn edit(&mut self, key: K) -> StyleMapBuilder<'_, K> {
        StyleMapBuilder {
            key,
            context: &mut self.context,
            field_map_builder: FieldMapBuilder::new(),
        }
    }

    /// Consumes the builder and commits its staged field mappings to the map.
    pub fn commit(&mut self, builder: StyleMapBuilder<'_, K>) {
        let key = builder.key.clone();
        let compiled = builder.field_map_builder.compile();
        self.field_maps.insert(key, compiled);
    }

    pub fn delete(&mut self, key: &K) {
        let Some(field_map) = self.field_maps.remove(key) else {
            return;
        };

        for span in field_map.index_map.into_values() {
            for field in
                field_map.fields[span.start..span.end].iter().copied()
            {
                let value_key = ValueKey {
                    key: key.clone(),
                    field,
                };

                self.context.values.remove_all(&value_key);
            }
        }
    }

    /// Applies all styles associated with `key` to the `target` object.
    pub fn apply_styles<S: 'static>(&self, key: &K, target: &mut S) {
        // 1. Get the FieldMap for this specific key (e.g., a specific UI Node)
        let Some(field_map) = self.field_maps.get(key) else {
            return;
        };

        // 2. Find the span of fields that apply to type S
        let source_id = TypeId::of::<S>();
        let Some(span) = field_map.index_map.get(&source_id) else {
            return;
        };

        // 3. Iterate through the relevant fields in linear memory
        let fields = &field_map.fields[span.start..span.end];

        for field in fields {
            // Get the applier function for this field
            let Some(untyped_style) = self.context.styles.get(field)
            else {
                continue;
            };

            // Cast the untyped style back to a typed one and execute
            if let Some(style) = untyped_style.typed::<S>() {
                style.apply(
                    target,
                    key,
                    field,
                    &self.context.registries,
                    &self.context.values,
                );
            }
        }
    }
}

pub struct StyleMapBuilder<'a, K> {
    pub key: K,
    pub context: &'a mut StyleContext<K>,
    pub field_map_builder: FieldMapBuilder<TypeId>,
}

impl<'a, K> StyleMapBuilder<'a, K>
where
    K: Clone + Ord + Hash + Eq + 'static,
{
    pub fn add<S, T>(
        &mut self,
        field_accessor: FieldAccessor<S, T>,
        value: T,
    ) where
        S: 'static,
        T: Clone + 'static,
    {
        let untyped_field = field_accessor.field.untyped();
        let source_id = TypeId::of::<S>();

        // 1. Ensure global style applier exists
        if !self.context.styles.contains_key(&untyped_field) {
            let style = Style::<K, S>::new::<T>().untyped();
            self.context.styles.insert(untyped_field, style);
        }

        // 2. Store the value in the TypeTable
        let value_key = ValueKey {
            key: self.key.clone(),
            field: untyped_field,
        };
        self.context.values.insert(value_key, value);

        // 3. Update registries and local builder state
        self.context.registries.register_field(field_accessor);
        self.field_map_builder.insert(source_id, untyped_field);
    }

    pub fn remove<S, T>(
        &mut self,
        field_accessor: &FieldAccessor<S, T>,
    ) where
        S: 'static,
        T: 'static,
    {
        let untyped_field = field_accessor.field.untyped();
        let source_id = TypeId::of::<S>();

        // 1. Stage removal from the field map
        self.field_map_builder.remove(&source_id, &untyped_field);

        // 2. Actually purge the value from the TypeTable
        let value_key = ValueKey {
            key: self.key.clone(),
            field: untyped_field,
        };
        self.context.values.remove::<T>(&value_key);
    }
}
