use core::any::TypeId;
use core::hash::Hash;

use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use hashbrown::{HashMap, HashSet};
use rectree::NodeId;

use crate::field_index::{FieldIndex, FieldIndexBuilder};
use crate::registry::FieldRegistries;
use crate::setter::{Setter, UntypedSetter, ValueId};
use crate::type_table::TypeTable;

pub type FieldRuleRegistry =
    HashMap<UntypedField, UntypedSetter<RuleId>>;

pub type RuleSetter<S> = Setter<RuleId, S>;

pub type RuleValueId = ValueId<RuleId>;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct RuleId {
    /// The current node's Id.
    pub node_id: NodeId,
    /// Sibling rank within the scope to disambiguates multiple
    /// elements at the same node.
    pub rank: usize,
}

impl RuleId {
    pub const fn new(node_id: NodeId, rank: usize) -> Self {
        Self { node_id, rank }
    }
}

pub struct RuleSet {
    pub values: TypeTable<RuleValueId>,
    pub field_indices: HashMap<RuleId, FieldIndex<TypeId>>,
}

impl RuleSet {
    pub fn new() -> Self {
        Self {
            values: TypeTable::new(),
            field_indices: HashMap::new(),
        }
    }

    pub fn edit<'a>(
        &'a mut self,
        id: RuleId,
        registries: &'a mut FieldRegistries,
    ) -> RuleSetBuilder<'a> {
        RuleSetBuilder {
            id,
            registries,
            values: &mut self.values,
            field_indices: &mut self.field_indices,
            field_index_builder: FieldIndexBuilder::new(),
        }
    }

    /// Like `apply_styles`, but skips fields already present in `visited`.
    /// Inserts each applied field into `visited` so callers can accumulate
    /// the set across multiple scope levels (leaf-first cascade).
    pub fn apply_styles_skipping<S: 'static>(
        &self,
        id: &RuleId,
        target: &mut S,
        registries: &FieldRegistries,
        visited: &mut HashSet<UntypedField>,
    ) {
        let Some(field_index) = self.field_indices.get(id) else {
            return;
        };
        let source_id = TypeId::of::<S>();
        let Some(span) = field_index.index_map.get(&source_id) else {
            return;
        };
        let fields = &field_index.fields[span.start..span.end];
        for &field in fields {
            if !visited.insert(field) {
                continue; // already set by a closer scope
            }
            let Some(untyped_setter) = registries.rules.get(&field)
            else {
                continue;
            };
            if let Some(setter) = untyped_setter.typed::<S>() {
                let value_id = ValueId::new(*id, field);
                setter.apply(
                    target,
                    &value_id,
                    &registries.accessors,
                    &self.values,
                );
            }
        }
    }

    pub fn delete(&mut self, id: &RuleId) {
        let Some(field_index) = self.field_indices.remove(id) else {
            return;
        };

        for span in field_index.index_map.into_values() {
            for field in field_index.fields[span.start..span.end]
                .iter()
                .copied()
            {
                let value_id = ValueId::new(*id, field);
                self.values.remove_all(&value_id);
            }
        }
    }

    /// Applies all setters associated with `key` to the `target` object.
    pub fn apply_styles<S: 'static>(
        &self,
        id: &RuleId,
        target: &mut S,
        registries: &FieldRegistries,
    ) {
        // 1. Get the FieldIndex for this specific key (e.g., a specific UI Node)
        let Some(field_index) = self.field_indices.get(id) else {
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
            let Some(untyped_setter) = registries.rules.get(field)
            else {
                continue;
            };

            if let Some(setter) = untyped_setter.typed::<S>() {
                let value_id = ValueId::new(*id, *field);

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

impl Default for RuleSet {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RuleSetBuilder<'a> {
    pub id: RuleId,
    pub registries: &'a mut FieldRegistries,
    pub values: &'a mut TypeTable<RuleValueId>,
    pub field_indices: &'a mut HashMap<RuleId, FieldIndex<TypeId>>,
    pub field_index_builder: FieldIndexBuilder<TypeId>,
}

impl<'a> RuleSetBuilder<'a> {
    pub fn add<S, T>(
        mut self,
        field_accessor: FieldAccessor<S, T>,
        value: T,
    ) -> Self
    where
        S: 'static,
        T: Clone + 'static,
    {
        let untyped_field = field_accessor.field.untyped();
        let source_id = TypeId::of::<S>();

        // 1. Ensure global setter exists
        if !self.registries.rules.contains_key(&untyped_field) {
            let setter = RuleSetter::<S>::new::<T>().untyped();
            self.registries.rules.insert(untyped_field, setter);
        }

        // 2. Store the value in the TypeTable
        let value_key = ValueId::new(self.id, untyped_field);
        self.values.insert(value_key, value);

        // 3. Update registries and local builder state
        self.registries.accessors.register_field(field_accessor);
        self.field_index_builder.insert(source_id, untyped_field);
        self
    }

    pub fn remove<S, T>(
        mut self,
        field_accessor: &FieldAccessor<S, T>,
    ) -> Self
    where
        S: 'static,
        T: 'static,
    {
        let untyped_field = field_accessor.field.untyped();
        let source_id = TypeId::of::<S>();

        // 1. Stage removal from the field index
        self.field_index_builder.remove(&source_id, &untyped_field);

        // 2. Actually purge the value from the TypeTable
        let value_key = ValueId::new(self.id, untyped_field);
        self.values.remove::<T>(&value_key);
        self
    }

    // /// Compiles the staged field index and inserts it into the rule set.
    // pub(crate) fn commit(self) {
    //     let field_index = self.field_index_builder.compile();
    //     self.field_indices.insert(self.id, field_index);
    // }
}
