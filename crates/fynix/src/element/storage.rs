use imaging::PaintSink;

use super::Element;
use super::layout::{ElementNodes, ElementTree};
use super::meta::{ElementMetas, ElementTypeMetas};
use crate::id::{GenId, IdGenerator};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};
use crate::type_table::TypeTable;

/// Type-erased storage for all element instances.
///
/// Internally holds one column per element type inside a
/// [`TypeTable`]. The [`ColumnId`] for each element is stored in
/// [`ElementMetas`] so that polymorphic access (via
/// [`Self::get_dyn`]) and removal work without knowing the
/// concrete type at the call site.
///
/// [`ColumnId`]: crate::type_table::ColumnId
pub struct Elements {
    // TODO(nixon): Make these private and provide a more
    // elegant API!
    pub elements: TypeTable<ElementId>,
    pub metas: ElementMetas,
    pub type_metas: ElementTypeMetas,
    id_generator: ElementIdGenerator,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            elements: TypeTable::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
            id_generator: IdGenerator::new(),
        }
    }

    /// Stores `element`, registers its type getter if needed,
    /// and returns a fresh [`ElementId`].
    pub fn add<E: Element>(
        &mut self,
        element: E,
        primary_style: Option<StyleId>,
    ) -> ElementId {
        let col = self.elements.ensure_column::<E>();
        self.type_metas.register::<E>(col);

        let id = self.id_generator.new_id();

        self.metas.init_element(id, col, primary_style);
        self.elements.insert(id, element);
        id
    }

    /// Returns a type-erased reference to the element.
    ///
    /// Prefer [`get_typed`](Elements::get_typed) when the
    /// concrete type is known, it avoids the getter dispatch.
    pub fn get_dyn(&self, id: &ElementId) -> Option<&dyn Element> {
        let col = self.metas.get(id)?.col;
        let type_meta = self.type_metas.get_column(col)?;
        type_meta.get_dyn(&self.elements, id)
    }

    /// Returns a typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not
    /// hold a value of type `E`.
    pub fn get_typed<E: Element>(
        &self,
        id: &ElementId,
    ) -> Option<&E> {
        self.elements.get(id)
    }

    /// Returns a mutable typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not
    /// hold a value of type `E`.
    pub fn get_typed_mut<E: Element>(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut E> {
        self.elements.get_mut(id)
    }

    /// Recursively removes the element subtree along with their styles.
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
            elements: &mut TypeTable<ElementId>,
            id_generator: &mut ElementIdGenerator,
            styles: &mut Styles,
            mut has_removed_styles: bool,
        ) -> bool {
            if let Some(meta) = metas.remove(id)
                && let Some(type_meta) =
                    type_metas.get_column(meta.col)
            {
                if !has_removed_styles
                    && let Some(primary_style) = meta.primary_style
                {
                    has_removed_styles =
                        styles.remove(&primary_style);
                }

                (type_meta.for_each_child_mut_fn)(
                    elements,
                    id,
                    &mut |child_id, elements| {
                        remove_recursive(
                            child_id,
                            metas,
                            type_metas,
                            elements,
                            id_generator,
                            styles,
                            has_removed_styles,
                        );
                    },
                );

                elements.dyn_remove_by_column(meta.col, id);
                id_generator.recycle(*id);
                return true;
            }

            false
        }

        remove_recursive(
            id,
            &mut self.metas,
            &self.type_metas,
            &mut self.elements,
            &mut self.id_generator,
            styles,
            false,
        )
    }

    /// Renders the subtree rooted at `id` into the `painter`.
    ///
    /// Each element's own visual layer is painted via
    /// [`crate::element::ElementBuild::render`] before its children
    /// are visited, so parents always draw behind their children.
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
        if let Some(type_meta) = self.type_metas.get_column(meta.col)
        {
            if let Some(element) =
                type_meta.get_dyn(&self.elements, id)
            {
                element.render(id, painter, &self.metas);
            }
            (type_meta.for_each_child_fn)(
                &self.elements,
                id,
                &mut |child| self.render(child, painter),
            );
        }
    }

    /// Runs a full three-pass layout cycle on the subtree
    /// rooted at `id`.
    ///
    /// The caller is responsible for setting the node's
    /// constraint on [`ElementMetas`] before calling this if
    /// a specific size is required.
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

/// Generational ID for element instances.
pub type ElementId = GenId<_ElementMarker>;
pub type ElementIdGenerator = IdGenerator<_ElementMarker>;

#[doc(hidden)]
pub struct _ElementMarker;
