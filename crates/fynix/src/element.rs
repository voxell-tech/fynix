use imaging::PaintSink;
use imaging::record::Scene;
use rectree::{Constraint, RectNode, RectNodes, Rectree, Size};
use typeslot::{SlotGroup, TypeSlot};

use crate::element::meta::{ElementMetas, ElementTypeMetas};
use crate::element::table::ElementTable;
use crate::id::{GenId, IdGenerator};
use crate::resource::Resources;

pub use fynix_macros::{Element, ElementSlot};

pub mod meta;
pub mod table;

/// Marker type for the element slot group.
#[derive(SlotGroup)]
pub struct ElementGroup;

/// Constructs a default (unstyled) instance of an element.
///
/// Derived by `#[derive(Element)]` - calls `Default::default()` unless
/// overridden with `#[element(new = my_fn)]`.
pub trait ElementNew {
    fn new() -> Self
    where
        Self: Sized;
}

/// Enumerates the children of an element.
///
/// Derived by `#[derive(Element)]` - iterates the field tagged `#[children]`,
/// or the fn given in `#[element(children = my_fn)]`. Defaults to no children.
pub trait ElementChildren {
    fn children(&self) -> impl IntoIterator<Item = &ElementId>
    where
        Self: Sized,
    {
        []
    }
}

/// Layout and rendering protocol for element types.
///
/// Implement this manually alongside `#[derive(Element)]`.
pub trait ElementBuild {
    fn constrain(&self, parent_constraint: Constraint) -> Constraint {
        parent_constraint
    }

    fn build(
        &self,
        id: &ElementId,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size;

    /// Paints the element's own visual layer into `painter`.
    ///
    /// The element's world-space position and layout size can
    /// be read from `metas` using `id`. Both are set by the
    /// layout pass and are safe to use for rendering
    /// coordinates.
    ///
    /// Child elements are rendered by the tree walker after
    /// this method returns - do not recurse into children
    /// here.
    ///
    /// The default implementation is a no-op, suitable for
    /// purely structural elements that have no visual of
    /// their own.
    #[expect(unused_variables)]
    fn render(
        &self,
        id: &ElementId,
        painter: &mut dyn PaintSink,
        metas: &ElementMetas,
    ) {
    }
}

/// Marker trait for element types. Use `#[derive(Element)]` to implement
/// this alongside [`ElementNew`] and [`ElementChildren`] automatically.
/// Implement [`ElementBuild`] manually.
pub trait Element:
    ElementNew
    + ElementChildren
    + ElementBuild
    + TypeSlot<ElementGroup>
    + 'static
{
}

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

    /// Recursively removes the element subtree.
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(&mut self, id: &ElementId) -> bool {
        fn remove_recursive(
            id: &ElementId,
            metas: &mut ElementMetas,
            type_metas: &ElementTypeMetas,
            elements: &mut ElementTable,
            id_generator: &mut ElementIdGenerator,
        ) -> bool {
            if let Some(meta) = metas.remove(id)
                && let Some(type_meta) =
                    type_metas.get_slot(meta.slot)
            {
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
                        );
                    },
                );

                elements.dyn_remove_by_slot(meta.slot, id);
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
        )
    }

    /// Renders the subtree rooted at `id` into `sink`.
    ///
    /// Each element's own visual layer is painted via
    /// [`ElementBuild::render`] before its children are visited,
    /// so parents always draw behind their children.
    ///
    /// Layout must be complete before calling this -
    /// positions come from
    /// [`rectree::RectNode::world_translation`].
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

/// Immutable view of the element tree used to implement
/// [`Rectree`].
///
/// Borrows the type tables from [`Elements`] so that
/// [`ElementMetas`] can be mutably borrowed separately
/// during layout.
pub struct ElementTree<'a> {
    elements: &'a ElementTable,
    type_metas: &'a ElementTypeMetas,
}

pub struct ElementNodes<'a> {
    metas: &'a mut ElementMetas,
    resources: &'a mut Resources,
}

impl ElementNodes<'_> {
    pub fn get_resource<R: 'static>(&self) -> Option<&R> {
        self.resources.get()
    }

    pub fn get_resource_mut<R: 'static>(&mut self) -> Option<&mut R> {
        self.resources.get_mut()
    }

    pub fn cache_scene(
        &mut self,
        id: &ElementId,
        scene: Scene,
    ) -> bool {
        if let Some(meta) = self.metas.get_mut(id) {
            meta.cached_scene = Some(scene);
            return true;
        }

        false
    }
}

// TODO: Hide this implementation to the `build` fn. Maybe
// add a `ElementNodesBuilder` wrapper struct.
impl RectNodes for ElementNodes<'_> {
    type Id = ElementId;

    fn get_node(
        &self,
        id: &ElementId,
    ) -> Option<&RectNode<ElementId>> {
        self.metas.get(id).map(|m| &m.node)
    }

    fn get_node_mut(
        &mut self,
        id: &ElementId,
    ) -> Option<&mut RectNode<ElementId>> {
        self.metas.get_mut(id).map(|m| &mut m.node)
    }
}

impl<'a> Rectree for ElementTree<'a> {
    type Id = ElementId;
    type Nodes = ElementNodes<'a>;

    fn for_each_child(
        &self,
        id: &ElementId,
        nodes: &mut Self::Nodes,
        mut f: impl FnMut(&ElementId, &mut Self::Nodes),
    ) {
        if let Some(type_meta) = nodes
            .metas
            .get(id)
            .and_then(|m| self.type_metas.get_slot(m.slot))
        {
            (type_meta.for_each_child_fn)(
                self.elements,
                id,
                &mut |child| f(child, nodes),
            );
        }
    }

    fn constrain(
        &self,
        id: &ElementId,
        nodes: &Self::Nodes,
        parent: Constraint,
    ) -> Constraint {
        nodes
            .metas
            .get(id)
            .and_then(|m| self.type_metas.get_slot(m.slot))
            .map(|m| {
                m.get_dyn(self.elements, id)
                    .map(|e| e.constrain(parent))
                    .unwrap_or(parent)
            })
            .unwrap_or(parent)
    }

    fn build(
        &self,
        id: &ElementId,
        constraint: Constraint,
        nodes: &mut Self::Nodes,
    ) -> Size {
        nodes
            .metas
            .get(id)
            .and_then(|m| self.type_metas.get_slot(m.slot))
            .map(|m| {
                m.get_dyn(self.elements, id)
                    .map(|e| e.build(id, constraint, nodes))
                    .unwrap_or_default()
            })
            .unwrap_or(Size::ZERO)
    }
}

/// Generational ID for element instances.
pub type ElementId = GenId<_ElementMarker>;
pub type ElementIdGenerator = IdGenerator<_ElementMarker>;

#[doc(hidden)]
pub struct _ElementMarker;
