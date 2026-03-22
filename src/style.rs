use core::any::TypeId;

use field_path::accessor::UntypedAccessor;
use field_path::field::UntypedField;
use field_path::field_accessor::FieldAccessor;
use hashbrown::HashMap;

use crate::element::Element;
use crate::field_index::{FieldIndex, FieldIndexBuilder};
use crate::id::{GenId, IdGenerator};
use crate::type_table::TypeTable;

pub struct Styles {
    registry:
        HashMap<UntypedField, (UntypedAccessor, UntypedSetStyle)>,
    style_values: TypeTable<StyleId>,
    field_indices: HashMap<StyleId, Style>,
    field_index_builder: FieldIndexBuilder,
    current_id: StyleId,
    id_generator: StyleIdGenerator,
}

impl Styles {
    pub fn new() -> Self {
        let mut id_generator = StyleIdGenerator::new();

        Self {
            registry: HashMap::new(),
            style_values: TypeTable::new(),
            field_indices: HashMap::new(),
            field_index_builder: FieldIndexBuilder::new(),
            current_id: id_generator.new_id(),
            id_generator,
        }
    }

    pub fn current_id(&self) -> StyleId {
        self.current_id
    }

    pub fn should_commit(&self) -> bool {
        !self.field_index_builder.is_empty()
    }

    pub fn commit_styles(&mut self, parent_id: Option<StyleId>) {
        let field_index =
            core::mem::take(&mut self.field_index_builder).compile();

        self.field_indices.insert(
            self.current_id,
            Style::new(parent_id, field_index),
        );

        self.current_id = self.id_generator.new_id();
    }

    pub fn set<E, T>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) where
        E: Element,
        T: Clone + 'static,
    {
        let untyped_field = field_accessor.field.untyped();
        let type_id = TypeId::of::<E>();

        if !self.registry.contains_key(&untyped_field) {
            self.registry.insert(
                untyped_field,
                (
                    field_accessor.accessor.untyped(),
                    SetStyle::<E>::new::<T>().untyped(),
                ),
            );
        }

        self.style_values.insert(self.current_id, value);
        self.field_index_builder.insert(type_id, untyped_field);
    }

    pub fn delete(&mut self, id: &StyleId) -> bool {
        if self.style_values.remove_all(id) {
            self.field_indices.remove(id);
            self.id_generator.recycle(*id);
            return true;
        }

        false
    }

    pub fn apply<E: Element>(&self, element: &mut E, id: &StyleId) {
        // TODO(nixon): Apply style!
    }
}

impl Default for Styles {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Style {
    parent_id: Option<StyleId>,
    field_index: FieldIndex,
}

impl Style {
    pub const fn new(
        parent_id: Option<StyleId>,
        field_index: FieldIndex,
    ) -> Self {
        Self {
            parent_id,
            field_index,
        }
    }
}

// TODO: Fix doc.
/// Function signature for setting a field of source `S` from a
/// [`TypeTable<K>`] via an [`Accessor`].
///
/// ## Returns
/// `true` if set succeeds, else `false`.
///
/// [`Accessor`]: field_path::accessor::Accessor
pub type SetStyleFn<E> = fn(
    &mut E,
    &UntypedAccessor,
    &StyleId,
    &TypeTable<StyleId>,
) -> bool;

/// Implementation of [`SetFieldFn`].
#[inline]
pub fn set_style<E, T>(
    element: &mut E,
    accessor: &UntypedAccessor,
    style_id: &StyleId,
    values: &TypeTable<StyleId>,
) -> bool
where
    E: 'static,
    T: Clone + 'static,
{
    if let Some(accessor) = accessor.typed::<E, T>()
        && let Some(value) = values.get::<T>(style_id)
    {
        *accessor.get_mut(element) = value.clone();
        return true;
    }
    false
}

pub struct SetStyle<E>
where
    E: Element,
{
    set_fn: SetStyleFn<E>,
}

impl<E> SetStyle<E>
where
    E: Element,
{
    pub fn new<T>() -> Self
    where
        T: Clone + 'static,
    {
        Self {
            set_fn: set_style::<E, T>,
        }
    }

    pub fn untyped(&self) -> UntypedSetStyle {
        UntypedSetStyle {
            source_id: TypeId::of::<E>(),
            set_fn: self.set_fn as *const (),
        }
    }

    pub fn apply(
        &self,
        element: &mut E,
        accessor: &UntypedAccessor,
        style_id: &StyleId,
        values: &TypeTable<StyleId>,
    ) -> bool {
        (self.set_fn)(element, accessor, style_id, values)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UntypedSetStyle {
    source_id: TypeId,
    set_fn: *const (),
}

impl UntypedSetStyle {
    pub fn typed<E>(&self) -> Option<SetStyle<E>>
    where
        E: Element,
    {
        if TypeId::of::<E>() == self.source_id {
            return Some(unsafe { self.typed_unchecked() });
        }

        None
    }

    /// ## Safety
    pub const unsafe fn typed_unchecked<E>(&self) -> SetStyle<E>
    where
        E: Element,
    {
        unsafe {
            use core::mem::transmute;
            SetStyle {
                set_fn: transmute::<*const (), SetStyleFn<E>>(
                    self.set_fn,
                ),
            }
        }
    }
}

pub type StyleId = GenId<_StyleMarker>;
pub type StyleIdGenerator = IdGenerator<_StyleMarker>;

pub struct _StyleMarker;
