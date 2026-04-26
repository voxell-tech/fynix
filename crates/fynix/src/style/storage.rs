use core::any::TypeId;

use field_path::accessor::UntypedAccessor;
use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use hashbrown::{HashMap, HashSet};

use crate::style::{
    SetStyle, Style, StyleBuilder, StyleId, StyleIdGenerator,
    StyleValue, UntypedSetStyle,
};
use crate::type_table::TypeTable;

/// Central style manager.
///
/// Maintains the registry of field setters, the stored style values, and the
/// committed chain of [`Style`] nodes.
///
/// [`set`]: Styles::set
/// [`commit_styles`]: Styles::commit_styles
/// [`apply`]: Styles::apply
pub struct Styles {
    /// Maps each field to its accessor and type-erased setter, registered
    /// once per `(S, T)` pair on the first [`set`](Styles::set) call.
    registry:
        HashMap<UntypedField, (UntypedAccessor, UntypedSetStyle)>,
    /// Stores the actual style values keyed by `(StyleId, T)`.
    pub style_values: TypeTable<StyleId>,
    /// Committed style nodes, each forming a singly-linked
    /// inheritance chain via their `parent_id`.
    pub styles: HashMap<StyleId, Style>,
    /// Accumulates field changes for the *current* (open) style
    /// node until the next [`commit_styles`](Styles::commit_styles)
    /// call.
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

    /// Returns `true` when there are pending field changes that need to be
    /// committed before the next element is created.
    pub fn should_commit(&self) -> bool {
        !self.style_builder.is_empty()
    }

    /// Flushes pending field changes into a new committed [`Style`] node
    /// and advances to a fresh [`StyleId`].
    ///
    /// `parent_id` links the new node into the inheritance chain so that
    /// [`apply`](Styles::apply) can walk up to ancestor defaults.
    pub fn commit_styles(&mut self, parent_id: Option<StyleId>) {
        let style =
            core::mem::take(&mut self.style_builder).build(parent_id);

        self.styles.insert(self.current_id, style);
        self.current_id = self.id_generator.new_id();
    }

    /// Queues a style default: field `field_accessor` on element type `S`
    /// will be set to `value` for all elements created under the current
    /// style scope.
    ///
    /// The setter is registered in the registry on the first call for a
    /// given field; subsequent calls only update the stored value.
    pub fn set<S: 'static, T: StyleValue>(
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

        self.style_values.insert(self.current_id, value);
        self.style_builder.insert(type_id, untyped_field);
    }

    /// Removes a committed style node and recycles its [`StyleId`].
    ///
    /// Returns `true` if the node was present and removed.
    pub fn delete(&mut self, id: &StyleId) -> bool {
        if self.style_values.remove_all(id) {
            self.styles.remove(id);
            self.id_generator.recycle(*id);
            return true;
        }

        false
    }

    /// Applies the style chain rooted at `id` to `element`.
    ///
    /// Walks the parent chain from leaf to root. The first value encountered
    /// for each field wins (leaf takes precedence over ancestors).
    pub fn apply<S: 'static>(&self, source: &mut S, id: &StyleId) {
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
                    set_style.apply(
                        source,
                        accessor,
                        &id,
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
