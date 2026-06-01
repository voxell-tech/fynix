use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::record::Scene;
use rectree::RectNode;

use crate::element::{Element, ElementId};
use crate::style::StyleId;
use crate::type_table::{ColumnId, TypeTable};

/// Per-element metadata.
pub struct ElementMeta {
    pub col_id: ColumnId,
    pub node: RectNode<ElementId>,
    pub cached_scene: Option<Scene>,
    /// When this element is removed, this style and all its
    /// descendants in the style tree are also removed.
    pub primary_style: Option<StyleId>,
}

/// Per-element metadata storage, keyed by [`ElementId`].
pub struct ElementMetas {
    map: HashMap<ElementId, ElementMeta>,
}

impl ElementMetas {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub(super) fn init_element(
        &mut self,
        id: ElementId,
        col_id: ColumnId,
        primary_style: Option<StyleId>,
    ) {
        self.map.insert(
            id,
            ElementMeta {
                col_id,
                node: RectNode::new(None),
                cached_scene: None,
                primary_style,
            },
        );
    }

    /// Removes the element meta and returns it for type-erased
    /// element storage cleanup.
    pub fn remove(&mut self, id: &ElementId) -> Option<ElementMeta> {
        self.map.remove(id)
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

/// Per-type dispatch table registry, slot-indexed.
pub struct ElementTypeMetas {
    slots: Vec<Option<ElementTypeMeta>>,
}

impl ElementTypeMetas {
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Registers `E` at `col` if it has not been registered yet.
    ///
    /// `col` must have been obtained from
    /// [`TypeTable::ensure_column::<E>`].
    pub fn register<E: Element>(&mut self, col: ColumnId) {
        let i = col.index();
        if self.slots.len() <= i {
            self.slots.resize_with(i + 1, || None);
        }
        if self.slots[i].is_none() {
            self.slots[i] = Some(ElementTypeMeta::new::<E>());
        }
    }

    /// Returns the [`ElementTypeMeta`] for `col`, or `None` if that
    /// column has not been registered.
    pub fn get_column(
        &self,
        col: ColumnId,
    ) -> Option<&ElementTypeMeta> {
        self.slots.get(col.index())?.as_ref()
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
    pub for_each_child_fn: ForEachChildFn,
    pub for_each_child_mut_fn: ForEachChildMutFn,
}

impl ElementTypeMeta {
    pub fn new<E: Element>() -> Self {
        Self {
            get_dyn_fn: get_dyn_element::<E>,
            for_each_child_fn: for_each_child::<E>,
            for_each_child_mut_fn: for_each_child_mut::<E>,
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

/// Returns `&dyn Element` from the table without knowing the concrete
/// type at the call site.
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
/// [`ElementChildren::children`].
///
/// Using a visitor avoids the need to name the concrete iterator type
/// returned by [`ElementChildren::children`], which differs per `E`
/// and cannot be expressed in a function-pointer signature.
///
/// [`ElementChildren::children`]: super::ElementChildren::children
pub type ForEachChildFn = fn(
    table: &TypeTable<ElementId>,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
);

/// Like [`ForEachChildFn`], but temporarily removes the element via
/// [`TypeTable::scope`] so the callback receives `&mut
/// TypeTable<ElementId>` without a borrow conflict.
pub type ForEachChildMutFn = fn(
    table: &mut TypeTable<ElementId>,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId, &mut TypeTable<ElementId>),
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

/// Like [`for_each_child`], but uses [`TypeTable::scope`] to lend
/// `&mut TypeTable<ElementId>` to the callback.
///
/// The element at `id` is absent from the table for the duration of
/// the callback, so the callback may freely mutate it (e.g. to
/// recursively remove children) without a borrow conflict.
#[inline]
pub fn for_each_child_mut<E: Element>(
    table: &mut TypeTable<ElementId>,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId, &mut TypeTable<ElementId>),
) {
    table.scope::<E, _>(id, |element, table| {
        for child in element.children() {
            f(child, table);
        }
    });
}
