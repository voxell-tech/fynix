use core::any::TypeId;
use core::hash::Hash;

use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use hashbrown::{HashMap, HashSet};
use rectree::NodeId;

use crate::field_index::{FieldIndex, FieldIndexBuilder};
use crate::registry::FieldRegistries;
use crate::setter::{Setter, ValueId};
use crate::type_table::TypeTable;

pub type StyleSetter<S> = Setter<StyleId, S>;

pub type StyleValueId = ValueId<StyleId>;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct StyleId {
    /// Parent's node id.
    pub parent_id: Option<NodeId>,
    /// Sibling rank within the scope to disambiguates multiple
    /// elements at the same node.
    pub rank: u32,
}

impl StyleId {
    pub const fn new(parent_id: Option<NodeId>, rank: u32) -> Self {
        Self { parent_id, rank }
    }
}

pub struct StyleCtx<'a> {
    style_chain: &'a mut StyleChain,
    registries: &'a mut FieldRegistries,
    current_id: StyleId,
    field_index_builder: FieldIndexBuilder<TypeId>,
}

impl StyleCtx<'_> {
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

        let FieldRegistries {
            accessors,
            style_setters,
        } = self.registries;

        // Register setter and accessor to registries if not exists.
        if !style_setters.contains_key(&untyped_field) {
            let setter = StyleSetter::<S>::new::<T>().untyped();

            style_setters.insert(untyped_field, setter);
            accessors.register_field(field_accessor);
        }

        // Store the value.
        let value_id = ValueId::new(self.current_id, untyped_field);
        self.style_chain.values.insert(value_id, value);

        // Update local builder state.
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

        // Stage removal from the local builder state.
        self.field_index_builder.remove(&source_id, &untyped_field);

        // Removes the value.
        let value_id = ValueId::new(self.current_id, untyped_field);
        self.style_chain.values.remove::<T>(&value_id);
        self
    }

    /// Compiles the staged field index and inserts it into the rule set.
    pub fn commit(&mut self) {
        self.current_id.rank += 1;
        let field_index =
            core::mem::take(&mut self.field_index_builder).compile();
        self.style_chain
            .field_indices
            .insert(self.current_id, field_index);
    }
}

pub struct StyleChain {
    values: TypeTable<StyleValueId>,
    field_indices: HashMap<StyleId, FieldIndex<TypeId>>,
}

impl StyleChain {
    pub fn new() -> Self {
        Self {
            values: TypeTable::new(),
            field_indices: HashMap::new(),
        }
    }

    pub fn edit<'a>(
        &'a mut self,
        id: StyleId,
        registries: &'a mut FieldRegistries,
    ) -> StylesBuilder<'a> {
        StylesBuilder {
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
        id: &StyleId,
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
            let Some(untyped_setter) =
                registries.style_setters.get(&field)
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

    pub fn delete(&mut self, id: &StyleId) {
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
        id: &StyleId,
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
            let Some(untyped_setter) =
                registries.style_setters.get(field)
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

impl Default for StyleChain {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StylesBuilder<'a> {
    pub id: StyleId,
    pub registries: &'a mut FieldRegistries,
    pub values: &'a mut TypeTable<StyleValueId>,
    pub field_indices: &'a mut HashMap<StyleId, FieldIndex<TypeId>>,
    pub field_index_builder: FieldIndexBuilder<TypeId>,
}

impl<'a> StylesBuilder<'a> {
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
        if !self.registries.style_setters.contains_key(&untyped_field)
        {
            let setter = StyleSetter::<S>::new::<T>().untyped();
            self.registries
                .style_setters
                .insert(untyped_field, setter);
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
