use imaging::PaintSink;
use rectree::{Constraint, Size};
use typeslot::{SlotGroup, TypeSlot};

use crate::element::layout::{ElementNodes, ElementTree};
use crate::element::meta::{ElementMetas, ElementTypeMetas};
use crate::element::table::ElementTable;
use crate::id::{GenId, IdGenerator};
use crate::resource::Resources;
use crate::style::{StyleId, Styles};

pub use fynix_macros::{Element, ElementSlot, ElementTemplate};

pub mod layout;
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

/// Marker trait for element template types.
///
/// Use `#[derive(ElementTemplate)]` to implement this and the
/// associated supertraits automatically.
///
/// Use this for generic types, for non-generic types use [`Element`].
pub trait ElementTemplate:
    ElementNew + ElementChildren + ElementBuild + 'static
{
}

/// Marker trait for element types.
///
/// Use `#[derive(Element)]` to implement this and the associated
/// supertraits automatically.
///
/// For generic types, use [`ElementTemplate`].
pub trait Element: ElementTemplate + TypeSlot<ElementGroup> {}

impl<T> Element for T where T: ElementTemplate + TypeSlot<ElementGroup>
{}

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
    pub fn add<E: Element>(
        &mut self,
        element: E,
        primary_style: Option<StyleId>,
    ) -> ElementId {
        self.type_metas.register::<E>();

        let id = self.id_generator.new_id();

        self.metas.init_element::<E>(id, primary_style);
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
            elements: &mut ElementTable,
            id_generator: &mut ElementIdGenerator,
            styles: &mut Styles,
            mut has_removed_styles: bool,
        ) -> bool {
            if let Some(meta) = metas.remove(id)
                && let Some(type_meta) =
                    type_metas.get_slot(meta.slot)
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
            styles,
            false,
        )
    }

    /// Renders the subtree rooted at `id` into the `painter`.
    ///
    /// Each element's own visual layer is painted via
    /// [`ElementBuild::render`] before its children are visited,
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

/// Creates a concrete newtype wrapper around a generic element type,
/// registers it with the element slot group, and forwards all
/// `ElementTemplate` supertraits to the inner type.
///
/// Use this when you have a generic element (e.g. `Button<MyAction>`)
/// that can't be registered directly due to the orphan rule.
///
/// # Example
///
/// ```ignore
/// fynix::register_element!(pub AppButton, my_crate::Button<MyAction>);
/// // AppButton now implements Element and can be used with FynixCtx.
/// ```
#[macro_export]
macro_rules! register_element {
    ($vis:vis $new_type:ident, $inner:ty) => {
        $vis struct $new_type(pub $inner);

        $crate::typeslot::register!(
            $crate::element::ElementGroup,
            $new_type
        );

        impl ::core::ops::Deref for $new_type {
            type Target = $inner;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::core::ops::DerefMut for $new_type {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl $crate::element::ElementNew for $new_type {
            #[inline]
            fn new() -> Self {
                Self($crate::element::ElementNew::new())
            }
        }

        impl $crate::element::ElementChildren for $new_type {
            #[inline]
            fn children(
                &self,
            ) -> impl ::core::iter::IntoIterator<
                Item = &$crate::element::ElementId,
            >
            where
                Self: ::core::marker::Sized,
            {
                $crate::element::ElementChildren::children(&self.0)
            }
        }

        impl $crate::element::ElementBuild for $new_type {
            #[inline]
            fn constrain(
                &self,
                parent_constraint: $crate::rectree::Constraint,
            ) -> $crate::rectree::Constraint {
                $crate::element::ElementBuild::constrain(
                    &self.0,
                    parent_constraint,
                )
            }

            #[inline]
            fn build(
                &self,
                id: &$crate::element::ElementId,
                constraint: $crate::rectree::Constraint,
                nodes: &mut $crate::element::layout::ElementNodes,
            ) -> $crate::rectree::Size {
                $crate::element::ElementBuild::build(
                    &self.0,
                    id,
                    constraint,
                    nodes,
                )
            }

            #[inline]
            fn render(
                &self,
                id: &$crate::element::ElementId,
                painter: &mut dyn $crate::imaging::PaintSink,
                metas: &$crate::element::meta::ElementMetas,
            ) {
                $crate::element::ElementBuild::render(
                    &self.0,
                    id,
                    painter,
                    metas,
                )
            }
        }

        impl $crate::element::ElementTemplate for $new_type {}
    };
}
