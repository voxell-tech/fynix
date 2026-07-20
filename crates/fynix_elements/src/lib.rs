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
        cross: Align,
        anchor_pos: Vec2,
        anchor_size: Size,
        child_size: Size,
    ) -> Vec2 {
        let (main, cross_val) = match side {
            Side::Top => (
                anchor_pos.y - child_size.height - gap,
                Self::cross_align(
                    cross,
                    anchor_pos.x,
                    anchor_size.width,
                    child_size.width,
                ),
            ),
            Side::Bottom => (
                anchor_pos.y + anchor_size.height + gap,
                Self::cross_align(
                    cross,
                    anchor_pos.x,
                    anchor_size.width,
                    child_size.width,
                ),
            ),
            Side::Left => (
                Self::cross_align(
                    cross,
                    anchor_pos.y,
                    anchor_size.height,
                    child_size.height,
                ),
                anchor_pos.x - child_size.width - gap,
            ),
            Side::Right => (
                Self::cross_align(
                    cross,
                    anchor_pos.y,
                    anchor_size.height,
                    child_size.height,
                ),
                anchor_pos.x + anchor_size.width + gap,
            ),
        };
        match side {
            Side::Top | Side::Bottom => Vec2::new(cross_val, main),
            Side::Left | Side::Right => Vec2::new(main, cross_val),
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
        let mut pos = anchor_node.translation;
        let mut current = anchor_node.parent_id?;
        loop {
            if current == overlay_id {
                return Some((pos, size));
            }
            let node = nodes.get_node(&current)?;
            pos = pos + node.translation;
            current = node.parent_id?;
        }
    }

    fn resolve_anchor_offset(
        &self,
        pos: Vec2,
        anchor_size: Size,
        ov_size: Size,
        viewport: Option<Viewport>,
    ) -> Vec2 {
        let cross = match self.side {
            Side::Top | Side::Bottom => self.h_align,
            Side::Left | Side::Right => self.v_align,
        };
        let t = Self::anchor_translation(
            self.side,
            self.gap,
            cross,
            pos,
            anchor_size,
            ov_size,
        ) + self.offset;
        let Some(vp) = viewport else {
            return t;
        };
        let overflows = |p: Vec2, sz: Size| -> bool {
            p.x < 0.0
                || p.y < 0.0
                || p.x + sz.width > vp.width
                || p.y + sz.height > vp.height
        };
        if !overflows(t, ov_size) {
            return t;
        }
        let flipped = Self::anchor_translation(
            self.side.opposite(),
            self.gap,
            cross,
            pos,
            anchor_size,
            ov_size,
        ) + self.offset;
        let candidate = if overflows(flipped, ov_size) {
            t
        } else {
            flipped
        };
        let max_x = (vp.width - ov_size.width).max(0.0);
        let max_y = (vp.height - ov_size.height).max(0.0);
        Vec2::new(
            candidate.x.max(0.0).min(max_x),
            candidate.y.max(0.0).min(max_y),
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
            .map(|c| {
                nodes.set_translation(c, Vec2::ZERO);
                nodes.get_size(c)
            })
            .unwrap_or(Size::ZERO);

        let content_size = constraint.constrain(content_size);

        if self.overlays.is_empty() {
            return constraint.constrain(content_size);
        }

        let mut result = content_size;

        let anchor_data = self
            .anchor
            .and_then(|a| self.compute_anchor_pos(a, *id, nodes));
        let has_anchor = anchor_data.is_some();
        let viewport = if self.flip && has_anchor {
            nodes.get_resource::<Viewport>().copied()
        } else {
            None
        };

        for ov in &self.overlays {
            let ov_size = nodes.get_size(ov);
            let t = if let Some((pos, size)) = anchor_data {
                self.resolve_anchor_offset(
                    pos, size, ov_size, viewport,
                )
            } else {
                let x = match self.h_align {
                    Align::Start => 0.0,
                    Align::Center => {
                        (content_size.width - ov_size.width) / 2.0
                    }
                    Align::End => content_size.width - ov_size.width,
                };
                let y = match self.v_align {
                    Align::Start => 0.0,
                    Align::Center => {
                        (content_size.height - ov_size.height) / 2.0
                    }
                    Align::End => {
                        content_size.height - ov_size.height
                    }
                };
                let mut p = Vec2::new(x, y) + self.offset;
                if self.clip {
                    let max_x =
                        (content_size.width - ov_size.width).max(0.0);
                    let max_y = (content_size.height
                        - ov_size.height)
                        .max(0.0);
                    p = Vec2::new(
                        p.x.max(0.0).min(max_x),
                        p.y.max(0.0).min(max_y),
                    );
                }
                p
            };
            nodes.set_translation(ov, t);
            if !has_anchor && !self.clip {
                result.width = result.width.max(t.x + ov_size.width);
                result.height =
                    result.height.max(t.y + ov_size.height);
            }
        }

        constraint.constrain(result)
    }
}

/// Viewport bounds for overflow detection and flip behavior.
/// Stored as a resource; read automatically by Overlay during build.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
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
