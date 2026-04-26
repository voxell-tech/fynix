use imaging::PaintSink;

use super::Element;
use super::layout::{ElementNodes, ElementTree};
use super::meta::{ElementMetas, ElementTypeMetas};
use super::table::ElementTable;
use crate::id::{GenId, IdGenerator};
use crate::resource::Resources;

/// Type-erased storage for all element instances.
///
/// Internally holds one [`ElementTable`] column per element
/// type. The slot index of each element is stored inside
/// [`ElementMetas`] so that polymorphic access (via
/// [`Self::get_dyn`]) and removal work without knowing the
/// concrete type at the call site.
pub struct Elements {
    // TODO(nixon): Make these private and provide a more
    // elegant API!
    pub elements: ElementTable,
    pub metas: ElementMetas,
    pub type_metas: ElementTypeMetas,
    id_generator: ElementIdGenerator,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            elements: ElementTable::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
            id_generator: IdGenerator::new(),
        }
    }

    /// Stores `element`, registers its type getter if needed,
    /// and returns a fresh [`ElementId`].
    pub fn add<E: Element>(&mut self, element: E) -> ElementId {
        self.type_metas.register::<E>();

        let id = self.id_generator.new_id();

        self.metas.init_element::<E>(id);
        self.elements.insert(id, element);
        id
    }

    /// Returns a type-erased reference to the element.
    ///
    /// Prefer [`get_typed`](Elements::get_typed) when the
    /// concrete type is known, it avoids the getter dispatch.
    pub fn get_dyn(&self, id: &ElementId) -> Option<&dyn Element> {
        let slot = self.metas.get(id)?.slot;
        let type_meta = self.type_metas.get_slot(slot)?;
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
        self.elements.get::<E>(id)
    }

    /// Returns a mutable typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not
    /// hold a value of type `E`.
    pub fn get_typed_mut<E: Element>(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut E> {
        self.elements.get_mut::<E>(id)
    }

    /// Removes the element and recycles its [`ElementId`].
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(&mut self, id: &ElementId) -> bool {
        if let Some(meta) = self.metas.remove(id)
            && self.elements.dyn_remove_by_slot(meta.slot, id)
        {
            self.id_generator.recycle(*id);
            return true;
        }

        false
    }

    /// Renders the subtree rooted at `id` into the `painter`.
    ///
    /// Each element's own visual layer is painted via
    /// [`crate::element::ElementBuild::render`] before its children are visited,
    /// so parents always draw behind their children.
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
        if let Some(type_meta) = self.type_metas.get_slot(meta.slot) {
            if let Some(element) =
                type_meta.get_dyn(&self.elements, id)
            {
                element.render(id, painter, &self.metas);
            }
            (type_meta.children_fn)(
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
