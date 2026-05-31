use imaging::PaintSink;
use rectree::{Constraint, Size};

use crate::element::layout::ElementNodes;
use crate::element::meta::ElementMetas;
use crate::init::Init;

pub mod layout;
pub mod meta;
pub mod storage;
pub mod table;

pub use fynix_macros::Element;
pub use storage::{ElementId, Elements};
pub use table::ElementTable;

/// Enumerates the children of an element.
///
/// Derived by `#[derive(Element)]`, iterates the field tagged
/// `#[children]`, or the fn given in `#[element(children = my_fn)]`.
///
/// Defaults to no children.
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
    /// Child elements are rendered by the tree walker after this
    /// method returns, do not recurse into children here.
    ///
    /// The default implementation is a no-op, suitable for purely
    /// structural elements.
    #[expect(unused_variables)]
    fn render(
        &self,
        id: &ElementId,
        painter: &mut dyn PaintSink,
        metas: &ElementMetas,
    ) {
    }
}

/// Marker trait for element types.
///
/// Use `#[derive(Element)]` to implement this and the associated
/// supertraits automatically.
pub trait Element:
    Init + ElementChildren + ElementBuild + 'static
{
}

impl<T> Element for T where
    T: Init + ElementChildren + ElementBuild + 'static
{
}
