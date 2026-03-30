#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::element::{Element, ElementId};
use fynix::rectree::{Constraint, Layouter, Size, Vec2};

#[derive(Default, Debug, Clone)]
pub struct Horizontal {
    children: Vec<ElementId>,
}

impl Horizontal {
    pub fn add(&mut self, id: ElementId) -> &mut Self {
        self.children.push(id);
        self
    }
}

impl Element for Horizontal {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self::default()
    }

    fn children(&self) -> impl IntoIterator<Item = &ElementId>
    where
        Self: Sized,
    {
        // Showcasing the generic way of doing it.
        #[allow(clippy::into_iter_on_ref)]
        (&self.children).into_iter()
    }

    fn build(
        &self,
        _constraint: Constraint,
        layouter: &mut impl Layouter<Id = ElementId>,
    ) -> Size
    where
        Self: Sized,
    {
        let mut size = Size::ZERO;

        for child in self.children.iter() {
            let child_size = layouter.get_size(child);
            layouter.set_position(child, Vec2::new(size.width, 0.0));

            size.height = size.height.max(child_size.height);
            size.width += child_size.width;
        }

        size
    }
}

#[derive(Default, Debug, Clone)]
pub struct Vertical {
    children: Vec<ElementId>,
}

impl Vertical {
    pub fn add(&mut self, id: ElementId) -> &mut Self {
        self.children.push(id);
        self
    }
}

impl Element for Vertical {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self::default()
    }

    fn children(&self) -> impl IntoIterator<Item = &ElementId>
    where
        Self: Sized,
    {
        self.children.iter()
    }

    fn build(
        &self,
        _constraint: Constraint,
        layouter: &mut impl Layouter<Id = ElementId>,
    ) -> Size
    where
        Self: Sized,
    {
        let mut size = Size::ZERO;

        for child in self.children.iter() {
            let child_size = layouter.get_size(child);
            layouter.set_position(child, Vec2::new(0.0, size.height));

            size.width = size.width.max(child_size.width);
            size.height += child_size.height;
        }

        size
    }
}

#[derive(Default, Debug, Clone)]
pub struct Label {
    pub text: String,
}

impl Element for Label {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self::default()
    }

    fn build(
        &self,
        constraint: Constraint,
        _layouter: &mut impl Layouter<Id = ElementId>,
    ) -> Size
    where
        Self: Sized,
    {
        todo!()
    }
}
