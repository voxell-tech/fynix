use core::any::TypeId;

use alloc::vec::Vec;
use hashbrown::HashMap;

use crate::{
    id::{GenId, IdGenerator},
    type_table::TypeTable,
};

#[derive(Default)]
pub struct Elements {
    elements: TypeTable<ElementId>,
    // TODO(nixon): Move `TypeId` info into `ElementId`?
    element_types: HashMap<ElementId, TypeId>,
    element_getters: HashMap<TypeId, GetDynElementFn>,
    id_generator: ElementIdGenerator,
}

impl Elements {
    pub fn new() -> Self {
        Self::default()
    }
}

pub type GetDynElementFn = for<'a> fn(
    table: &'a TypeTable<ElementId>,
    id: &ElementId,
) -> Option<&'a dyn Element>;

#[inline]
pub fn get_dyn_element<'a, E: Element>(
    table: &'a TypeTable<ElementId>,
    id: &ElementId,
) -> Option<&'a dyn Element> {
    let element = table.get::<E>(id);
    element.map(|e| e as &dyn Element)
}

impl Elements {
    pub fn add<E: Element>(&mut self, element: E) -> ElementId {
        let type_id = TypeId::of::<E>();

        if !self.element_getters.contains_key(&type_id) {
            self.element_getters
                .insert(type_id, get_dyn_element::<E>);
        }

        let id = self.id_generator.new_id();

        self.element_types.insert(id, type_id);
        self.elements.insert(id, element);
        id
    }

    pub fn get(&mut self, id: &ElementId) -> Option<&dyn Element> {
        if let Some(type_id) = self.element_types.get(id)
            && let Some(getter) = self.element_getters.get(type_id)
        {
            return getter(&self.elements, id);
        }

        None
    }

    pub fn remove(&mut self, id: &ElementId) -> bool {
        if let Some(type_id) = self.element_types.remove(id)
            && self.elements.dyn_remove(&type_id, id)
        {
            self.id_generator.recycle(*id);
            return true;
        }

        false
    }
}

pub trait Element: 'static {
    fn new() -> Self
    where
        Self: Sized;
}

#[derive(Default, Debug)]
pub struct Horizontal {
    children: Vec<ElementId>,
}

impl Horizontal {
    pub fn add(&mut self, id: ElementId) {
        self.children.push(id);
    }
}

impl Element for Horizontal {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self::default()
    }
}

pub type ElementId = GenId<_ElementMarker>;
pub type ElementIdGenerator = IdGenerator<_ElementMarker>;

pub struct _ElementMarker;
