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
    pub fn add(&mut self, id: ElementId) {
        self.children.push(id);
    }
}

impl Element for Horizontal {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self::default()
    }
}

#[derive(Default, Debug, Clone)]
pub struct Vertical {
    children: Vec<ElementId>,
}

impl Vertical {
    pub fn add(&mut self, id: ElementId) {
        self.children.push(id);
    }
}

impl Element for Vertical {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self::default()
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
