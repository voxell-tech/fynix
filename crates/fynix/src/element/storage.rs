use core::any::TypeId;

use hashbrown::HashSet;
use imaging::PaintSink;

use super::Element;
use super::layout::{ElementNodes, ElementTree};
use super::meta::{ElementMeta, ElementMetas, ElementTypeMetas};
use crate::resource::Resources;
use crate::style::StyleId;
use crate::type_pool::{ColumnKey, TypePool};

/// Type-erased storage for all element instances.
///
/// Internally holds one column per element type inside a
/// [`TypePool`].
pub struct Elements {
    // TODO(nixon): Make these private and provide a more
    // elegant API!
    pub elements: TypePool,
    pub metas: ElementMetas,
    pub type_metas: ElementTypeMetas,
    /// Elements whose subtree changed and needs re-layout/render,
    /// e.g. after a reactive scope rebuilt its child.
    pub dirty_elements: HashSet<ElementId>,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            elements: TypePool::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
            dirty_elements: HashSet::new(),
        }
    }

    /// Marks an element's subtree dirty so [`Self::layout`] re-lays
    /// it out on the next call.
    ///
    /// Resets the node's layout state here so rectree re-layout the
    /// subtree.
    pub fn mark_dirty(&mut self, id: ElementId) {
        if self.dirty_elements.insert(id)
            && let Some(meta) = self.metas.get_mut(&id)
        {
            meta.node.state.reset();
        }
    }

    /// Stores `element`, registers its type if needed, and returns
    /// a fresh [`ElementId`].
    #[must_use]
    pub fn add<E: Element>(
        &mut self,
        element: E,
        primary_style: Option<StyleId>,
    ) -> ElementId {
        self.add_with_id(
            #[inline(always)]
            |_| element,
            primary_style,
        )
    }

    /// Like [`Self::add`], but calls `create` with the element's own
    /// [`ElementId`] before the element is stored.
    ///
    /// Use this when an element must embed a reference to its own id,
    /// such as a node that links back to a sibling structure keyed on
    /// that id.
    #[must_use]
    pub fn add_with_id<E: Element>(
        &mut self,
        create: impl FnOnce(ElementId) -> E,
        primary_style: Option<StyleId>,
    ) -> ElementId {
        let col = self.elements.ensure_column::<E>();
        self.type_metas.register::<E>(col);

        let key = self.elements.insert_with_key(|key| {
            let id = ElementId(key);
            let element = create(id);

            // Set parent id on each child's meta node.
            for child_id in element.children() {
                if let Some(meta) = self.metas.get_mut(child_id) {
                    meta.node.parent_id = Some(id);
                }
            }
            self.metas.init_element(id, primary_style);
            element
        });

        let id = ElementId(key);
        self.mark_dirty(id);
        id
    }

    /// Returns a type-erased reference to the element.
    ///
    /// Prefer [`Self::get_typed`] when the concrete type is known, it
    /// avoids the getter dispatch.
    pub fn get_dyn(&self, id: &ElementId) -> Option<&dyn Element> {
        let type_meta = self.type_metas.get_column(id.col_id())?;
        type_meta.get_dyn(&self.elements, id)
    }

    /// Returns a typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not hold a value
    /// of type `E`.
    pub fn get_typed<E: Element>(
        &self,
        id: &ElementId,
    ) -> Option<&E> {
        self.elements.get(id)
    }

    /// Returns a mutable typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not hold a
    /// value of type `E`.
    pub fn get_typed_mut<E: Element>(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut E> {
        self.elements.get_mut(id)
    }

    /// Returns the [`TypeId`] of the element's concrete type, or
    /// `None` if `id` does not exist.
    pub fn type_id_of(&self, id: &ElementId) -> Option<TypeId> {
        self.elements.value_type_id(id)
    }

    /// Recursively removes the element subtree.
    ///
    /// `on_removed` runs for each removed element, in
    /// parent-before-child order, with its id and meta. Use it to
    /// clean up per-element resources such as styles and scopes.
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(
        &mut self,
        id: &ElementId,
        mut on_removed: impl FnMut(&ElementId, &ElementMeta),
    ) -> bool {
        fn remove_recursive(
            id: &ElementId,
            metas: &mut ElementMetas,
            type_metas: &ElementTypeMetas,
            elements: &mut TypePool,
            on_removed: &mut impl FnMut(&ElementId, &ElementMeta),
        ) -> bool {
            if let Some(meta) = metas.remove(id)
                && let Some(type_meta) =
                    type_metas.get_column(id.col_id())
            {
                on_removed(id, &meta);

                type_meta.for_each_child_mut(
                    elements,
                    id,
                    &mut |child_id, elements| {
                        remove_recursive(
                            child_id,
                            metas,
                            type_metas,
                            elements,
                            on_removed,
                        );
                    },
                );

                elements.dyn_remove(id);
                return true;
            }

            false
        }

        // Mark the parent as dirty before dropped the child so we can
        // re-layout the parent's subtree once the child is gone.
        let parent_id =
            self.metas.get(id).and_then(|meta| meta.node.parent_id);

        let removed = remove_recursive(
            id,
            &mut self.metas,
            &self.type_metas,
            &mut self.elements,
            &mut on_removed,
        );

        if removed && let Some(parent_id) = parent_id {
            self.mark_dirty(parent_id);
        }

        removed
    }

    /// Renders the subtree rooted at `id` into the `painter`.
    ///
    /// Each element's own visual layer is painted via
    /// [`super::ElementBuild::render`] before its children are
    /// visited, so parents always draw behind their children.
    ///
    /// Layout must be complete before calling this.
    pub fn render(
        &self,
        id: &ElementId,
        painter: &mut impl PaintSink,
    ) {
        let Some(meta) = self.metas.get(id) else {
            return;
        };
        if let Some(type_meta) =
            self.type_metas.get_column(id.col_id())
        {
            if let Some(element) =
                type_meta.get_dyn(&self.elements, id)
            {
                element.render(id, painter, &self.metas);
            }
            type_meta.for_each_child(
                &self.elements,
                id,
                &mut |child| self.render(child, painter),
            );
        }
        let _ = meta;
    }

    /// Lays out the subtree of every dirty element, draining the
    /// dirty set. Nodes are reset when marked dirty, not here.
    pub fn layout(&mut self, resources: &mut Resources) {
        let tree = ElementTree {
            elements: &self.elements,
            type_metas: &self.type_metas,
        };

        let mut nodes = ElementNodes {
            metas: &mut self.metas,
            resources,
        };

        for id in self.dirty_elements.drain() {
            if self.elements.contains(&id) {
                rectree::layout(&tree, &mut nodes, &id);
            }
        }
    }
}

impl Default for Elements {
    fn default() -> Self {
        Self::new()
    }
}

/// Identifier for an element instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(pub(crate) ColumnKey);

impl ElementId {
    /// A sentinel id that will never refer to a live element.
    pub const PLACEHOLDER: Self = Self(ColumnKey::PLACEHOLDER);
}

impl core::ops::Deref for ElementId {
    type Target = ColumnKey;
    fn deref(&self) -> &ColumnKey {
        &self.0
    }
}
