#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use fynix::Fynix;
use fynix::element::{Element, ElementId, ElementNodes};
use fynix::rectree::{Constraint, NodeContext, Size, Vec2};
use imaging::Composite;
use imaging::GlyphRunRef;
use imaging::PaintSink;
use imaging::record::Glyph as ImagingGlyph;
use kurbo::Affine;
use parley::PositionedLayoutItem;
use parley::style::StyleProperty;
use parley::{FontContext, Layout, LayoutContext};
use peniko::BrushRef;
use peniko::Color;
use peniko::Fill;
use peniko::Style;

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
    ) -> Size {
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
    ) -> Size {
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

#[derive(Clone)]
pub struct Label {
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    // Parley's default brush type is `[u8; 4]` (RGBA). We only use
    // the layout for glyph positions; the actual color comes from
    // `self.color` at render time via imaging.
    layout_cache: RefCell<Option<Layout<[u8; 4]>>>,
}

impl core::fmt::Debug for Label {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        f.debug_struct("Label")
            .field("text", &self.text)
            .field("font_size", &self.font_size)
            .field("color", &self.color)
            .finish_non_exhaustive()
    }
}

impl Element for Label {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            text: String::new(),
            font_size: 16.0,
            color: Color::BLACK,
            layout_cache: RefCell::new(None),
        }
    }

    fn build(
        &self,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size {
        let size = if let Some(TextContext { font_cx, layout_cx }) =
            nodes.get_resource_mut::<TextContext>()
        {
            let mut builder = layout_cx
                .ranged_builder(font_cx, &self.text, 1.0, false);
            builder.push_default(StyleProperty::FontSize(
                self.font_size,
            ));
            let mut layout = builder.build(&self.text);
            layout.break_all_lines(None);
            let size = Size::new(layout.width(), layout.height());
            *self.layout_cache.borrow_mut() = Some(layout);
            size
        } else {
            Size::ZERO
        };

        constraint.constrain(size)
    }

    fn render(
        &self,
        painter: &mut dyn PaintSink,
        pos: Vec2,
        _size: Size,
    ) {
        let cache = self.layout_cache.borrow();
        let Some(layout) = cache.as_ref() else { return };

        let transform =
            Affine::translate((pos.x as f64, pos.y as f64));
        let style = Style::Fill(Fill::NonZero);

        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item
                else {
                    continue;
                };
                let run = glyph_run.run();
                let mut glyphs =
                    glyph_run.positioned_glyphs().map(|g| {
                        ImagingGlyph {
                            id: g.id,
                            x: g.x,
                            y: g.y,
                        }
                    });
                painter.glyph_run(
                    GlyphRunRef {
                        font: run.font(),
                        transform,
                        glyph_transform: None,
                        font_size: run.font_size(),
                        hint: false,
                        normalized_coords: run.normalized_coords(),
                        style: &style,
                        brush: BrushRef::Solid(self.color),
                        composite: Composite::default(),
                    },
                    &mut glyphs,
                );
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct TextContext {
    pub font_cx: FontContext,
    pub layout_cx: LayoutContext,
}

/// Initialize the resources needed for the elements in this crate to
/// work correctly.
pub fn init_resources(fynix: &mut Fynix) {
    fynix.resources_mut().init::<TextContext>();
}
