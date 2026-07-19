#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use fynix::RectNodes;
use fynix::element::layout::ElementNodes;
use fynix::element::storage::ElementHandle;
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

        let clip = self.clip;
        let side = self.side;
        let gap = self.gap;
        let offset = self.offset;
        let h_align = self.h_align;
        let v_align = self.v_align;
        let cross = match side {
            Side::Top | Side::Bottom => h_align,
            Side::Left | Side::Right => v_align,
        };
        let anchor_data = self
            .anchor
            .and_then(|a| compute_anchor_pos(a, *id, nodes));
        let has_anchor = anchor_data.is_some();
        let viewport = if self.flip && has_anchor {
            nodes.get_resource::<Viewport>().copied()
        } else {
            None
        };

        for ov in &self.overlays {
            let ov_size = nodes.get_size(ov);
            let t = if let Some((pos, size)) = anchor_data {
                resolve_anchor_offset(
                    side, cross, gap, offset, pos, size, ov_size,
                    viewport,
                )
            } else {
                let p = alignment_offset(
                    h_align,
                    v_align,
                    content_size,
                    ov_size,
                ) + offset;
                if clip {
                    clamp_to_bounds(p, ov_size, content_size)
                } else {
                    p
                }
            };
            nodes.set_translation(ov, t);
            if !has_anchor && !clip {
                result.width = result.width.max(t.x + ov_size.width);
                result.height =
                    result.height.max(t.y + ov_size.height);
            }
        }

        constraint.constrain(result)
    }
}

fn alignment_offset(
    h_align: Align,
    v_align: Align,
    content: Size,
    child: Size,
) -> Vec2 {
    let x = match h_align {
        Align::Start => 0.0,
        Align::Center => (content.width - child.width) / 2.0,
        Align::End => content.width - child.width,
    };
    let y = match v_align {
        Align::Start => 0.0,
        Align::Center => (content.height - child.height) / 2.0,
        Align::End => content.height - child.height,
    };
    Vec2::new(x, y)
}

fn clamp_to_bounds(
    pos: Vec2,
    child_size: Size,
    bounds: Size,
) -> Vec2 {
    let max_x = (bounds.width - child_size.width).max(0.0);
    let max_y = (bounds.height - child_size.height).max(0.0);
    Vec2::new(pos.x.max(0.0).min(max_x), pos.y.max(0.0).min(max_y))
}

fn resolve_anchor_offset(
    side: Side,
    cross: Align,
    gap: f32,
    offset: Vec2,
    pos: Vec2,
    anchor_size: Size,
    ov_size: Size,
    viewport: Option<Viewport>,
) -> Vec2 {
    let t = anchor_translation(
        side,
        cross,
        gap,
        pos,
        anchor_size,
        ov_size,
    ) + offset;
    let Some(vp) = viewport else {
        return t;
    };
    if !anchor_overflows(t, ov_size, &vp) {
        return t;
    }
    let flipped = anchor_translation(
        side.opposite(),
        cross,
        gap,
        pos,
        anchor_size,
        ov_size,
    ) + offset;
    let candidate = if anchor_overflows(flipped, ov_size, &vp) {
        t
    } else {
        flipped
    };
    clamp_to_bounds(
        candidate,
        ov_size,
        Size::new(vp.width, vp.height),
    )
}

fn walk_to_root(
    start: Option<ElementId>,
    nodes: &ElementNodes,
) -> Vec2 {
    let mut pos = Vec2::ZERO;
    let mut current = match start {
        Some(id) => id,
        None => return Vec2::ZERO,
    };
    loop {
        let Some(node) = nodes.get_node(&current) else {
            break;
        };
        pos = pos + node.translation;
        match node.parent_id {
            Some(parent_id) => current = parent_id,
            None => break,
        }
    }
    pos
}

fn compute_anchor_pos(
    anchor_id: ElementId,
    overlay_id: ElementId,
    nodes: &ElementNodes,
) -> Option<(Vec2, Size)> {
    let anchor_node = nodes.get_node(&anchor_id)?;
    let size = anchor_node.size;
    let anchor_root = anchor_node.translation
        + walk_to_root(anchor_node.parent_id, nodes);
    let overlay_root = walk_to_root(Some(overlay_id), nodes);
    Some((
        Vec2::new(
            anchor_root.x - overlay_root.x,
            anchor_root.y - overlay_root.y,
        ),
        size,
    ))
}

fn anchor_translation(
    side: Side,
    cross: Align,
    gap: f32,
    anchor_pos: Vec2,
    anchor_size: Size,
    child_size: Size,
) -> Vec2 {
    let (main, cross_val) = match side {
        Side::Top => (
            anchor_pos.y - child_size.height - gap,
            cross_align(
                cross,
                anchor_pos.x,
                anchor_size.width,
                child_size.width,
            ),
        ),
        Side::Bottom => (
            anchor_pos.y + anchor_size.height + gap,
            cross_align(
                cross,
                anchor_pos.x,
                anchor_size.width,
                child_size.width,
            ),
        ),
        Side::Left => (
            cross_align(
                cross,
                anchor_pos.y,
                anchor_size.height,
                child_size.height,
            ),
            anchor_pos.x - child_size.width - gap,
        ),
        Side::Right => (
            cross_align(
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

fn anchor_overflows(
    pos: Vec2,
    size: Size,
    viewport: &Viewport,
) -> bool {
    pos.x < 0.0
        || pos.y < 0.0
        || pos.x + size.width > viewport.width
        || pos.y + size.height > viewport.height
}

pub struct OverlayComposer {
    content: Option<ElementId>,
    overlays: Vec<ElementId>,
    anchor: Option<ElementId>,
    side: Option<Side>,
    gap: Option<f32>,
    h_align: Option<Align>,
    v_align: Option<Align>,
    offset: Option<Vec2>,
    clip: Option<bool>,
    flip: Option<bool>,
}

impl OverlayComposer {
    pub fn new() -> Self {
        Self {
            content: None,
            overlays: Vec::new(),
            anchor: None,
            side: None,
            gap: None,
            h_align: None,
            v_align: None,
            offset: None,
            clip: None,
            flip: None,
        }
    }

    pub fn content(mut self, id: impl Into<ElementId>) -> Self {
        self.content = Some(id.into());
        self
    }

    pub fn overlay(mut self, id: impl Into<ElementId>) -> Self {
        self.overlays.push(id.into());
        self
    }

    pub fn anchor(mut self, id: impl Into<ElementId>) -> Self {
        self.anchor = Some(id.into());
        self
    }

    pub fn side(mut self, s: Side) -> Self {
        self.side = Some(s);
        self
    }

    pub fn gap(mut self, g: f32) -> Self {
        self.gap = Some(g);
        self
    }

    pub fn h_align(mut self, a: Align) -> Self {
        self.h_align = Some(a);
        self
    }

    pub fn v_align(mut self, a: Align) -> Self {
        self.v_align = Some(a);
        self
    }

    pub fn offset(mut self, v: Vec2) -> Self {
        self.offset = Some(v);
        self
    }

    pub fn clip(mut self, b: bool) -> Self {
        self.clip = Some(b);
        self
    }

    pub fn flip(mut self, b: bool) -> Self {
        self.flip = Some(b);
        self
    }
}

impl Default for OverlayComposer {
    fn default() -> Self {
        Self::new()
    }
}

impl<W> Composer<W> for OverlayComposer {
    type Style = Overlay;
    type Element = Overlay;

    fn compose(
        self,
        style: Self::Style,
        ctx: &mut FynixCtx<'_, '_, W>,
    ) -> ElementHandle<Overlay> {
        ctx.add_with::<Overlay>(move |o, _| {
            o.content = self.content;
            o.overlays = self.overlays;
            o.h_align = self.h_align.unwrap_or(style.h_align);
            o.v_align = self.v_align.unwrap_or(style.v_align);
            o.offset = self.offset.unwrap_or(style.offset);
            o.clip = self.clip.unwrap_or(style.clip);
            o.anchor = self.anchor.or(style.anchor);
            o.side = self.side.unwrap_or(style.side);
            o.gap = self.gap.unwrap_or(style.gap);
            o.flip = self.flip.unwrap_or(style.flip);
        })
        .handle()
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
