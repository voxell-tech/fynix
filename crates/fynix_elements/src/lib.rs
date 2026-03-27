#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::element::{Element, ElementId};

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
}
