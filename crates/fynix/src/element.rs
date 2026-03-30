use core::any::TypeId;

use rectree::{Constraint, Layouter, Size};

use crate::element::meta::{
    ElementMeta, ElementMetas, ElementTypeMetas,
};
use crate::id::{GenId, IdGenerator};
use crate::type_table::TypeTable;

mod meta;

/// Type-erased storage for all element instances.
///
/// Internally holds one [`TypeTable`] slot per element type. A
/// parallel [`HashMap`] tracks the concrete type of each
/// [`ElementId`] so that polymorphic access (via [`Self::get`]) and
/// removal work without knowing the type at the call site.
pub struct Elements {
    id_generator: ElementIdGenerator,
    elements: TypeTable<ElementId>,
    metas: ElementMetas,
    type_metas: ElementTypeMetas,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            id_generator: IdGenerator::new(),
            elements: TypeTable::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
        }
    }

    /// Stores `element`, registers its type getter if needed, and
    /// returns a fresh [`ElementId`].
    pub fn add<E: Element>(&mut self, element: E) -> ElementId {
        let type_id = TypeId::of::<E>();

        self.type_metas.register::<E>();

        let id = self.id_generator.new_id();

        self.metas.init_element(id, type_id);
        self.elements.insert(id, element);
        id
    }

    /// Returns a type-erased reference to the element.
    ///
    /// Prefer [`get_typed`](Elements::get_typed) when the concrete
    /// type is known, it avoids the getter dispatch.
    pub fn get_dyn(&self, id: &ElementId) -> Option<&dyn Element> {
        if let Some(ElementMeta { type_id, .. }) = self.metas.get(id)
            && let Some(type_meta) = self.type_metas.get(type_id)
        {
            return type_meta.get_dyn(id, &self.elements);
        }

        None
    }

    /// Returns a typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not hold a value
    /// of type `E`.
    pub fn get_typed<E: Element>(
        &self,
        id: &ElementId,
    ) -> Option<&E> {
        self.elements.get::<E>(id)
    }

    /// Removes the element and recycles its [`ElementId`].
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(&mut self, id: &ElementId) -> bool {
        self.metas.remove(id);
        if let Some(ElementMeta { type_id, .. }) =
            self.metas.remove(id)
            && self.elements.dyn_remove(&type_id, id)
        {
            self.id_generator.recycle(*id);
            return true;
        }

        false
    }
}

impl Default for Elements {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker trait for widget types.
///
/// Implement this for any type you want to add to the element tree
/// via [`BuildCtx::add`](crate::ctx::BuildCtx::add). The single
/// required method, `new`, must return a default (unstyled) instance.
/// Styles are applied immediately after construction by the build
/// context.
pub trait Element: 'static {
    fn new() -> Self
    where
        Self: Sized;

    fn children(&self) -> impl IntoIterator<Item = &ElementId>
    where
        Self: Sized,
    {
        []
    }

    fn constrain(&self, parent_constraint: Constraint) -> Constraint {
        parent_constraint
    }

    #[expect(unused_variables)]
    fn build(
        &self,
        constraint: Constraint,
        layouter: &mut impl Layouter<Id = ElementId>,
    ) -> Size
    where
        Self: Sized,
    {
        constraint.min
    }
}

/// Function pointer for returning `&dyn Element` from a [`TypeTable`]
/// without knowing the concrete type at the call site.
///
/// One monomorphized instance is registered per element type on first
/// insertion.
pub type GetDynElementFn = for<'a> fn(
    table: &'a TypeTable<ElementId>,
    id: &ElementId,
) -> Option<&'a dyn Element>;

/// Monomorphized implementation of [`GetDynElementFn`] for element
/// type `E`.
#[inline]
pub fn get_dyn_element<'a, E: Element>(
    table: &'a TypeTable<ElementId>,
    id: &ElementId,
) -> Option<&'a dyn Element> {
    let element = table.get::<E>(id);
    element.map(|e| e as &dyn Element)
}

/// Generational ID for element instances.
pub type ElementId = GenId<_ElementMarker>;
pub type ElementIdGenerator = IdGenerator<_ElementMarker>;

#[doc(hidden)]
pub struct _ElementMarker;
