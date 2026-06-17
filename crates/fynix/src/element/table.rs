use imaging::record::Scene;
use typarena::ColumnId;
use typarena::type_table::TypeTable;

use crate::element::ElementId;
use crate::element::layout::ElementNode;
use crate::style::StyleId;

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
pub struct ElementTable {
    table: TypeTable<ElementId>,
    node_col: ColumnId,
    scene_col: ColumnId,
    style_col: ColumnId,
}

impl ElementTable {
    pub fn new() -> Self {
        let mut table = TypeTable::new();
        let node_col = table.ensure_column::<ElementNode>();
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
        self.table.insert_by_column(
            id,
            ElementNode::new(None),
            self.node_col,
        );
        if let Some(style) = primary_style {
            self.table.insert_by_column(id, style, self.style_col);
        }
    }

    /// Drops every metadata column for `id`. Returns `true` if any
    /// column held it.
    pub(super) fn remove(&mut self, id: &ElementId) -> bool {
        self.table.remove_row(id)
    }

    /// Returns the layout node for `id`, if present.
    pub fn node(&self, id: &ElementId) -> Option<&ElementNode> {
        self.table.get_by_column(self.node_col, id)
    }

    /// Returns a mutable reference to the layout node for `id`.
    pub fn node_mut(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut ElementNode> {
        self.table.get_mut_by_column(self.node_col, id)
    }

    /// Returns the cached scene for `id`, if one has been stored.
    pub fn scene(&self, id: &ElementId) -> Option<&Scene> {
        self.table.get_by_column(self.scene_col, id)
    }

    /// Caches `scene` for `id`.
    pub fn set_scene(&mut self, id: &ElementId, scene: Scene) {
        self.table.insert_by_column(*id, scene, self.scene_col);
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

/// Arbitrary per-element components.
///
/// Beyond the fixed node, scene, and style columns, an element can
/// carry at most one value of any other type `T`, stored in a column
/// created on first insert. Used for the at-most-one-per-element data
/// such as a watch or a field binding. Components are
/// dropped with the element when its row is removed.
impl ElementTable {
    /// Attaches `value` to `id`, replacing any previous component of
    /// type `T`. Returns the replaced value, if any.
    pub fn insert_component<T: 'static>(
        &mut self,
        id: ElementId,
        value: T,
    ) -> Option<T> {
        self.table.insert(id, value)
    }

    /// Removes and returns `id`'s component of type `T`, if present.
    pub fn remove_component<T: 'static>(
        &mut self,
        id: &ElementId,
    ) -> Option<T> {
        self.table.remove(id)
    }

    /// Returns `id`'s component of type `T`, if present.
    pub fn get_component<T: 'static>(
        &self,
        id: &ElementId,
    ) -> Option<&T> {
        self.table.get::<T>(id)
    }

    /// Returns a mutable reference to `id`'s component of type `T`,
    /// if present.
    pub fn get_component_mut<T: 'static>(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut T> {
        self.table.get_mut::<T>(id)
    }

    /// Iterates `(id, &T)` over every element carrying a component of
    /// type `T`, in unspecified order.
    pub fn components<T: 'static>(
        &self,
    ) -> impl Iterator<Item = (&ElementId, &T)> {
        self.table.iter::<T>()
    }
}

impl Default for ElementTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use typarena::type_pool::TypePool;

    use super::*;
    use crate::element::ElementId;
    use crate::style::StyleIdGenerator;

    #[derive(Default)]
    struct IdGenerator {
        pool: TypePool,
    }

    impl IdGenerator {
        fn generate(&mut self) -> ElementId {
            ElementId(self.pool.insert(1))
        }
    }

    #[test]
    fn init_seeds_node_and_primary_style() {
        let id = IdGenerator::default().generate();
        let style = StyleIdGenerator::new().new_id();
        let mut table = ElementTable::new();
        table.init_element(id, Some(style));

        assert!(table.node(&id).is_some());
        assert_eq!(table.primary_style(&id), Some(style));
        // No scene is cached until one is set.
        assert!(table.scene(&id).is_none());
    }

    #[test]
    fn init_without_primary_style_leaves_it_absent() {
        let id = IdGenerator::default().generate();
        let mut table = ElementTable::new();
        table.init_element(id, None);

        assert!(table.node(&id).is_some());
        assert_eq!(table.primary_style(&id), None);
    }

    #[test]
    fn set_scene_round_trips() {
        let id = IdGenerator::default().generate();
        let mut table = ElementTable::new();
        table.init_element(id, None);

        assert!(table.scene(&id).is_none());
        table.set_scene(&id, Scene::new());
        assert_eq!(table.scene(&id), Some(&Scene::new()));
    }

    #[test]
    fn remove_drops_every_column() {
        let mut generator = IdGenerator::default();
        let id0 = generator.generate();
        let id1 = generator.generate();

        let style = StyleIdGenerator::new().new_id();
        let mut table = ElementTable::new();
        table.init_element(id0, Some(style));
        table.init_element(id1, None);
        table.set_scene(&id0, Scene::new());

        assert!(table.remove(&id0));
        assert!(table.node(&id0).is_none());
        assert!(table.scene(&id0).is_none());
        assert_eq!(table.primary_style(&id0), None);
        // A second removal finds nothing left.
        assert!(!table.remove(&id0));
        // Sibling metadata is untouched.
        assert!(table.node(&id1).is_some());
    }

    #[test]
    fn component_round_trips_and_is_dropped_on_remove() {
        let (id, other) = two_ids();
        let mut table = ElementTable::new();
        table.init_element(id, None);
        table.init_element(other, None);

        assert_eq!(table.insert_component(id, 7u32), None);
        assert_eq!(table.get_component::<u32>(&id), Some(&7));
        *table.get_component_mut::<u32>(&id).unwrap() = 9;
        assert_eq!(table.get_component::<u32>(&id), Some(&9));

        // Iteration yields only elements carrying the component.
        let ids = table
            .components::<u32>()
            .map(|(id, _)| *id)
            .collect::<alloc::vec::Vec<_>>();
        assert_eq!(ids, [id]);

        // Removing the element drops its component with it.
        assert!(table.remove(&id));
        assert_eq!(table.get_component::<u32>(&id), None);
    }
}
