use core::any::TypeId;

use alloc::vec::Vec;
use hashbrown::HashMap;

use crate::type_table::TypeTable;

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct ElementId {
    id: u32,
    generation: u32,
}

impl ElementId {
    const fn from_raw(id: u32, generation: u32) -> Self {
        Self { id, generation }
    }
}

#[derive(Default)]
pub struct BuildCtx {
    elements: TypeTable<ElementId>,
    element_types: HashMap<ElementId, TypeId>,
    getters: HashMap<TypeId, GetElementFn>,
    next_id: u32,
    unused_ids: Vec<ElementId>,
}

pub type GetElementFn = for<'a> fn(
    table: &'a TypeTable<ElementId>,
    id: &ElementId,
) -> Option<&'a dyn Element>;

impl BuildCtx {
    #[must_use]
    pub fn add<E: Element>(&mut self) -> ElementId {
        let element = E::new();
        self.add_impl(element)
    }

    #[must_use]
    pub fn add_with<E: Element>(
        &mut self,
        f: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId {
        let mut element = E::new();
        f(&mut element, self);
        self.add_impl(element)
    }

    fn add_impl<E: Element>(&mut self, element: E) -> ElementId {
        let type_id = TypeId::of::<E>();

        if !self.getters.contains_key(&type_id) {
            self.getters.insert(type_id, |table, id| {
                let element = table.get::<E>(id);
                element.map(|e| e as &dyn Element)
            });
        }

        let id = self.unused_ids.pop().unwrap_or_else(|| {
            let id = ElementId::from_raw(self.next_id, 0);
            self.next_id += 1;
            id
        });

        self.element_types.insert(id, type_id);
        self.elements.insert(id, element);
        id
    }

    pub fn get(&mut self, id: &ElementId) -> Option<&dyn Element> {
        if let Some(type_id) = self.element_types.get(id)
            && let Some(getter) = self.getters.get(type_id)
        {
            return getter(&self.elements, id);
        }

        None
    }

    pub fn remove(&mut self, id: &ElementId) -> bool {
        if let Some(type_id) = self.element_types.remove(id) {
            return self.elements.dyn_remove(&type_id, id);
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

#[derive(Default, Debug)]
pub struct Frame;

impl Element for Frame {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }
}
