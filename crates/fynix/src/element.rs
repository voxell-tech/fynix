use rectree::{Constraint, Rectree, Size};

use crate::element::meta::{ElementMetas, ElementTypeMetas};
use crate::id::{GenId, IdGenerator};
use crate::resource::Resources;
use crate::type_table::TypeTable;

pub mod meta;

/// Type-erased storage for all element instances.
///
/// Internally holds one [`TypeTable`] slot per element type. The
/// [`TypeId`] of each element is stored inside [`ElementMetas`]
/// so that polymorphic access (via [`Self::get_dyn`]) and removal
/// work without knowing the concrete type at the call site.
pub struct Elements {
    id_generator: ElementIdGenerator,
    // TODO(nixon): This needs to use `TypeSlot` for fast lookups.
    // Implication: No implication since all `Elements` are defined by
    // us/users. So they will need to have the derive anyways.
    elements: TypeTable<ElementId>,
    metas: ElementMetas,
    type_metas: ElementTypeMetas,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            id_generator: IdGenerator::new(),
            elements: TypeTable::new(),
            metas: ElementMetas::new(),
            type_metas: ElementTypeMetas::new(),
        }
    }

    /// Stores `element`, registers its type getter if needed, and
    /// returns a fresh [`ElementId`].
    pub fn add<E: Element>(&mut self, element: E) -> ElementId {
        self.type_metas.register::<E>();

        let id = self.id_generator.new_id();

        self.metas.init_element::<E>(id);
        self.elements.insert(id, element);
        id
    }

    /// Returns a type-erased reference to the element.
    ///
    /// Prefer [`get_typed`](Elements::get_typed) when the concrete
    /// type is known, it avoids the getter dispatch.
    pub fn get_dyn(&self, id: &ElementId) -> Option<&dyn Element> {
        let type_id = self.metas.get_type_id(id)?;
        let type_meta = self.type_metas.get(&type_id)?;
        type_meta.get_dyn(&self.elements, id)
    }

    /// Returns a typed reference to the element.
    ///
    /// Returns `None` if `id` does not exist or does not hold a
    /// value of type `E`.
    pub fn get_typed<E: Element>(
        &self,
        id: &ElementId,
    ) -> Option<&E> {
        self.elements.get::<E>(id)
    }

    /// Removes the element and recycles its [`ElementId`].
    ///
    /// Returns `true` if the element was present and removed.
    pub fn remove(&mut self, id: &ElementId) -> bool {
        if let Some(type_id) = self.metas.remove(id)
            && self.elements.dyn_remove(&type_id, id)
        {
            self.id_generator.recycle(*id);
            return true;
        }

        false
    }

    /// Runs a full three-pass layout cycle on the subtree rooted at
    /// `id`.
    ///
    /// The caller is responsible for setting the node's constraint
    /// on [`ElementMetas`] before calling this if a specific size
    /// is required.
    pub fn layout(&mut self, id: &ElementId, resources: &Resources) {
        let tree = ElementTree {
            elements: &self.elements,
            type_metas: &self.type_metas,
            resources,
        };
        rectree::layout(&tree, &mut self.metas, id);
    }
}

impl Default for Elements {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable view of the element tree used to implement [`Rectree`].
///
/// Borrows the type tables from [`Elements`] so that [`ElementMetas`]
/// can be mutably borrowed separately during layout.
struct ElementTree<'a> {
    elements: &'a TypeTable<ElementId>,
    type_metas: &'a ElementTypeMetas,
    resources: &'a Resources,
}

impl Rectree for ElementTree<'_> {
    type Id = ElementId;
    type Nodes = ElementMetas;

    fn for_each_child(
        &self,
        id: &ElementId,
        metas: &mut ElementMetas,
        mut f: impl FnMut(&ElementId, &mut ElementMetas),
    ) {
        let type_id = metas.get_type_id(id);
        if let Some(type_meta) =
            type_id.and_then(|t| self.type_metas.get(&t))
        {
            (type_meta.children_fn)(
                self.elements,
                id,
                &mut |child| f(child, metas),
            );
        }
    }

    fn constrain(
        &self,
        id: &ElementId,
        nodes: &ElementMetas,
        parent: Constraint,
    ) -> Constraint {
        let type_id = nodes.get_type_id(id);
        type_id
            .and_then(|t| self.type_metas.get(&t))
            .map(|m| (m.constrain_fn)(self.elements, id, parent))
            .unwrap_or(parent)
    }

    fn build(
        &self,
        id: &ElementId,
        constraint: Constraint,
        metas: &mut ElementMetas,
    ) -> Size {
        let type_id = metas.get_type_id(id);
        type_id
            .and_then(|t| self.type_metas.get(&t))
            .map(|m| {
                (m.build_fn)(
                    self.elements,
                    id,
                    constraint,
                    metas,
                    self.resources,
                )
            })
            .unwrap_or(Size::ZERO)
    }
}

/// Marker trait for widget types.
///
/// Implement this for any type you want to add to the element tree
/// via [`BuildCtx::add`](crate::ctx::BuildCtx::add). The single
/// required method, `new`, must return a default (unstyled) instance.
/// Styles are applied immediately after construction by the build
/// context.
pub trait Element: 'static {
    fn new() -> Self
    where
        Self: Sized;

    fn children(&self) -> impl IntoIterator<Item = &ElementId>
    where
        Self: Sized,
    {
        []
    }

    fn constrain(&self, parent_constraint: Constraint) -> Constraint {
        parent_constraint
    }

    #[expect(unused_variables)]
    fn build(
        &self,
        constraint: Constraint,
        metas: &mut ElementMetas,
        resources: &Resources,
    ) -> Size
    where
        Self: Sized,
    {
        constraint.min
    }
}

/// Generational ID for element instances.
pub type ElementId = GenId<_ElementMarker>;
pub type ElementIdGenerator = IdGenerator<_ElementMarker>;

#[doc(hidden)]
pub struct _ElementMarker;
