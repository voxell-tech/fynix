use core::any::TypeId;

use hashbrown::HashMap;

use crate::element::meta::{ElementMetas, ElementTypeMetas};
use crate::id::{GenId, IdGenerator};
use crate::layout::{Constraint, Layouter, Size};
use crate::type_table::TypeTable;

mod meta;

/// Type-erased storage for all element instances.
///
/// Internally holds one [`TypeTable`] slot per element type. A
/// parallel [`HashMap`] tracks the concrete type of each
/// [`ElementId`] so that polymorphic access (via [`Self::get`]) and
/// removal work without knowing the type at the call site.
pub struct Elements {
    elements: TypeTable<ElementId>,
    // TODO(nixon): Move `TypeId` info into `ElementId`?
    element_types: HashMap<ElementId, TypeId>,
    element_getters: HashMap<TypeId, GetDynElementFn>,
    id_generator: ElementIdGenerator,
    // TODO: Create a new type for each.
    metas: ElementMetas,
    type_metas: ElementTypeMetas,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            elements: TypeTable::new(),
            element_types: HashMap::new(),
            element_getters: HashMap::new(),
            id_generator: IdGenerator::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
        }
    }

    /// Stores `element`, registers its type getter if needed, and
    /// returns a fresh [`ElementId`].
    pub fn add<E: Element>(&mut self, element: E) -> ElementId {
        let type_id = TypeId::of::<E>();

        self.type_metas.register::<E>();
        // if !self.element_getters.contains_key(&type_id) {
        //     self.element_getters
        //         .insert(type_id, get_dyn_element::<E>);
        // }

        let id = self.id_generator.new_id();

        self.metas.init_element(id, type_id);
        // self.element_types.insert(id, type_id);
        self.elements.insert(id, element);
        id
    }

    // /// Returns a type-erased reference to the element.
    // ///
    // /// Prefer [`get_typed`](Elements::get_typed) when the concrete
    // /// type is known, it avoids the getter dispatch.
    // pub fn get_dyn(&self, id: &ElementId) -> Option<&dyn Element> {
    //     if let Some(type_id) = self.element_types.get(id)
    //         && let Some(getter) = self.element_getters.get(type_id)
    //     {
    //         return getter(&self.elements, id);
    //     }

    //     None
    // }

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
        if let Some(type_id) = self.element_types.remove(id)
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
