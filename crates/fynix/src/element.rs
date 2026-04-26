use imaging::PaintSink;
use rectree::{Constraint, Size};
use typeslot::{SlotGroup, TypeSlot};

use crate::element::layout::ElementNodes;
use crate::element::meta::ElementMetas;

pub use fynix_macros::{Element, ElementSlot, ElementTemplate};

pub mod layout;
pub mod meta;
pub mod storage;
pub mod table;

pub use storage::{
    _ElementMarker, ElementId, ElementIdGenerator, Elements,
};
pub use table::ElementTable;

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
