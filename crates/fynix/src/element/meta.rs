use imaging::record::Scene;
use rectree::RectNode;

use crate::element::ElementId;
use crate::style::StyleId;
use crate::typing::ColumnId;
use crate::typing::type_table::TypeTable;

/// Per-element metadata storage, keyed by [`ElementId`].
///
/// Each metadata field lives in its own [`TypeTable`] column
/// (struct-of-arrays). The layout node is dense, one per element,
/// while the cached scene and primary style are sparse, present only
/// when set.
///
/// The three columns are created up front and their [`ColumnId`]s
/// cached, so every accessor reaches its column by index and skips
/// the per-call [`TypeId`](core::any::TypeId) hash lookup.
pub struct ElementMetas {
    table: TypeTable<ElementId>,
    node_col: ColumnId,
    scene_col: ColumnId,
    style_col: ColumnId,
}

impl ElementMetas {
    pub fn new() -> Self {
        let mut table = TypeTable::new();
        let node_col = table.ensure_column::<RectNode<ElementId>>();
        let scene_col = table.ensure_column::<Scene>();
        let style_col = table.ensure_column::<StyleId>();
        Self {
            table,
            node_col,
            scene_col,
            style_col,
        }
    }

    /// Seeds the layout node for `id` and records its
    /// `primary_style` when one is given.
    pub(super) fn init_element(
        &mut self,
        id: ElementId,
        primary_style: Option<StyleId>,
    ) {
        self.table.insert(id, RectNode::<ElementId>::new(None));
        if let Some(style) = primary_style {
            self.table.insert(id, style);
        }
    }

    /// Drops every metadata column for `id`. Returns `true` if any
    /// column held it.
    pub(super) fn remove(&mut self, id: &ElementId) -> bool {
        self.table.remove_row(id)
    }

    /// Returns the layout node for `id`, if present.
    pub fn node(
        &self,
        id: &ElementId,
    ) -> Option<&RectNode<ElementId>> {
        self.table.get_by_column(self.node_col, id)
    }

    /// Returns a mutable reference to the layout node for `id`.
    pub fn node_mut(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut RectNode<ElementId>> {
        self.table.get_mut_by_column(self.node_col, id)
    }

    /// Returns the cached scene for `id`, if one has been stored.
    pub fn scene(&self, id: &ElementId) -> Option<&Scene> {
        self.table.get_by_column(self.scene_col, id)
    }

    /// Caches `scene` for `id`.
    pub fn set_scene(&mut self, id: &ElementId, scene: Scene) {
        self.table.insert(*id, scene);
    }

    /// Returns the `primary_style` recorded for `id`, if any.
    ///
    /// When this element is removed, that style and all its
    /// descendants in the style tree are also removed.
    pub fn primary_style(&self, id: &ElementId) -> Option<StyleId> {
        self.table
            .get_by_column::<StyleId>(self.style_col, id)
            .copied()
    }
}

impl Default for ElementMetas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::ElementId;
    use crate::style::StyleIdGenerator;
    use crate::typing::type_pool::TypePool;

    /// Mints two distinct element ids via a throwaway pool.
    fn two_ids() -> (ElementId, ElementId) {
        let mut pool = TypePool::new();
        (ElementId(pool.insert(0u8)), ElementId(pool.insert(1u8)))
    }

    #[test]
    fn init_seeds_node_and_primary_style() {
        let (id, _) = two_ids();
        let style = StyleIdGenerator::new().new_id();
        let mut metas = ElementMetas::new();
        metas.init_element(id, Some(style));

        assert!(metas.node(&id).is_some());
        assert_eq!(metas.primary_style(&id), Some(style));
        // No scene is cached until one is set.
        assert!(metas.scene(&id).is_none());
    }

    #[test]
    fn init_without_primary_style_leaves_it_absent() {
        let (id, _) = two_ids();
        let mut metas = ElementMetas::new();
        metas.init_element(id, None);

        assert!(metas.node(&id).is_some());
        assert_eq!(metas.primary_style(&id), None);
    }

    #[test]
    fn set_scene_round_trips() {
        let (id, _) = two_ids();
        let mut metas = ElementMetas::new();
        metas.init_element(id, None);

        assert!(metas.scene(&id).is_none());
        metas.set_scene(&id, Scene::new());
        assert_eq!(metas.scene(&id), Some(&Scene::new()));
    }

    #[test]
    fn remove_drops_every_column() {
        let (id, other) = two_ids();
        let style = StyleIdGenerator::new().new_id();
        let mut metas = ElementMetas::new();
        metas.init_element(id, Some(style));
        metas.init_element(other, None);
        metas.set_scene(&id, Scene::new());

        assert!(metas.remove(&id));
        assert!(metas.node(&id).is_none());
        assert!(metas.scene(&id).is_none());
        assert_eq!(metas.primary_style(&id), None);
        // A second removal finds nothing left.
        assert!(!metas.remove(&id));
        // Sibling metadata is untouched.
        assert!(metas.node(&other).is_some());
    }
}
