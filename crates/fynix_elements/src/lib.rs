#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::Fynix;
use fynix::element::meta::ElementMetas;
use fynix::element::{
    Element, ElementBuild, ElementId, ElementNodes,
};
use fynix::imaging::kurbo::Affine;
use fynix::imaging::peniko::{Brush, BrushRef, Color, Fill, Style};
use fynix::imaging::record::{Glyph, Scene, replay_transformed};
use fynix::imaging::{Composite, GlyphRunRef, PaintSink, kurbo};
use fynix::rectree::{Constraint, NodeContext, Size, Vec2};
use parley::style::StyleProperty;
use parley::{
    Alignment, AlignmentOptions, FontStyle, PositionedLayoutItem,
};
use parley::{FontContext, LayoutContext};

#[derive(Element, Default, Debug, Clone, Copy)]
pub struct WindowSize {
    pub size: Size,
    #[children]
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

#[derive(Element, Default, Debug, Clone)]
pub struct Horizontal {
    #[children]
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

#[derive(Element, Default, Debug, Clone)]
pub struct Vertical {
    #[children]
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

#[derive(Element, Debug, Clone)]
pub struct Label {
    pub text: String,
    pub fill: Brush,
    pub font_size: f32,
    pub font_style: FontStyle,
    pub alignment: Alignment,
}

impl Default for Label {
    fn default() -> Self {
        Self {
            text: String::new(),
            fill: Brush::Solid(Color::WHITE),
            font_size: 16.0,
            font_style: Default::default(),
            alignment: Default::default(),
        }
    }
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
