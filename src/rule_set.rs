use core::any::TypeId;
use core::hash::Hash;

use field_path::field_accessor::FieldAccessor;
use field_path::registry::FieldAccessorRegistry;
use hashbrown::HashMap;
use rectree::layout::DepthNode;

use crate::field_index::{FieldIndex, FieldIndexBuilder};
use crate::setter::{FieldSetterRegistry, Setter, ValueId};
use crate::type_table::TypeTable;

#[derive(Debug, Copy, Clone)]
pub struct StyleId {
    /// The index of the last instantiated element in local context.
    pub local_rank: usize,
    pub depth_node: DepthNode,
}

pub struct FieldRegistries<K> {
    pub accessors: FieldAccessorRegistry,
    pub setters: FieldSetterRegistry<K>,
}

pub struct RuleSet<K> {
    pub values: TypeTable<ValueId<K>>,
    pub field_indices: HashMap<K, FieldIndex<TypeId>>,
}

impl<K> RuleSet<K>
where
    K: Clone + Hash + Eq + 'static,
{
    pub fn edit<'a>(
        &'a mut self,
        key: K,
        registries: &'a mut FieldRegistries<K>,
    ) -> RuleSetBuilder<'a, K> {
        RuleSetBuilder {
            key,
            registries,
            values: &mut self.values,
            field_index_builder: FieldIndexBuilder::new(),
        }
    }

    /// Consumes the builder and commits its staged field indices.
    pub fn commit(&mut self, builder: RuleSetBuilder<'_, K>) {
        let key = builder.key.clone();
        let compiled = builder.field_index_builder.compile();
        self.field_indices.insert(key, compiled);
    }

    pub fn delete(&mut self, key: &K) {
        let Some(field_index) = self.field_indices.remove(key) else {
            return;
        };

        for span in field_index.index_map.into_values() {
            for field in field_index.fields[span.start..span.end]
                .iter()
                .copied()
            {
                let value_id = ValueId::new(key.clone(), field);
                self.values.remove_all(&value_id);
            }
        }
    }

    /// Applies all setters associated with `key` to the `target` object.
    pub fn apply_styles<S: 'static>(
        &self,
        key: &K,
        target: &mut S,
        registries: &FieldRegistries<K>,
    ) {
        // 1. Get the FieldIndex for this specific key (e.g., a specific UI Node)
        let Some(field_index) = self.field_indices.get(key) else {
            return;
        };

        // 2. Find the span of fields that apply to type S
        let source_id = TypeId::of::<S>();
        let Some(span) = field_index.index_map.get(&source_id) else {
            return;
        };

        // 3. Iterate through the relevant fields in linear memory
        let fields = &field_index.fields[span.start..span.end];

        for field in fields {
            let Some(untyped_setter) = registries.setters.get(field)
            else {
                continue;
            };

            if let Some(setter) = untyped_setter.typed::<S>() {
                let value_id = ValueId::new(key.clone(), *field);

                setter.apply(
                    target,
                    &value_id,
                    &registries.accessors,
                    &self.values,
                );
            }
        }
    }
}

pub struct RuleSetBuilder<'a, K> {
    pub key: K,
    pub registries: &'a mut FieldRegistries<K>,
    pub values: &'a mut TypeTable<ValueId<K>>,
    pub field_index_builder: FieldIndexBuilder<TypeId>,
}

impl<'a, K> RuleSetBuilder<'a, K>
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

        // 1. Ensure global setter exists
        if !self.registries.setters.contains_key(&untyped_field) {
            let setter = Setter::<K, S>::new::<T>().untyped();
            self.registries.setters.insert(untyped_field, setter);
        }

        // 2. Store the value in the TypeTable
        let value_key = ValueId {
            key: self.key.clone(),
            field: untyped_field,
        };
        self.values.insert(value_key, value);

        // 3. Update registries and local builder state
        self.registries.accessors.register_field(field_accessor);
        self.field_index_builder.insert(source_id, untyped_field);
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

        // 1. Stage removal from the field index
        self.field_index_builder.remove(&source_id, &untyped_field);

        // 2. Actually purge the value from the TypeTable
        let value_key = ValueId {
            key: self.key.clone(),
            field: untyped_field,
        };
        self.values.remove::<T>(&value_key);
    }
}
