#![no_std]

extern crate alloc;

use field_path::field_accessor::FieldAccessor;
use rectree::layout::{LayoutSolver, LayoutWorld};
use rectree::{NodeId, Rectree};

use crate::element::{Element, ElementId, Elements};
use crate::style::{StyleId, Styles};

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
        style_id: Option<StyleId>,
    ) -> BuildCtx<'_> {
        BuildCtx {
            last_style_id: style_id,
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

pub struct BuildCtx<'a> {
    last_style_id: Option<StyleId>,
    elements: &'a mut Elements,
    styles: &'a mut Styles,
}

impl BuildCtx<'_> {
    #[must_use]
    pub fn add<E>(&mut self) -> ElementId
    where
        E: Element,
    {
        if self.styles.should_commit() {
            self.styles.commit_styles(self.last_style_id);
            self.last_style_id = Some(self.styles.current_id());
        }

        let mut element = E::new();
        if let Some(id) = &self.last_style_id {
            self.styles.apply(&mut element, id);
        }
        self.elements.add(element)
    }

    #[must_use]
    pub fn add_with<E>(
        &mut self,
        f: impl FnOnce(&mut E, &mut Self),
    ) -> ElementId
    where
        E: Element,
    {
        if self.styles.should_commit() {
            self.styles.commit_styles(self.last_style_id);
            self.last_style_id = Some(self.styles.current_id());
        }
        let last_style_id = self.last_style_id;

        let mut element = E::new();
        if let Some(id) = &self.last_style_id {
            self.styles.apply(&mut element, id);
        }
        f(&mut element, self);

        // Retain last style id before we enter the closure.
        self.last_style_id = last_style_id;

        self.elements.add(element)
    }

    pub fn set<E, T>(
        &mut self,
        field_accessor: FieldAccessor<E, T>,
        value: T,
    ) where
        E: Element,
        T: Clone + 'static,
    {
        self.styles.set(field_accessor, value);
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::element::Horizontal;

//     pub use super::*;

//     #[test]
//     fn test() {
//         let mut fynix = Fynix::new();

//         let mut ctx = fynix.root_ctx();

//         let root_id = ctx.add_with::<Horizontal>(|e, ctx| {
//             // ctx.styles.set(field_accessor, value);
//         });
//     }
// }
