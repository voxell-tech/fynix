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
            field_indices: &mut self.field_indices,
            field_index_builder: FieldIndexBuilder::new(),
        }
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
    pub field_indices: &'a mut HashMap<K, FieldIndex<TypeId>>,
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

    /// Compiles the staged field index and inserts it into the rule set.
    pub fn commit(self) {
        let compiled = self.field_index_builder.compile();
        self.field_indices.insert(self.key, compiled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hashbrown::HashMap;

    use crate::type_table::TypeTable;

    #[derive(Default, Debug, PartialEq, Clone)]
    struct Frame {
        width: f32,
        height: f32,
        opacity: f32,
    }

    #[derive(Default, Debug, PartialEq, Clone)]
    struct Label {
        font_size: f32,
        bold: bool,
    }

    fn make_registries<K>() -> FieldRegistries<K> {
        FieldRegistries {
            accessors: FieldAccessorRegistry::default(),
            setters: HashMap::new(),
        }
    }

    fn make_rule_set<K: Hash + Eq>() -> RuleSet<K> {
        RuleSet {
            values: TypeTable::new(),
            field_indices: HashMap::new(),
        }
    }

    // --- Basic application ---

    #[test]
    fn single_rule_applied_to_widget() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 200.0f32);
        b.commit();

        let mut frame = Frame::default();
        rs.apply_styles(&1u32, &mut frame, &reg);
        assert_eq!(frame.width, 200.0);
    }

    #[test]
    fn multiple_fields_on_same_node_all_applied() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 300.0f32);
        b.add(field_path::field_accessor!(<Frame>::height), 150.0f32);
        b.add(field_path::field_accessor!(<Frame>::opacity), 0.8f32);
        b.commit();

        let mut frame = Frame::default();
        rs.apply_styles(&1u32, &mut frame, &reg);
        assert_eq!(frame.width, 300.0);
        assert_eq!(frame.height, 150.0);
        assert_eq!(frame.opacity, 0.8);
    }

    // --- Multiple widget types ---

    #[test]
    fn same_node_can_hold_rules_for_different_widget_types() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 100.0f32);
        b.add(field_path::field_accessor!(<Label>::font_size), 16.0f32);
        b.commit();

        let mut frame = Frame::default();
        rs.apply_styles(&1u32, &mut frame, &reg);
        assert_eq!(frame.width, 100.0);
        assert_eq!(frame.height, 0.0); // untouched

        let mut label = Label::default();
        rs.apply_styles(&1u32, &mut label, &reg);
        assert_eq!(label.font_size, 16.0);
    }

    #[test]
    fn applying_frame_rules_does_not_affect_label_fields() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 500.0f32);
        b.commit();

        // Label has no rules under this key — should remain default.
        let mut label = Label { font_size: 12.0, bold: false };
        rs.apply_styles(&1u32, &mut label, &reg);
        assert_eq!(label.font_size, 12.0);
    }

    // --- Isolation between nodes ---

    #[test]
    fn independent_nodes_do_not_share_values() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 100.0f32);
        b.commit();

        let mut b = rs.edit(2u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 200.0f32);
        b.commit();

        let mut frame1 = Frame::default();
        rs.apply_styles(&1u32, &mut frame1, &reg);

        let mut frame2 = Frame::default();
        rs.apply_styles(&2u32, &mut frame2, &reg);

        assert_eq!(frame1.width, 100.0);
        assert_eq!(frame2.width, 200.0);
    }

    #[test]
    fn apply_styles_on_unknown_key_is_no_op() {
        let reg = make_registries::<u32>();
        let rs = make_rule_set::<u32>();

        let mut frame = Frame { width: 50.0, height: 50.0, opacity: 1.0 };
        rs.apply_styles(&999u32, &mut frame, &reg);

        assert_eq!(frame.width, 50.0);
        assert_eq!(frame.height, 50.0);
    }

    // --- Override & re-edit ---

    #[test]
    fn re_editing_a_field_overwrites_its_value() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 100.0f32);
        b.commit();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 250.0f32);
        b.commit();

        let mut frame = Frame::default();
        rs.apply_styles(&1u32, &mut frame, &reg);
        assert_eq!(frame.width, 250.0);
    }

    // --- Delete ---

    #[test]
    fn delete_removes_key_so_apply_becomes_no_op() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 200.0f32);
        b.commit();

        rs.delete(&1u32);

        let mut frame = Frame::default();
        rs.apply_styles(&1u32, &mut frame, &reg);
        assert_eq!(frame.width, 0.0); // default, rule no longer applied
    }

    #[test]
    fn delete_on_unknown_key_does_not_panic() {
        let mut rs = make_rule_set::<u32>();
        rs.delete(&42u32);
    }

    // --- Builder remove ---

    #[test]
    fn removing_a_field_in_builder_excludes_it_from_apply() {
        let mut reg = make_registries::<u32>();
        let mut rs = make_rule_set::<u32>();

        let mut b = rs.edit(1u32, &mut reg);
        b.add(field_path::field_accessor!(<Frame>::width), 100.0f32);
        b.add(field_path::field_accessor!(<Frame>::height), 80.0f32);
        b.remove(&field_path::field_accessor!(<Frame>::height));
        b.commit();

        let mut frame = Frame::default();
        rs.apply_styles(&1u32, &mut frame, &reg);
        assert_eq!(frame.width, 100.0);
        assert_eq!(frame.height, 0.0); // was removed from the rule
    }
}
