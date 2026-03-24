#![no_std]

extern crate alloc;

use field_path::field_accessor::FieldAccessor;
use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};

use crate::element::{Element, ElementId, Elements};
use crate::style::{StyleId, StyleValue, Styles};

pub mod any_wrapper;
pub mod element;
pub mod field_index;
pub mod style;
pub mod type_table;

mod id;

pub struct Fynix {
    pub tree: Rectree,
    pub elements: Elements,
    pub styles: Styles,
}

impl Fynix {
    pub fn new() -> Self {
        Self {
            tree: Rectree::new(),
            elements: Elements::new(),
            styles: Styles::new(),
        }
    }

    #[inline]
    pub const fn root_ctx(&mut self) -> BuildCtx<'_> {
        self.create_ctx(None)
    }

    #[inline]
    pub const fn create_ctx(
        &mut self,
        parent_style_id: Option<StyleId>,
    ) -> BuildCtx<'_> {
        BuildCtx {
            parent_style_id,
            elements: &mut self.elements,
            styles: &mut self.styles,
        }
    }
}

impl Default for Fynix {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutWorld for Fynix {
    fn get_solver(&self, _id: &NodeId) -> &dyn LayoutSolver {
        todo!()
    }
}

pub struct FynixCtx<'a, W> {
    pub fynix: &'a mut Fynix,
    pub world: &'a mut W,
}

// TODO: Merge into FynixCtx.
pub struct BuildCtx<'a> {
    parent_style_id: Option<StyleId>,
    elements: &'a mut Elements,
    styles: &'a mut Styles,
}

impl BuildCtx<'_> {
    #[must_use]
    pub fn add<E: Element>(&mut self) -> ElementId {
        let element = self.create_element::<E>();
        self.elements.add(element)
    }

    #[must_use]
    pub fn add_with<E: Element>(
        &mut self,
        f: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId {
        let mut element = self.create_element::<E>();
        let parent_style_id = self.parent_style_id;

        f(&mut element, self);

        // Restore parent style id from before the closure.
        self.parent_style_id = parent_style_id;

        self.elements.add(element)
    }

    pub fn set<E: Element, T: StyleValue>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) {
        self.styles.set(field_accessor, value);
    }

    /// Commits pending styles if needed, creates a new element (`E`),
    /// and applies styles to it.
    fn create_element<E: Element>(&mut self) -> E {
        if self.styles.should_commit() {
            let committed_id = self.styles.current_id();
            self.styles.commit_styles(self.parent_style_id);
            self.parent_style_id = Some(committed_id);
        }
        let mut element = E::new();
        if let Some(id) = &self.parent_style_id {
            self.styles.apply(&mut element, id);
        }
        element
    }
}

// #[cfg(test)]
// mod tests {
//     use field_path::field_accessor;

//     use crate::element::Horizontal;

//     pub use super::*;

//     #[test]
//     fn test() {
//         let mut fynix = Fynix::new();

//         let mut ctx = fynix.root_ctx();

//         type Frame = Horizontal;
//         let root_id = ctx.add_with::<Horizontal>(|e, ctx| {
//             ctx.styles.set(
//                 field_accessor!(<Horizontal>),
//                 Horizontal::new(),
//             );
//             e.add(ctx.add::<Frame>());
//             e.add(ctx.add::<Frame>());

//             ctx.styles.set(
//                 field_accessor!(<Horizontal>),
//                 Horizontal::new(),
//             );

//             e.add(ctx.add::<Frame>());
//             e.add(ctx.add::<Frame>());

//             e.add(ctx.add_with::<Frame>(|e, ctx| {
//                 ctx.styles.set(
//                     field_accessor!(<Horizontal>),
//                     Horizontal::new(),
//                 );
//                 e.add(ctx.add::<Frame>());
//                 e.add(ctx.add::<Frame>());
//                 e.add(ctx.add::<Frame>());
//                 e.add(ctx.add::<Frame>());
//             }));

//             e.add(ctx.add::<Frame>());
//         });
//     }
// }
