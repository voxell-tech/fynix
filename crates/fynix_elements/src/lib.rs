#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::Fynix;
use fynix::element::layout::ElementNodes;
use fynix::element::meta::ElementMetas;
use fynix::element::{
    Element, ElementBuild, ElementId, ElementTemplate,
};
use fynix::imaging::kurbo::{Affine, Stroke};
use fynix::imaging::peniko::{Brush, BrushRef, Color, Fill, Style};
use fynix::imaging::record::{Glyph, Scene, replay_transformed};
use fynix::imaging::{
    Composite, FillRef, GlyphRunRef, PaintSink, StrokeRef, kurbo,
};
use fynix::rectree::{Constraint, NodeContext, Size, Vec2};
use parley::style::StyleProperty;
use parley::{
    Alignment, AlignmentOptions, FontStyle, PositionedLayoutItem,
};
use parley::{FontContext, LayoutContext};

#[derive(Element, Debug, Clone, Copy)]
pub struct WindowSize {
    pub size: Size,
    #[element(children)]
    child: Option<ElementId>,
}

impl WindowSize {
    pub fn set_child(&mut self, id: ElementId) {
        self.child = Some(id);
    }
}

impl ElementBuild for WindowSize {
    fn constrain(
        &self,
        _parent_constraint: Constraint,
    ) -> Constraint {
        Constraint::loose(self.size)
    }

    fn build(
        &self,
        _id: &ElementId,
        _constraint: Constraint,
        _nodes: &mut ElementNodes,
    ) -> Size {
        self.size
    }
}

#[derive(Element, Debug, Clone)]
pub struct Horizontal {
    #[element(children)]
    children: Vec<ElementId>,
}

impl Horizontal {
    pub fn add(&mut self, id: ElementId) -> &mut Self {
        self.children.push(id);
        self
    }
}

impl ElementBuild for Horizontal {
    fn build(
        &self,
        _id: &ElementId,
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

#[derive(Element, Debug, Clone)]
pub struct Vertical {
    #[element(children)]
    children: Vec<ElementId>,
}

impl Vertical {
    pub fn add(&mut self, id: ElementId) -> &mut Self {
        self.children.push(id);
        self
    }
}

impl ElementBuild for Vertical {
    fn build(
        &self,
        _id: &ElementId,
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

#[derive(Element, Debug, Clone, Copy)]
pub struct Pad {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
    #[element(children)]
    child: Option<ElementId>,
}

impl Pad {
    pub fn set_child(&mut self, id: ElementId) {
        self.child = Some(id);
    }

    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left, child: None }
    }

    pub fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }

    /// Equal padding on top/bottom (`v`) and left/right (`h`).
    pub fn symmetric(v: f32, h: f32) -> Self {
        Self::new(v, h, v, h)
    }

    /// Padding on left and right only.
    pub fn horizontal(h: f32) -> Self {
        Self::new(0.0, h, 0.0, h)
    }

    /// Padding on top and bottom only.
    pub fn vertical(v: f32) -> Self {
        Self::new(v, 0.0, v, 0.0)
    }
}

impl ElementBuild for Pad {
    fn constrain(&self, parent_constraint: Constraint) -> Constraint {
        let h = self.left + self.right;
        let v = self.top + self.bottom;
        Constraint {
            min: Size::ZERO,
            max: Size::new(
                (parent_constraint.max.width - h).max(0.0),
                (parent_constraint.max.height - v).max(0.0),
            ),
        }
    }

    fn build(
        &self,
        _id: &ElementId,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size {
        let child_size = self
            .child
            .as_ref()
            .map(|id| {
                nodes.set_translation(
                    id,
                    Vec2::new(self.left, self.top),
                );
                nodes.get_size(id)
            })
            .unwrap_or_default();
        constraint.constrain(Size::new(
            child_size.width + self.left + self.right,
            child_size.height + self.top + self.bottom,
        ))
    }
}

#[derive(ElementTemplate)]
pub struct Button<A: 'static> {
    pub on_click: Option<A>,
    #[element(default = Brush::Solid(Color::BLACK))]
    pub fill: Brush,
    pub stroke: Stroke,
    #[element(default = Brush::Solid(Color::WHITE))]
    pub stroke_brush: Brush,
    pub corner_radius: f64,
    #[element(children)]
    pub child: Option<ElementId>,
}

impl<A> Button<A> {
    pub fn set_child(&mut self, id: ElementId) {
        self.child = Some(id);
    }

    pub fn set_on_click(&mut self, action: A) {
        self.on_click = Some(action);
    }
}

impl<A> ElementBuild for Button<A> {
    fn build(
        &self,
        _id: &ElementId,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size {
        constraint.constrain(
            self.child
                .as_ref()
                .map(|id| nodes.get_size(id))
                .unwrap_or_default(),
        )
    }

    fn render(
        &self,
        id: &ElementId,
        painter: &mut dyn PaintSink,
        metas: &ElementMetas,
    ) {
        let Some(meta) = metas.get(id) else { return };
        let pos = meta.node.world_translation;
        let size = meta.node.size;
        let shape = kurbo::RoundedRect::new(
            pos.x as f64,
            pos.y as f64,
            (pos.x + size.width) as f64,
            (pos.y + size.height) as f64,
            self.corner_radius,
        );
        painter.fill(FillRef::new(shape, &self.fill));
        painter.stroke(StrokeRef::new(
            shape,
            &self.stroke,
            &self.stroke_brush,
        ));
    }
}

#[derive(Element, Debug, Clone)]
pub struct Label {
    pub text: String,
    #[element(default = Brush::Solid(Color::WHITE))]
    pub fill: Brush,
    #[element(default = 16.0)]
    pub font_size: f32,
    pub font_style: FontStyle,
    pub alignment: Alignment,
}

impl ElementBuild for Label {
    fn build(
        &self,
        id: &ElementId,
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
            builder.push_default(StyleProperty::FontStyle(
                self.font_style,
            ));
            builder.push_default(StyleProperty::Brush(
                self.fill.clone(),
            ));

            let mut layout = builder.build(&self.text);
            let max_width = constraint
                .max
                .width
                .is_finite()
                .then_some(constraint.max.width);
            layout.break_all_lines(max_width);
            layout.align(
                max_width,
                self.alignment,
                AlignmentOptions::default(),
            );

            let mut scene = Scene::new();

            for line in layout.lines() {
                for item in line.items() {
                    let PositionedLayoutItem::GlyphRun(glyph_run) =
                        item
                    else {
                        continue;
                    };

                    let style = glyph_run.style();
                    let run = glyph_run.run();
                    let mut glyphs = glyph_run
                        .positioned_glyphs()
                        .map(|g| Glyph {
                            id: g.id,
                            x: g.x,
                            y: g.y,
                        });

                    scene.glyph_run(
                        GlyphRunRef {
                            font: run.font(),
                            transform: Affine::IDENTITY,
                            glyph_transform: None,
                            font_size: run.font_size(),
                            font_embolden: kurbo::Vec2::ZERO,
                            hint: false,
                            normalized_coords: run
                                .normalized_coords(),
                            style: &Style::Fill(Fill::NonZero),
                            brush: BrushRef::from(&style.brush),
                            composite: Composite::default(),
                        },
                        &mut glyphs,
                    );
                }
            }

            nodes.cache_scene(id, scene);
            Size::new(layout.width(), layout.height())
        } else {
            Size::ZERO
        };

        constraint.constrain(size)
    }

    fn render(
        &self,
        id: &ElementId,
        painter: &mut dyn PaintSink,
        metas: &ElementMetas,
    ) {
        let Some(meta) = metas.get(id) else { return };
        let Some(scene) = meta.cached_scene.as_ref() else {
            return;
        };
        let pos = meta.node.world_translation;
        let transform =
            Affine::translate((pos.x as f64, pos.y as f64));
        replay_transformed(scene, painter, transform);
    }
}

#[derive(Default, Clone)]
pub struct TextContext {
    pub font_cx: FontContext,
    pub layout_cx: LayoutContext<Brush>,
}

/// Initialize the resources needed for the elements in this crate to
/// work correctly.
pub fn init_resources(fynix: &mut Fynix) {
    fynix.resources.init::<TextContext>();
}
