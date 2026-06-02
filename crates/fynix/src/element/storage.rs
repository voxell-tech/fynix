use imaging::PaintSink;

use super::Element;
use super::layout::{ElementNodes, ElementTree};
use super::meta::{ElementMetas, ElementTypeMetas};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};
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
}

impl Elements {
    pub fn new() -> Self {
        Self {
            elements: TypePool::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
        }
    }

    /// Stores `element`, registers its type if needed, and returns
    /// a fresh [`ElementId`].
    pub fn add<E: Element>(
        &mut self,
        element: E,
        primary_style: Option<StyleId>,
    ) -> ElementId {
        let col = self.elements.ensure_column::<E>();
        self.type_metas.register::<E>(col);

        let key = self.elements.insert_with_key(|key| {
            let id = ElementId(key);

            // Set parent id on each child's meta node.
            for child_id in element.children() {
                if let Some(meta) = self.metas.get_mut(child_id) {
                    meta.node.parent_id = Some(id);
                }
            }
            self.metas.init_element(id, primary_style);
            element
        });

        ElementId(key)
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

    /// Recursively removes the element subtree along with their
    /// styles.
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(
        &mut self,
        id: &ElementId,
        styles: &mut Styles,
    ) -> bool {
        fn remove_recursive(
            id: &ElementId,
            metas: &mut ElementMetas,
            type_metas: &ElementTypeMetas,
            elements: &mut TypePool,
            styles: &mut Styles,
            mut has_removed_styles: bool,
        ) -> bool {
            if let Some(meta) = metas.remove(id)
                && let Some(type_meta) =
                    type_metas.get_column(id.col_id())
            {
                if !has_removed_styles
                    && let Some(primary_style) = meta.primary_style
                {
                    has_removed_styles =
                        styles.remove(&primary_style);
                }

                type_meta.for_each_child_mut(
                    elements,
                    id,
                    &mut |child_id, elements| {
                        remove_recursive(
                            child_id,
                            metas,
                            type_metas,
                            elements,
                            styles,
                            has_removed_styles,
                        );
                    },
                );

                elements.dyn_remove(id);
                return true;
            }

            false
        }

        remove_recursive(
            id,
            &mut self.metas,
            &self.type_metas,
            &mut self.elements,
            styles,
            false,
        )
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

    /// Runs a full three-pass layout cycle on the subtree rooted at
    /// `id`.
    ///
    /// The caller is responsible for setting the node's constraint on
    /// [`ElementMetas`] before calling this if a specific size is
    /// required.
    pub fn layout(
        &mut self,
        id: &ElementId,
        resources: &mut Resources,
    ) {
        let tree = ElementTree {
            elements: &self.elements,
            type_metas: &self.type_metas,
        };

        let mut nodes = ElementNodes {
            metas: &mut self.metas,
            resources,
        };
        rectree::layout(&tree, &mut nodes, id);
    }
}

impl Default for Elements {
    fn default() -> Self {
        Self::new()
    }
}

/// Identifier for an element instance.
///
/// Wraps the [`ColumnKey`] returned by [`TypePool::insert`], so the
/// id is also the direct storage key - no secondary lookup needed.
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
