use alloc::vec::Vec;

use hashbrown::HashMap;
use imaging::record::Scene;
use rectree::RectNode;

use crate::element::{Element, ElementId};
use crate::style::StyleId;
use crate::type_pool::{ColumnId, TypePool};

/// Per-element metadata.
pub struct ElementMeta {
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
        primary_style: Option<StyleId>,
    ) {
        self.map.insert(
            id,
            ElementMeta {
                node: RectNode::new(None),
                cached_scene: None,
                primary_style,
            },
        );
    }

    /// Removes the element meta and returns it for cleanup.
    pub(super) fn remove(
        &mut self,
        id: &ElementId,
    ) -> Option<ElementMeta> {
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

/// Per-type dispatch table registry, column-indexed.
pub struct ElementTypeMetas {
    columns: Vec<Option<ElementTypeMeta>>,
}

impl ElementTypeMetas {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
        }
    }

    /// Registers `E` at `col` if it has not been registered yet.
    ///
    /// `col` must have been obtained from
    /// [`TypePool::ensure_column::<E>`].
    pub fn register<E: Element>(&mut self, col: ColumnId) {
        let i = col.index();
        if self.columns.len() <= i {
            self.columns.resize_with(i + 1, || None);
        }
        if self.columns[i].is_none() {
            self.columns[i] = Some(ElementTypeMeta::new::<E>());
        }
    }

    /// Returns the [`ElementTypeMeta`] for `col`, or `None` if that
    /// column has not been registered.
    pub fn get_column(
        &self,
        col: ColumnId,
    ) -> Option<&ElementTypeMeta> {
        self.columns.get(col.index())?.as_ref()
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
pub struct ElementTypeMeta {
    /// Returns `&dyn Element` from the pool without knowing the
    /// concrete type at the call site.
    pub get_dyn_fn: GetDynElementFn,
    /// Visits each child of an element by calling `f` for every
    /// [`ElementId`] the element yields from
    /// [`super::ElementChildren::children`].
    pub for_each_child_fn: ForEachChildFn,
    /// Like [`ForEachChildFn`], but provides `&mut TypePool` to the
    /// callback via [`TypePool::scope`].
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
        pool: &'a TypePool,
        id: &ElementId,
    ) -> Option<&'a dyn Element> {
        (self.get_dyn_fn)(pool, id)
    }

    pub fn for_each_child(
        &self,
        pool: &TypePool,
        id: &ElementId,
        f: &mut dyn FnMut(&ElementId),
    ) {
        (self.for_each_child_fn)(pool, id, f);
    }

    pub fn for_each_child_mut(
        &self,
        pool: &mut TypePool,
        id: &ElementId,
        f: &mut dyn FnMut(&ElementId, &mut TypePool),
    ) {
        (self.for_each_child_mut_fn)(pool, id, f);
    }
}

/// See [`ElementTypeMeta::get_dyn_fn`].
pub type GetDynElementFn = for<'a> fn(
    pool: &'a TypePool,
    id: &ElementId,
) -> Option<&'a dyn Element>;

/// See [`ElementTypeMeta::for_each_child_fn`].
pub type ForEachChildFn = fn(
    pool: &TypePool,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
);

/// See [`ElementTypeMeta::for_each_child_mut_fn`].
pub type ForEachChildMutFn = fn(
    pool: &mut TypePool,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId, &mut TypePool),
);

/// See [`ElementTypeMeta::get_dyn_fn`].
#[inline]
pub fn get_dyn_element<'a, E: Element>(
    pool: &'a TypePool,
    id: &ElementId,
) -> Option<&'a dyn Element> {
    pool.get::<E>(id).map(|e| e as &dyn Element)
}

/// See [`ElementTypeMeta::for_each_child_fn`].
#[inline]
pub fn for_each_child<E: Element>(
    pool: &TypePool,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId),
) {
    if let Some(element) = pool.get::<E>(id) {
        for child in element.children() {
            f(child);
        }
    }
}

/// See [`ElementTypeMeta::for_each_child_mut_fn`].
#[inline]
pub fn for_each_child_mut<E: Element>(
    pool: &mut TypePool,
    id: &ElementId,
    f: &mut dyn FnMut(&ElementId, &mut TypePool),
) {
    pool.scope::<E, _>(id, |element, pool| {
        for child in element.children() {
            f(child, pool);
        }
    });
}
