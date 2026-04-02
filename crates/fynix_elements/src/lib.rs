#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::element::{Element, ElementId, ElementNodes};
use fynix::rectree::{Constraint, NodeContext, Size, Vec2};
use parley::style::StyleProperty;
use parley::{FontContext, FontFamily, LayoutContext};

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
        // TODO: Refer to this when creating the #[derive(Element)]
        // macro. And remove it after that.

        // Showcasing the generic way of doing it.
        #[allow(clippy::into_iter_on_ref)]
        (&self.children).into_iter()
    }

    fn build(
        &self,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size
    where
        Self: Sized,
    {
        let mut size = Size::ZERO;

        for child in self.children.iter() {
            let child_size = nodes.get_size(child);
            nodes.set_translation(child, Vec2::new(size.width, 0.0));

            size.height = size.height.max(child_size.height);
            size.width += child_size.width;
        }

        constraint.constrain(size)
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
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size
    where
        Self: Sized,
    {
        let mut size = Size::ZERO;

        for child in self.children.iter() {
            let child_size = nodes.get_size(child);
            nodes.set_translation(child, Vec2::new(0.0, size.height));

            size.width = size.width.max(child_size.width);
            size.height += child_size.height;
        }

        constraint.constrain(size)
    }
}

#[derive(Default, Debug, Clone)]
pub struct Label {
    pub text: String,
    pub font_size: f32,
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
        nodes: &mut ElementNodes,
    ) -> Size
    where
        Self: Sized,
    {
        let size = if let Some(TextContext { font_cx, layout_cx }) =
            nodes.get_resource_mut::<TextContext>()
        {
            let mut builder = layout_cx
                .ranged_builder(font_cx, &self.text, 1.0, false);
            builder.push_default(StyleProperty::FontSize(
                self.font_size,
            ));
            // builder.push_default(parley::GenericFamily::Serif);
            let mut layout = builder.build(&self.text);
            layout.break_all_lines(None);
            Size::new(layout.width(), layout.height())
        } else {
            Size::ZERO
        };

        constraint.constrain(size)
    }
}

#[derive(Default, Clone)]
pub struct TextContext {
    pub font_cx: FontContext,
    pub layout_cx: LayoutContext,
}
