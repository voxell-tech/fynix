use core::any::TypeId;

use field_path::accessor::UntypedAccessor;
use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use hashbrown::{HashMap, HashSet};

use crate::style::{
    SetStyle, Stylable, Style, StyleBuilder, StyleId,
    StyleIdGenerator, StyleValue, StyleValueId, UntypedSetStyle,
};
use crate::type_table::TypeTable;

/// Central style manager.
///
/// Maintains the registry of field setters, the stored style values,
/// and the committed chain of [`Style`] nodes.
pub struct Styles {
    /// Maps each field to its accessor and type-erased setter,
    /// registered once per `(S, T)` pair on the first
    /// [`set`](Styles::set) call.
    registry:
        HashMap<UntypedField, (UntypedAccessor, UntypedSetStyle)>,
    /// Stores the actual style values keyed by [`StyleValueId`].
    pub style_values: TypeTable<StyleValueId>,
    /// Committed style nodes, each forming a singly-linked
    /// inheritance chain via their `parent_id`.
    pub styles: HashMap<StyleId, Style>,
    /// Accumulates field changes for the current (open) style node
    /// until the next [`commit_styles`](Styles::commit_styles) call.
    style_builder: StyleBuilder,
    /// The ID of the open style node currently being built.
    current_id: StyleId,
    id_generator: StyleIdGenerator,
}

impl Styles {
    pub fn new() -> Self {
        let mut id_generator = StyleIdGenerator::new();

        Self {
            registry: HashMap::new(),
            style_values: TypeTable::new(),
            styles: HashMap::new(),
            style_builder: StyleBuilder::new(),
            current_id: id_generator.new_id(),
            id_generator,
        }
    }

    /// Returns the ID of the currently open (uncommitted) style node.
    pub fn current_id(&self) -> StyleId {
        self.current_id
    }

    /// Returns `true` when there are pending field changes that need
    /// to be committed before the next element is created.
    pub fn should_commit(&self) -> bool {
        !self.style_builder.is_empty()
    }

    /// Discards any pending (uncommitted) field changes.
    pub fn clear_builder(&mut self) {
        self.style_builder.clear();
    }

    /// Flushes pending field changes into a new committed [`Style`]
    /// node and advances to a fresh [`StyleId`].
    ///
    /// `parent_id` links the new node into the inheritance chain so
    /// that [`apply`](Styles::apply) can walk up to ancestor defaults.
    ///
    /// `is_nested` controls which child slot on the parent is used:
    /// `true` sets `nested_child` (one scope deeper), `false` sets
    /// `adjacent_child` (same scope, next sibling).
    pub fn commit_styles(
        &mut self,
        parent_id: Option<StyleId>,
        is_nested: bool,
    ) {
        let committed_id = self.current_id;
        let style =
            core::mem::take(&mut self.style_builder).build(parent_id);

        self.styles.insert(committed_id, style);

        if let Some(parent) = parent_id {
            self.add_child_to_style(parent, committed_id, is_nested);
        }

        self.current_id = self.id_generator.new_id();
    }

    fn add_child_to_style(
        &mut self,
        parent_id: StyleId,
        child_id: StyleId,
        is_nested: bool,
    ) {
        let Some(parent) = self.styles.get_mut(&parent_id) else {
            return;
        };

        if is_nested {
            debug_assert!(parent.nested_child.is_none());
            parent.nested_child = Some(child_id);
        } else {
            debug_assert!(parent.adjacent_child.is_none());
            parent.adjacent_child = Some(child_id);
        }
    }

    /// Queues a style default: field `field_accessor` on source type
    /// `S` will be set to `value` for all targets created under the
    /// current style scope.
    ///
    /// The setter is registered in the registry on the first call for
    /// a given field; subsequent calls only update the stored value.
    pub fn set<S: Stylable, T: StyleValue>(
        &mut self,
        field_accessor: FieldAccessor<S, T>,
        value: T,
    ) {
        let untyped_field = field_accessor.field.untyped();
        let type_id = TypeId::of::<S>();

        if !self.registry.contains_key(&untyped_field) {
            self.registry.insert(
                untyped_field,
                (
                    field_accessor.accessor.untyped(),
                    SetStyle::<S>::new::<T>().untyped(),
                ),
            );
        }

        self.style_values.insert(
            StyleValueId::new(self.current_id, untyped_field),
            value,
        );
        self.style_builder.insert(type_id, untyped_field);
    }

    /// Recursively removes the style node and all its descendants.
    ///
    /// Returns `true` if the node was present and removed.
    pub fn remove(&mut self, id: &StyleId) -> bool {
        let Some(style) = self.styles.remove(id) else {
            return false;
        };
        self.id_generator.recycle(*id);

        for c in style.children().into_iter().flatten() {
            self.remove(c);
        }

        true
    }

    /// Applies the style chain rooted at `id` to `source`.
    ///
    /// Walks the parent chain from leaf to root. The first value
    /// encountered for each field wins (leaf takes precedence).
    pub fn apply<S: Stylable>(&self, source: &mut S, id: &StyleId) {
        let type_id = TypeId::of::<S>();
        let mut applied = HashSet::new();

        // Walk leaf-to-root; the first value seen for a field wins.
        let mut current = Some(*id);
        while let Some(id) = current {
            let Some(style) = self.styles.get(&id) else {
                break;
            };
            current = style.parent_id;

            let Some(fields) = style.get_fields(&type_id) else {
                continue;
            };

            for field in fields {
                if applied.contains(field) {
                    continue;
                }

                if let Some((accessor, untyped_set)) =
                    self.registry.get(field)
                    && let Some(set_style) = untyped_set.typed::<S>()
                {
                    let key = StyleValueId::new(id, *field);
                    set_style.apply(
                        source,
                        accessor,
                        &key,
                        &self.style_values,
                    );
                    applied.insert(*field);
                }
            }
        }
    }
}

impl Default for Styles {
    fn default() -> Self {
        Self::new()
    }
}
