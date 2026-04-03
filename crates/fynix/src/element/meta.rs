use core::any::TypeId;

use hashbrown::HashMap;
use rectree::{Constraint, RectNode, Size};

use crate::element::{Element, ElementId, ElementNodes};
use crate::type_table::TypeTable;

/// Per-element metadata stored alongside the layout node.
pub struct ElementMeta {
    pub type_id: TypeId,
    pub node: RectNode<ElementId>,
}

/// Per-element layout node storage, keyed by [`ElementId`].
///
/// Implements [`rectree::RectNodes`] so it can be passed directly to
/// rectree's layout free functions.
pub struct ElementMetas {
    map: HashMap<ElementId, ElementMeta>,
}

impl ElementMetas {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn init_element<E: 'static>(&mut self, id: ElementId) {
        self.map.insert(
            id,
            ElementMeta {
                type_id: TypeId::of::<E>(),
                node: RectNode::new(None),
            },
        );
    }

    /// Returns the [`TypeId`] of the element stored at `id`.
    pub fn get_type_id(&self, id: &ElementId) -> Option<TypeId> {
        self.map.get(id).map(|m| m.type_id)
    }

    /// Removes the element meta and returns its [`TypeId`] for
    /// use in type-erased element storage cleanup.
    pub fn remove(&mut self, id: &ElementId) -> Option<TypeId> {
        self.map.remove(id).map(|m| m.type_id)
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

/// Registry of per-type dispatch tables, one entry per element type.
pub struct ElementTypeMetas {
    map: HashMap<TypeId, ElementTypeMeta>,
}

impl ElementTypeMetas {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn register<E: Element>(&mut self) {
        let type_id = TypeId::of::<E>();
        if !self.map.contains_key(&type_id) {
            self.map.insert(type_id, ElementTypeMeta::new::<E>());
        }
    }

    pub fn get(&self, id: &TypeId) -> Option<&ElementTypeMeta> {
        self.map.get(id)
    }
}

impl Default for ElementTypeMetas {
    fn default() -> Self {
        Self::new()
    }
}

/// Monomorphized function pointers for a single element type.
///
/// Registered once per type via [`ElementTypeMetas::register`].
/// Each function implements one step of the layout protocol without
/// knowing the concrete type at the call site.
pub struct ElementTypeMeta {
    pub get_dyn_fn: GetDynElementFn,
    pub children_fn: ChildrenElementFn,
    pub constrain_fn: ConstrainElementFn,
    pub build_fn: BuildElementFn,
}

impl ElementTypeMeta {
    pub fn new<E: Element>() -> Self {
        Self {
            get_dyn_fn: get_dyn_element::<E>,
            children_fn: for_each_child::<E>,
            constrain_fn: constrain_element::<E>,
            build_fn: build_element::<E>,
        }
    }

    pub fn get_dyn<'a>(
        &self,
        table: &'a TypeTable<ElementId>,
        id: &ElementId,
    ) -> Option<&'a dyn Element> {
        (self.get_dyn_fn)(table, id)
    }
}

/// Returns `&dyn Element` from the table without knowing the
/// concrete type at the call site.
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
    table.get::<E>(id).map(|e| e as &dyn Element)
}

/// Visits each child of an element by calling `f` for every
/// [`ElementId`] the element yields from
/// [`Element::children`].
///
/// Using a visitor avoids the need to name the concrete iterator
/// type returned by [`Element::children`], which differs per `E`
/// and cannot be expressed in a function-pointer signature.
pub type ChildrenElementFn = fn(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
);

#[inline]
pub fn for_each_child<E: Element>(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
) {
    if let Some(element) = table.get::<E>(id) {
        for child in element.children() {
            f(child);
        }
    }
}

/// Calls [`Element::constrain`] without knowing the concrete type.
pub type ConstrainElementFn = fn(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    parent: Constraint,
) -> Constraint;

#[inline]
pub fn constrain_element<E: Element>(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    parent: Constraint,
) -> Constraint {
    table
        .get::<E>(id)
        .map(|e| e.constrain(parent))
        .unwrap_or(parent)
}

/// Calls [`Element::build`] without knowing the concrete type.
pub type BuildElementFn = fn(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    constraint: Constraint,
    nodes: &mut ElementNodes,
) -> Size;

#[inline]
pub fn build_element<E: Element>(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    constraint: Constraint,
    nodes: &mut ElementNodes,
) -> Size {
    table
        .get::<E>(id)
        .map(|e| e.build(constraint, nodes))
        .unwrap_or(Size::ZERO)
}
