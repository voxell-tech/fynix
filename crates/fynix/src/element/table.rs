use core::marker::PhantomData;

use imaging::record::Scene;
use typarena::ColumnId;
use typarena::type_table::TypeTable;

use crate::element::ElementId;
use crate::element::layout::ElementNode;
use crate::element::observer::{ObserverFn, Observers};
use crate::reactive::binding::Binding;
use crate::reactive::watcher::Watcher;
use crate::style::StyleId;

/// Per-element component storage, keyed by [`ElementId`].
///
/// Each component field lives in its own [`TypeTable`] column
/// (struct-of-arrays).
///
/// # Observers
///
/// Per-column [`Observers`] fire after a component is inserted or
/// removed. Register with [`Self::on_insert`] / [`Self::on_remove`].
pub struct ElementTable<W: 'static> {
    table: TypeTable<ElementId>,
    observers: Observers<W>,
    node_col: ColumnId,
    scene_col: ColumnId,
    style_col: ColumnId,
    watch_col: ColumnId,
    binding_col: ColumnId,
    _world_marker: PhantomData<fn() -> W>,
}

impl<W> ElementTable<W> {
    pub fn new() -> Self {
        let mut table = TypeTable::new();

        // Store column ids for fast access.
        let node_col = table.ensure_column::<ElementNode>();
        let scene_col = table.ensure_column::<Scene>();
        let style_col = table.ensure_column::<StyleId>();
        let watch_col = table.ensure_column::<Watcher<W>>();
        let binding_col = table.ensure_column::<Binding<W>>();

        Self {
            table,
            observers: Observers::new(),
            node_col,
            scene_col,
            style_col,
            watch_col,
            binding_col,
            _world_marker: PhantomData,
        }
    }

    /// Registers `observer` to run after a component of type `T` is
    /// inserted on any element (every insert), receiving the table
    /// and that element's id. One observer per `T`; a later call
    /// replaces it.
    pub fn on_insert<T: 'static>(&mut self, observer: ObserverFn<W>) {
        let col = self.table.ensure_column::<T>();
        self.observers.set_on_insert(col, observer);
    }

    /// Registers `observer` to run after a component of type `T` is
    /// removed from an element. One observer per `T`; a later call
    /// replaces it.
    pub fn on_remove<T: 'static>(&mut self, observer: ObserverFn<W>) {
        let col = self.table.ensure_column::<T>();
        self.observers.set_on_remove(col, observer);
    }

    /// Seeds the layout node for `id` and records its
    /// `primary_style` when one is given.
    pub(super) fn init_element(
        &mut self,
        id: ElementId,
        primary_style: Option<StyleId>,
    ) {
        self.insert_component_by_column(
            id,
            ElementNode::new(None),
            self.node_col,
        );
        if let Some(style) = primary_style {
            self.insert_component_by_column(
                id,
                style,
                self.style_col,
            );
        }
    }
}

/// Rendering & layouting.
impl<W> ElementTable<W> {
    /// Drops every column for `id` (the whole row), returning `true`
    /// if any held it. Bulk teardown does not fire `on_remove`
    /// observers; only [`remove_component`](Self::remove_component)
    /// does.
    pub(super) fn remove(&mut self, id: &ElementId) -> bool {
        self.table.remove_row(id)
    }

    /// Creates a [`RenderElementTable`].
    pub fn render_table(&self) -> RenderElementTable<'_> {
        RenderElementTable {
            table: &self.table,
            node_col: self.node_col,
            scene_col: self.scene_col,
        }
    }

    /// Creates a [`LayoutElementTable`].
    pub fn layout_table(&mut self) -> LayoutElementTable<'_> {
        LayoutElementTable {
            table: &mut self.table,
            node_col: self.node_col,
            scene_col: self.scene_col,
        }
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
    pub fn set_scene(&mut self, id: ElementId, scene: Scene) {
        self.insert_component_by_column(id, scene, self.scene_col);
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

/// Reactive.
impl<W> ElementTable<W> {
    /// Attaches `watcher` to `id`, replacing any previous one.
    /// Returns the replaced watcher, if any.
    pub fn insert_watcher(
        &mut self,
        id: ElementId,
        watcher: Watcher<W>,
    ) -> Option<Watcher<W>> {
        self.insert_component_by_column(id, watcher, self.watch_col)
    }

    /// Attaches `binding` to `id`, replacing any previous one.
    /// Returns the replaced binding, if any.
    pub fn insert_binding(
        &mut self,
        id: ElementId,
        binding: Binding<W>,
    ) -> Option<Binding<W>> {
        self.insert_component_by_column(id, binding, self.binding_col)
    }
}

/// Arbitrary components.
impl<W> ElementTable<W> {
    /// Inserts via a pre-resolved column, then fires the `on_insert`
    /// observer for that column if one is registered. Every typed
    /// insert routes through here, so observers see every insert.
    #[inline]
    fn insert_component_by_column<T: 'static>(
        &mut self,
        id: ElementId,
        value: T,
        col: ColumnId,
    ) -> Option<T> {
        let previous = self.table.insert_by_column(id, value, col);
        if let Some(observer) = self.observers.get_on_insert(col) {
            observer(self, id);
        }
        previous
    }

    /// Removes via a pre-resolved column, then fires the `on_remove`
    /// observer for that column if a value was actually removed.
    #[inline]
    fn remove_component_by_column<T: 'static>(
        &mut self,
        id: &ElementId,
        col: ColumnId,
    ) -> Option<T> {
        let removed = self.table.remove_by_column::<T>(id, col);
        if removed.is_some()
            && let Some(observer) = self.observers.get_on_remove(col)
        {
            observer(self, *id);
        }
        removed
    }

    /// Attaches `value` to `id`, replacing any previous component of
    /// type `T`. Returns the replaced value, if any.
    pub fn insert_component<T: 'static>(
        &mut self,
        id: ElementId,
        value: T,
    ) -> Option<T> {
        let col = self.table.ensure_column::<T>();
        self.insert_component_by_column(id, value, col)
    }

    /// Removes and returns `id`'s component of type `T`, if present.
    pub fn remove_component<T: 'static>(
        &mut self,
        id: &ElementId,
    ) -> Option<T> {
        let col = self.table.type_column::<T>()?;
        self.remove_component_by_column::<T>(id, col)
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

    /// Returns `true` if `id` carries a component of type `T`.
    pub fn contains<T: 'static>(&self, id: &ElementId) -> bool {
        self.get_component::<T>(id).is_some()
    }

    /// Iterates `(id, &T)` over every element carrying a component of
    /// type `T`, in unspecified order.
    pub fn components<T: 'static>(
        &self,
    ) -> impl Iterator<Item = (&ElementId, &T)> {
        self.table.iter::<T>()
    }
}

impl<W> Default for ElementTable<W> {
    fn default() -> Self {
        Self::new()
    }
}

/// A read-only view of the node and scene columns, handed to the
/// renderer.
pub struct RenderElementTable<'a> {
    table: &'a TypeTable<ElementId>,
    node_col: ColumnId,
    scene_col: ColumnId,
}

impl RenderElementTable<'_> {
    /// Returns the layout node for `id`, if present.
    pub fn node(&self, id: &ElementId) -> Option<&ElementNode> {
        self.table.get_by_column(self.node_col, id)
    }

    /// Returns the cached scene for `id`, if one has been stored.
    pub fn scene(&self, id: &ElementId) -> Option<&Scene> {
        self.table.get_by_column(self.scene_col, id)
    }
}

/// A mutable view of the node and scene columns, used during layout.
pub struct LayoutElementTable<'a> {
    table: &'a mut TypeTable<ElementId>,
    node_col: ColumnId,
    scene_col: ColumnId,
}

impl LayoutElementTable<'_> {
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

    type TestElementTable = ElementTable<()>;

    #[test]
    fn init_seeds_node_and_primary_style() {
        let id = IdGenerator::default().generate();
        let style = StyleIdGenerator::new().new_id();
        let mut table = TestElementTable::new();
        table.init_element(id, Some(style));

        assert!(table.node(&id).is_some());
        assert_eq!(table.primary_style(&id), Some(style));
        // No scene is cached until one is set.
        assert!(table.scene(&id).is_none());
    }

    #[test]
    fn init_without_primary_style_leaves_it_absent() {
        let id = IdGenerator::default().generate();
        let mut table = TestElementTable::new();
        table.init_element(id, None);

        assert!(table.node(&id).is_some());
        assert_eq!(table.primary_style(&id), None);
    }

    #[test]
    fn set_scene_round_trips() {
        let id = IdGenerator::default().generate();
        let mut table = TestElementTable::new();
        table.init_element(id, None);

        assert!(table.scene(&id).is_none());
        table.set_scene(id, Scene::new());
        assert_eq!(table.scene(&id), Some(&Scene::new()));
    }

    #[test]
    fn remove_drops_every_column() {
        let mut generator = IdGenerator::default();
        let id0 = generator.generate();
        let id1 = generator.generate();

        let style = StyleIdGenerator::new().new_id();
        let mut table = TestElementTable::new();
        table.init_element(id0, Some(style));
        table.init_element(id1, None);
        table.set_scene(id0, Scene::new());

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
        let mut generator = IdGenerator::default();
        let id = generator.generate();
        let other = generator.generate();
        let mut table = TestElementTable::new();
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

    #[test]
    fn on_insert_observer_derives_component() {
        let id = IdGenerator::default().generate();
        let mut table = TestElementTable::new();
        table.init_element(id, None);

        // Inserting a `u32` derives an `i64` marker on the same id.
        table.on_insert::<u32>(|table, id| {
            table.insert_component(id, 99i64);
        });

        table.insert_component(id, 7u32);
        assert_eq!(table.get_component::<u32>(&id), Some(&7));
        assert_eq!(table.get_component::<i64>(&id), Some(&99));
    }

    #[test]
    fn on_remove_observer_runs_when_a_value_is_removed() {
        let id = IdGenerator::default().generate();
        let mut table = TestElementTable::new();
        table.init_element(id, None);

        table.on_remove::<u32>(|table, id| {
            table.insert_component(id, -1i64);
        });

        // No value to remove: the observer does not run.
        assert_eq!(table.remove_component::<u32>(&id), None);
        assert_eq!(table.get_component::<i64>(&id), None);

        // Now there is a value: removing it runs the observer.
        table.insert_component(id, 7u32);
        assert_eq!(table.remove_component::<u32>(&id), Some(7));
        assert_eq!(table.get_component::<i64>(&id), Some(&-1));
    }
}
