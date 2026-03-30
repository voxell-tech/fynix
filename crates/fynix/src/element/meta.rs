use core::any::TypeId;

use hashbrown::HashMap;

use crate::element::{Element, ElementId};
use crate::layout::{Constraint, Layouter, Size, Vec2};
use crate::type_table::TypeTable;

#[derive(Debug, Clone)]
pub struct ElementMetas {
    map: HashMap<ElementId, ElementMeta>,
}

impl ElementMetas {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn remove(&mut self, id: &ElementId) -> Option<ElementMeta> {
        self.map.remove(id)
    }

    pub fn init_element(&mut self, id: ElementId, type_id: TypeId) {
        self.map.insert(id, ElementMeta::init(type_id));
    }

    pub fn set_parent(
        &mut self,
        id: &ElementId,
        parent: ElementId,
    ) -> bool {
        if let Some(meta) = self.get_mut(id) {
            meta.parent = Some(parent);
            return true;
        }

        false
    }

    pub fn set_position(
        &mut self,
        id: &ElementId,
        position: Vec2,
    ) -> bool {
        if let Some(meta) = self.get_mut(id) {
            meta.position = position;
            return true;
        }

        false
    }

    pub fn set_size(&mut self, id: &ElementId, size: Size) -> bool {
        if let Some(meta) = self.get_mut(id) {
            meta.size = size;
            return true;
        }

        false
    }

    pub fn set_constraint(
        &mut self,
        id: &ElementId,
        constraint: Constraint,
    ) -> bool {
        if let Some(meta) = self.get_mut(id) {
            meta.constraint = constraint;
            return true;
        }

        false
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

impl Layouter for ElementMetas {
    type Id = ElementId;

    fn get_size(&self, id: &Self::Id) -> Size {
        self.get(id).map(|m| m.size).unwrap_or_default()
    }

    fn set_position(&mut self, id: &Self::Id, position: Vec2) {
        self.set_position(id, position);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElementMeta {
    type_id: TypeId,
    parent: Option<ElementId>,
    size: Size,
    position: Vec2,
    constraint: Constraint,
}

impl ElementMeta {
    pub const fn init(type_id: TypeId) -> Self {
        Self {
            type_id,
            parent: None,
            size: Size::ZERO,
            position: Vec2::ZERO,
            constraint: Constraint::unbounded(),
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy)]
pub struct ElementTypeMeta {
    get_dyn_fn: GetDynElementFn,
    constrain_fn: ConstrainElementFn,
    build_fn: BuildElementFn,
}

impl ElementTypeMeta {
    pub const fn new<E: Element>() -> Self {
        Self {
            get_dyn_fn: get_dyn_element::<E>,
            constrain_fn: constrain_element::<E>,
            build_fn: build_element::<E>,
        }
    }

    pub fn get_dyn<'a>(
        &'a self,
        id: &ElementId,
        table: &'a TypeTable<ElementId>,
    ) -> Option<&'a dyn Element> {
        (self.get_dyn_fn)(id, table)
    }

    pub fn constrain(
        &self,
        id: &ElementId,
        table: &TypeTable<ElementId>,
        type_metas: &ElementTypeMetas,
        metas: &mut ElementMetas,
    ) {
        (self.constrain_fn)(id, table, type_metas, metas)
    }

    pub fn build(
        &self,
        id: &ElementId,
        table: &TypeTable<ElementId>,
        type_metas: &ElementTypeMetas,
        metas: &mut ElementMetas,
    ) {
        (self.build_fn)(id, table, type_metas, metas)
    }
}

/// Function pointer for returning `&dyn Element` from a [`TypeTable`]
/// without knowing the concrete type at the call site.
///
/// One monomorphized instance is registered per element type on first
/// insertion.
pub type GetDynElementFn = for<'a> fn(
    id: &ElementId,
    table: &'a TypeTable<ElementId>,
) -> Option<&'a dyn Element>;

/// Monomorphized implementation of [`GetDynElementFn`] for element
/// type `E`.
#[inline]
pub fn get_dyn_element<'a, E: Element>(
    id: &ElementId,
    table: &'a TypeTable<ElementId>,
) -> Option<&'a dyn Element> {
    let element = table.get::<E>(id);
    element.map(|e| e as &dyn Element)
}

pub type ConstrainElementFn = fn(
    &ElementId,
    &TypeTable<ElementId>,
    &ElementTypeMetas,
    &mut ElementMetas,
);

pub fn constrain_element<E: Element>(
    id: &ElementId,
    table: &TypeTable<ElementId>,
    type_metas: &ElementTypeMetas,
    metas: &mut ElementMetas,
) {
    let type_id = TypeId::of::<E>();

    let Some(element) = table.get::<E>(id) else {
        return;
    };

    let Some(ElementMeta { constraint, .. }) = metas.get(id).copied()
    else {
        return;
    };

    let Some(type_meta) = type_metas.get(&type_id) else {
        return;
    };

    for child in element.children() {
        let Some(child_element) = type_meta.get_dyn(child, table)
        else {
            continue;
        };

        let child_constraint = child_element.constrain(constraint);

        metas.set_parent(child, *id);
        metas.set_constraint(child, child_constraint);

        // Recursively constrain the child.
        type_meta.constrain(child, table, type_metas, metas);
    }
}

pub type BuildElementFn = fn(
    &ElementId,
    &TypeTable<ElementId>,
    &ElementTypeMetas,
    &mut ElementMetas,
);

pub fn build_element<E: Element>(
    id: &ElementId,
    table: &TypeTable<ElementId>,
    type_metas: &ElementTypeMetas,
    metas: &mut ElementMetas,
) {
    let Some(element) = table.get::<E>(id) else {
        return;
    };

    let Some(ElementMeta {
        parent, constraint, ..
    }) = metas.get(id).copied()
    else {
        return;
    };

    let size = element.build(constraint, metas);
    metas.set_size(id, size);

    // Recursively build the parent.
    if let Some(parent) = parent
        && let Some(ElementMeta { type_id, .. }) = metas.get(&parent)
        && let Some(type_meta) = type_metas.get(type_id)
    {
        type_meta.build(id, table, type_metas, metas);
    }
}

// TODO: Optimize builder functions to stop when bubbling size up when it isn't changing.
// TODO: Stop constrain down when constraints stays the same.
