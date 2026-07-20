#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::RectNodes;
use fynix::element::layout::ElementNodes;
use fynix::element::table::RenderElementTable;
use fynix::imaging::kurbo::{Affine, Stroke};
use fynix::imaging::peniko::{Brush, BrushRef, Color, Fill, Style};
use fynix::imaging::record::{Glyph, Scene, replay_transformed};
use fynix::imaging::{
    Composite, FillRef, GlyphRunRef, PaintSink, StrokeRef, kurbo,
};
use fynix::prelude::*;
pub use parley;
use parley::style::StyleProperty;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontStyle,
    LayoutContext, PositionedLayoutItem,
};

#[derive(Init, Element, Debug, Clone, Copy)]
pub struct WindowSize {
    pub size: Size,
    #[elem(children)]
    child: Option<ElementId>,
}

impl WindowSize {
    pub fn set_child(&mut self, id: impl Into<ElementId>) {
        self.child = Some(id.into());
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

#[derive(Init, Element, Debug, Clone)]
pub struct Horizontal {
    #[elem(children)]
    children: Vec<ElementId>,
}

impl Horizontal {
    pub fn add(&mut self, id: impl Into<ElementId>) -> &mut Self {
        self.children.push(id.into());
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

#[derive(Init, Element, Debug, Clone)]
pub struct Vertical {
    #[elem(children)]
    children: Vec<ElementId>,
}

impl Vertical {
    pub fn add(&mut self, id: impl Into<ElementId>) -> &mut Self {
        self.children.push(id.into());
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

#[derive(Init, Element, Debug, Clone, Copy)]
pub struct Pad {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
    #[elem(children)]
    child: Option<ElementId>,
}

impl Pad {
    pub fn set_child(&mut self, id: impl Into<ElementId>) {
        self.child = Some(id.into());
    }

    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
            child: None,
        }
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

#[derive(Init, Element)]
pub struct Button {
    #[init(Brush::Solid(Color::BLACK))]
    pub fill: Brush,
    pub stroke: Stroke,
    #[init(Brush::Solid(Color::WHITE))]
    pub stroke_brush: Brush,
    pub corner_radius: f64,
    #[elem(children)]
    pub child: Option<ElementId>,
}

impl Button {
    pub fn set_child(&mut self, id: impl Into<ElementId>) {
        self.child = Some(id.into());
    }
}

impl ElementBuild for Button {
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
        table: RenderElementTable,
    ) {
        let Some(node) = table.node(id) else { return };
        let pos = node.world_translation;
        let size = node.size;
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

#[derive(Init, Element)]
pub struct Frame {
    pub fill: Brush,
    pub corner_radius: f64,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
    #[elem(children)]
    pub child: Option<ElementId>,
}

impl Frame {
    pub fn set_child(&mut self, id: impl Into<ElementId>) {
        self.child = Some(id.into());
    }
}

impl ElementBuild for Frame {
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

    fn render(
        &self,
        id: &ElementId,
        painter: &mut dyn PaintSink,
        table: RenderElementTable,
    ) {
        let Some(node) = table.node(id) else { return };
        let pos = node.world_translation;
        let size = node.size;
        let shape = kurbo::RoundedRect::new(
            pos.x as f64,
            pos.y as f64,
            (pos.x + size.width) as f64,
            (pos.y + size.height) as f64,
            self.corner_radius,
        );
        painter.fill(FillRef::new(shape, &self.fill));
    }
}

#[derive(Init, Element, Debug, Clone)]
pub struct Label {
    pub text: String,
    #[init(Brush::Solid(Color::WHITE))]
    pub fill: Brush,
    #[init(16.0)]
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
                            brush_transform: None,
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
        table: RenderElementTable,
    ) {
        let Some(node) = table.node(id) else { return };
        let Some(scene) = table.scene(id) else {
            return;
        };
        let pos = node.world_translation;
        let transform =
            Affine::translate((pos.x as f64, pos.y as f64));
        replay_transformed(scene, painter, transform);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

impl Side {
    fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Center,
    Start,
    End,
}

#[derive(Init, Debug, Clone)]
pub struct Overlay {
    #[init(None)]
    pub content: Option<ElementId>,
    #[init(Default::default())]
    pub overlays: Vec<ElementId>,
    #[init(Align::Start)]
    pub h_align: Align,
    #[init(Align::Center)]
    pub v_align: Align,
    #[init(Vec2::ZERO)]
    pub offset: Vec2,
    #[init(false)]
    pub clip: bool,
    #[init(None)]
    pub anchor: Option<ElementId>,
    #[init(Side::Bottom)]
    pub side: Side,
    #[init(0.0)]
    pub gap: f32,
    #[init(true)]
    pub flip: bool,
}

impl Overlay {
    pub fn content(mut self, id: impl Into<ElementId>) -> Self {
        self.content = Some(id.into());
        self
    }

    pub fn overlay(mut self, id: impl Into<ElementId>) -> Self {
        self.overlays.push(id.into());
        self
    }

    fn cross_align(
        align: Align,
        anchor_start: f32,
        anchor_span: f32,
        child_span: f32,
    ) -> f32 {
        match align {
            Align::Start => anchor_start,
            Align::Center => {
                anchor_start + (anchor_span - child_span) / 2.0
            }
            Align::End => anchor_start + anchor_span - child_span,
        }
    }

    fn anchor_translation(
        side: Side,
        gap: f32,
        cross_align: Align,
        anchor_pos: Vec2,
        anchor_size: Size,
        child_size: Size,
    ) -> Vec2 {
        match side {
            Side::Top => Vec2::new(
                Self::cross_align(
                    cross_align,
                    anchor_pos.x,
                    anchor_size.width,
                    child_size.width,
                ),
                anchor_pos.y - child_size.height - gap,
            ),
            Side::Bottom => Vec2::new(
                Self::cross_align(
                    cross_align,
                    anchor_pos.x,
                    anchor_size.width,
                    child_size.width,
                ),
                anchor_pos.y + anchor_size.height + gap,
            ),
            Side::Left => Vec2::new(
                anchor_pos.x - child_size.width - gap,
                Self::cross_align(
                    cross_align,
                    anchor_pos.y,
                    anchor_size.height,
                    child_size.height,
                ),
            ),
            Side::Right => Vec2::new(
                anchor_pos.x + anchor_size.width + gap,
                Self::cross_align(
                    cross_align,
                    anchor_pos.y,
                    anchor_size.height,
                    child_size.height,
                ),
            ),
        }
    }

    fn compute_anchor_pos(
        &self,
        anchor_id: ElementId,
        overlay_id: ElementId,
        nodes: &ElementNodes,
    ) -> Option<(Vec2, Size)> {
        let anchor_node = nodes.get_node(&anchor_id)?;
        let size = anchor_node.size;
        let mut accumulated_pos = anchor_node.translation;
        let mut current_id = anchor_node.parent_id?;
        loop {
            if current_id == overlay_id {
                return Some((accumulated_pos, size));
            }
            let node = nodes.get_node(&current_id)?;
            accumulated_pos = accumulated_pos + node.translation;
            current_id = node.parent_id?;
        }
    }

    fn resolve_anchor_offset(
        &self,
        anchor_pos: Vec2,
        anchor_size: Size,
        overlay_size: Size,
        viewport: Option<Size>,
    ) -> Vec2 {
        let cross_align = match self.side {
            Side::Top | Side::Bottom => self.h_align,
            Side::Left | Side::Right => self.v_align,
        };
        let translation = Self::anchor_translation(
            self.side,
            self.gap,
            cross_align,
            anchor_pos,
            anchor_size,
            overlay_size,
        ) + self.offset;
        let Some(viewport_size) = viewport else {
            return translation;
        };
        let overflows = |pos: Vec2, child_size: Size| -> bool {
            pos.x < 0.0
                || pos.y < 0.0
                || pos.x + child_size.width > viewport_size.width
                || pos.y + child_size.height > viewport_size.height
        };
        if !overflows(translation, overlay_size) {
            return translation;
        }
        let flipped_translation = Self::anchor_translation(
            self.side.opposite(),
            self.gap,
            cross_align,
            anchor_pos,
            anchor_size,
            overlay_size,
        ) + self.offset;
        let best_candidate =
            if overflows(flipped_translation, overlay_size) {
                translation
            } else {
                flipped_translation
            };
        let clamp_x =
            (viewport_size.width - overlay_size.width).max(0.0);
        let clamp_y =
            (viewport_size.height - overlay_size.height).max(0.0);
        Vec2::new(
            best_candidate.x.max(0.0).min(clamp_x),
            best_candidate.y.max(0.0).min(clamp_y),
        )
    }
}

impl ElementChildren for Overlay {
    fn children(&self) -> impl IntoIterator<Item = &ElementId> {
        self.content.iter().chain(self.overlays.iter())
    }
}

impl ElementBuild for Overlay {
    fn constrain(&self, parent: Constraint) -> Constraint {
        parent
    }

    fn build(
        &self,
        id: &ElementId,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size {
        let content_size = self
            .content
            .as_ref()
            .map(|content_id| {
                nodes.set_translation(content_id, Vec2::ZERO);
                nodes.get_size(content_id)
            })
            .unwrap_or(Size::ZERO);

        let content_size = constraint.constrain(content_size);

        if self.overlays.is_empty() {
            return content_size;
        }

        let mut result = content_size;

        let anchor_data = self.anchor.and_then(|anchor_id| {
            self.compute_anchor_pos(anchor_id, *id, nodes)
        });
        let has_anchor = anchor_data.is_some();
        let viewport = if self.flip && has_anchor {
            Some(constraint.max)
        } else {
            None
        };

        for overlay_id in &self.overlays {
            let overlay_size = nodes.get_size(overlay_id);
            let translation = if let Some((pos, size)) = anchor_data {
                self.resolve_anchor_offset(
                    pos,
                    size,
                    overlay_size,
                    viewport,
                )
            } else {
                let align_x = Self::cross_align(
                    self.h_align,
                    0.0,
                    content_size.width,
                    overlay_size.width,
                );
                let align_y = Self::cross_align(
                    self.v_align,
                    0.0,
                    content_size.height,
                    overlay_size.height,
                );
                let mut pos =
                    Vec2::new(align_x, align_y) + self.offset;
                if self.clip {
                    let clamp_x = (content_size.width
                        - overlay_size.width)
                        .max(0.0);
                    let clamp_y = (content_size.height
                        - overlay_size.height)
                        .max(0.0);
                    pos = Vec2::new(
                        pos.x.max(0.0).min(clamp_x),
                        pos.y.max(0.0).min(clamp_y),
                    );
                }
                pos
            };
            nodes.set_translation(overlay_id, translation);
            if !has_anchor && !self.clip {
                result.width = result
                    .width
                    .max(translation.x + overlay_size.width);
                result.height = result
                    .height
                    .max(translation.y + overlay_size.height);
            }
        }

        constraint.constrain(result)
    }
}

#[derive(Default, Clone)]
pub struct TextContext {
    pub font_cx: FontContext,
    pub layout_cx: LayoutContext<Brush>,
}

/// Initialize the resources needed for the elements in this crate to
/// work correctly.
pub fn init_resources<W>(fynix: &mut Fynix<W>) {
    fynix.resources.init::<TextContext>();
}
