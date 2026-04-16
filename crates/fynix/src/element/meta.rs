use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::record::Scene;
use rectree::RectNode;
use typeslot::{SlotGroup, TypeSlot};

use crate::element::ElementTable;
use crate::element::{Element, ElementGroup, ElementId};

/// Per-element metadata stored alongside the layout node.
pub struct ElementMeta {
    pub slot: usize,
    pub node: RectNode<ElementId>,
    pub cached_scene: Option<Scene>,
}

/// Per-element layout node storage, keyed by [`ElementId`].
///
/// Implements [`rectree::RectNodes`] so it can be passed
/// directly to rectree's layout free functions.
pub struct ElementMetas {
    map: HashMap<ElementId, ElementMeta>,
}

impl ElementMetas {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn init_element<E: 'static + TypeSlot<ElementGroup>>(
        &mut self,
        id: ElementId,
    ) {
        self.map.insert(
            id,
            ElementMeta {
                slot: ElementGroup::slot::<E>(),
                node: RectNode::new(None),
                cached_scene: None,
            },
        );
    }

    /// Removes the element meta and returns its slot index
    /// for type-erased element storage cleanup.
    pub fn remove(&mut self, id: &ElementId) -> Option<usize> {
        self.map.remove(id).map(|m| m.slot)
    }

    pub fn get(&self, id: &ElementId) -> Option<&ElementMeta> {
        self.map.get(id)
    }

    pub fn get_mut(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut ElementMeta> {
        self.map.get_mut(id)
    }
}

impl Default for ElementMetas {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry of per-type dispatch tables, one entry per
/// element type.
///
/// Slot-indexed parallel to [`ElementTable`]: the column at
/// index `ElementGroup::slot::<E>()` holds the
/// [`ElementTypeMeta`] for `E`.
pub struct ElementTypeMetas {
    slots: Vec<Option<ElementTypeMeta>>,
}

impl ElementTypeMetas {
    pub fn new() -> Self {
        let mut slots = Vec::new();
        slots.resize_with(ElementGroup::len(), || None);
        Self { slots }
    }

    pub fn register<E: Element + TypeSlot<ElementGroup>>(&mut self) {
        let slot = ElementGroup::slot::<E>();
        if self.slots[slot].is_none() {
            self.slots[slot] = Some(ElementTypeMeta::new::<E>());
        }
    }

    pub fn get(&self, slot: usize) -> Option<&ElementTypeMeta> {
        self.slots.get(slot)?.as_ref()
    }
}

impl Default for ElementTypeMetas {
    fn default() -> Self {
        Self::new()
    }
}

/// Monomorphized function pointers for a single element type.
///
/// Registered once per type via
/// [`ElementTypeMetas::register`]. Each function implements
/// one step of the layout protocol without knowing the
/// concrete type at the call site.
pub struct ElementTypeMeta {
    pub get_dyn_fn: GetDynElementFn,
    pub children_fn: ChildrenElementFn,
}

impl ElementTypeMeta {
    pub fn new<E: Element + TypeSlot<ElementGroup>>() -> Self {
        Self {
            get_dyn_fn: get_dyn_element::<E>,
            children_fn: for_each_child::<E>,
        }
    }

    pub fn get_dyn<'a>(
        &self,
        table: &'a ElementTable,
        id: &ElementId,
    ) -> Option<&'a dyn Element> {
        (self.get_dyn_fn)(table, id)
    }
}

/// Returns `&dyn Element` from the table without knowing the
/// concrete type at the call site.
pub type GetDynElementFn = for<'a> fn(
    table: &'a ElementTable,
    id: &ElementId,
) -> Option<&'a dyn Element>;

/// Monomorphized implementation of [`GetDynElementFn`] for
/// element type `E`.
#[inline]
pub fn get_dyn_element<'a, E: Element + TypeSlot<ElementGroup>>(
    table: &'a ElementTable,
    id: &ElementId,
) -> Option<&'a dyn Element> {
    table.get::<E>(id).map(|e| e as &dyn Element)
}

/// Visits each child of an element by calling `f` for every
/// [`ElementId`] the element yields from
/// [`Element::children`].
///
/// Using a visitor avoids the need to name the concrete
/// iterator type returned by [`Element::children`], which
/// differs per `E` and cannot be expressed in a
/// function-pointer signature.
pub type ChildrenElementFn = fn(
    table: &ElementTable,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
);

#[inline]
pub fn for_each_child<E: Element + TypeSlot<ElementGroup>>(
    table: &ElementTable,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
) {
    if let Some(element) = table.get::<E>(id) {
        for child in element.children() {
            f(child);
        }
    }
}
