use fynix::element::layout::ElementNodes;
use fynix::element::storage::ElementHandle;
use fynix::element::table::RenderElementTable;
use fynix::imaging::{FillRef, PaintSink, StrokeRef};
use fynix::interaction::{Handler, HandlerFn};
use fynix::prelude::*;
use fynix_elements::{Button, Frame, Horizontal, Label, Pad};
use fynix_interaction::prelude::*;
use vello::peniko::{Brush, Color};

use crate::CadWorld;
use crate::helpers::{
    hovered_tool_changed, tool_enter_handler, tool_leave_handler,
    unified_fg, unified_fill,
};
use crate::theme::*;

/// Lays out toolbar at left edge and status bar at right edge of
/// the full content width.
#[derive(Init, Element)]
pub(crate) struct ToolbarRow {
    #[elem(children)]
    pub(crate) children: Vec<ElementId>,
}

impl ElementBuild for ToolbarRow {
    fn build(
        &self,
        _id: &ElementId,
        constraint: Constraint,
        nodes: &mut ElementNodes,
    ) -> Size {
        debug_assert!(self.children.len() <= 2);
        let width = constraint.max.width;
        let tb_size = self
            .children
            .first()
            .map(|id| nodes.get_size(id))
            .unwrap_or(Size::ZERO);
        let sb_size = self
            .children
            .get(1)
            .map(|id| nodes.get_size(id))
            .unwrap_or(Size::ZERO);
        if let Some(id) = self.children.first() {
            nodes.set_translation(id, Vec2::new(0.0, 0.0));
        }
        if let Some(id) = self.children.get(1) {
            nodes.set_translation(
                id,
                Vec2::new(width - sb_size.width, 0.0),
            );
        }
        let height = tb_size.height.max(sb_size.height);
        constraint.constrain(Size::new(width, height))
    }
}

#[derive(Init)]
pub(crate) struct IconLabelButtonStyle {
    #[init(ACCENT)]
    accent_color: Color,
    #[init(CORNER_RADIUS)]
    corner_radius: f64,
    #[init(FONT_SIZE)]
    font_size: f32,
    #[init(PAD_V)]
    pad_v: f32,
    #[init(PAD_H)]
    pad_h: f32,
}

pub(crate) struct IconLabelButton {
    pub label: String,
    pub icon_text: Option<String>,
    pub tool_icon: Option<Tool>,
    pub accent: bool,
    pub fill: Brush,
    pub fg: Brush,
    pub enabled: bool,
    pub on_click: Option<Handler<PrimaryClick<Mouse>, CadWorld>>,
    pub on_enter: Option<Handler<PointerEnter<Mouse>, CadWorld>>,
    pub on_leave: Option<Handler<PointerLeave<Mouse>, CadWorld>>,
}

impl IconLabelButton {
    fn build_content(
        ctx: &mut FynixCtx<'_, '_, CadWorld>,
        style: &IconLabelButtonStyle,
        accent: bool,
        tool_icon: Option<Tool>,
        icon_text: &Option<String>,
        label: String,
        fg: Brush,
    ) -> ElementId {
        ctx.add_with::<Horizontal>(|h, ctx| {
            if accent {
                h.add(ctx.add_with::<Frame>(|bar, _| {
                    bar.fill = Brush::Solid(style.accent_color);
                    bar.corner_radius = 1.5;
                    bar.top = 2.0;
                    bar.bottom = 2.0;
                    bar.left = 1.0;
                    bar.right = 2.0;
                }));
                h.add(ctx.add_with::<Pad>(|sp, _| {
                    *sp = Pad::horizontal(1.0);
                }));
            }

            if let Some(tool) = tool_icon {
                h.add(ctx.add_with::<Pad>(|dot, _| {
                    *dot = Pad::horizontal(2.0);
                }));
                h.add(ctx.add_with::<ToolIcon>(|icon, _| {
                    icon.tool = Some(tool);
                    icon.color = fg.clone();
                    icon.size = style.font_size as f64 + 2.0;
                }));
                h.add(ctx.add_with::<Pad>(|sep, _| {
                    *sep = Pad::horizontal(4.0);
                }));
            } else if let Some(text) = icon_text {
                h.add(ctx.add_with::<Label>(|l, _| {
                    l.text = text.clone();
                    l.font_size = style.font_size;
                    l.fill = fg.clone();
                }));
                h.add(ctx.add_with::<Pad>(|sp, _| {
                    *sp = Pad::horizontal(3.0);
                }));
            }

            h.add(ctx.add_with::<Label>(|l, _| {
                l.text = label;
                l.font_size = style.font_size;
                l.fill = fg;
            }));
        })
        .id()
    }
}

impl Composer<CadWorld> for IconLabelButton {
    type Style = IconLabelButtonStyle;
    type Element = Button;

    fn compose(
        self,
        style: Self::Style,
        ctx: &mut FynixCtx<'_, '_, CadWorld>,
    ) -> ElementHandle<Button> {
        let IconLabelButton {
            label,
            icon_text,
            tool_icon,
            accent,
            fill,
            fg,
            enabled,
            on_click,
            on_enter,
            on_leave,
        } = self;

        let content_id = Self::build_content(
            ctx, &style, accent, tool_icon, &icon_text, label, fg,
        );

        let mut button = ctx.add_with::<Button>(|b, ctx| {
            b.fill = fill;
            b.corner_radius = style.corner_radius;
            b.set_child(ctx.add_with::<Pad>(|p, _ctx| {
                *p = Pad::symmetric(style.pad_v, style.pad_h);
                p.set_child(content_id);
            }));
        });

        if enabled && let Some(on_click) = on_click {
            button = button.interact_raw(on_click);
        }
        if let Some(on_enter) = on_enter {
            button = button.interact_raw(on_enter);
        }
        if let Some(on_leave) = on_leave {
            button = button.interact_raw(on_leave);
        }

        button.handle()
    }
}

#[derive(Init, Element, Debug, Clone)]
pub(crate) struct ToolIcon {
    pub tool: Option<Tool>,
    pub color: Brush,
    pub size: f64,
}

impl ElementBuild for ToolIcon {
    fn build(
        &self,
        _id: &ElementId,
        constraint: Constraint,
        _nodes: &mut ElementNodes,
    ) -> Size {
        constraint
            .constrain(Size::new(self.size as f32, self.size as f32))
    }

    fn render(
        &self,
        id: &ElementId,
        painter: &mut dyn PaintSink,
        table: RenderElementTable,
    ) {
        use fynix::imaging::kurbo::{
            BezPath, Circle, RoundedRect, Shape, Stroke,
        };

        let Some(node) = table.node(id) else { return };
        let Some(tool) = self.tool else { return };
        let pos = node.world_translation;
        let sz = node.size;
        let cx = (pos.x + sz.width / 2.0) as f64;
        let cy = (pos.y + sz.height / 2.0) as f64;
        let r = (sz.width.min(sz.height) / 2.0 - 2.0).max(2.0) as f64;
        let stroke = Stroke::new(1.5);

        match tool {
            Tool::Select => {
                let mut path = BezPath::new();
                path.move_to((cx - r * 0.5, cy + r * 0.7));
                path.line_to((cx, cy - r * 0.7));
                path.line_to((cx + r * 0.5, cy + r * 0.7));
                painter.stroke(StrokeRef::new(
                    path,
                    &Stroke::new(2.0),
                    &self.color,
                ));
            }
            Tool::Line => {
                let mut path = BezPath::new();
                path.move_to((cx - r * 0.6, cy + r * 0.6));
                path.line_to((cx + r * 0.6, cy - r * 0.6));
                painter.stroke(StrokeRef::new(
                    path,
                    &stroke,
                    &self.color,
                ));
            }
            Tool::Rectangle => {
                painter.stroke(StrokeRef::new(
                    RoundedRect::new(
                        cx - r * 0.6,
                        cy - r * 0.6,
                        cx + r * 0.6,
                        cy + r * 0.6,
                        1.0,
                    ),
                    &stroke,
                    &self.color,
                ));
            }
            Tool::Circle => {
                let path =
                    Circle::new((cx, cy), r * 0.6).into_path(0.1);
                painter.stroke(StrokeRef::new(
                    path,
                    &stroke,
                    &self.color,
                ));
            }
            Tool::Arc => {
                let mut path = BezPath::new();
                let l = r * 0.5;
                path.move_to((cx - l, cy + l));
                let k = 0.5522847498;
                path.curve_to(
                    (cx - l, cy + l - l * k),
                    (cx - l + l * k, cy - l),
                    (cx + l, cy - l),
                );
                painter.stroke(StrokeRef::new(
                    path,
                    &stroke,
                    &self.color,
                ));
            }
            Tool::Move => {
                for (dx, dy) in [
                    (-0.35, 0.0),
                    (0.35, 0.0),
                    (0.0, -0.35),
                    (0.0, 0.35),
                ] {
                    let path =
                        Circle::new((cx + dx * r, cy + dy * r), 2.0)
                            .into_path(0.1);
                    painter.fill(FillRef::new(path, &self.color));
                }
            }
            Tool::Erase => {
                let inner_r = r * 0.55;
                painter.stroke(StrokeRef::new(
                    RoundedRect::new(
                        cx - inner_r,
                        cy - inner_r,
                        cx + inner_r,
                        cy + inner_r,
                        1.0,
                    ),
                    &stroke,
                    &self.color,
                ));
                painter.stroke(StrokeRef::new(
                    {
                        let mut p = BezPath::new();
                        p.move_to((cx - inner_r, cy - inner_r));
                        p.line_to((cx + inner_r, cy + inner_r));
                        p
                    },
                    &Stroke::new(1.0),
                    &self.color,
                ));
                painter.stroke(StrokeRef::new(
                    {
                        let mut p = BezPath::new();
                        p.move_to((cx + inner_r, cy - inner_r));
                        p.line_to((cx - inner_r, cy + inner_r));
                        p
                    },
                    &Stroke::new(1.0),
                    &self.color,
                ));
            }
        }
    }
}

pub(crate) struct ToolButton {
    label: String,
    is_active: bool,
    tool: Option<Tool>,
    on_click: Option<Handler<PrimaryClick<Mouse>, CadWorld>>,
}

impl ToolButton {
    pub(crate) fn new(
        label: impl Into<String>,
        is_active: bool,
    ) -> Self {
        Self {
            label: label.into(),
            is_active,
            tool: None,
            on_click: None,
        }
    }

    pub(crate) fn for_tool(tool: Tool, is_active: bool) -> Self {
        Self {
            label: tool.display_label().into(),
            is_active,
            tool: Some(tool),
            on_click: None,
        }
    }

    pub(crate) fn on_click(
        mut self,
        handler: impl HandlerFn<PrimaryClick<Mouse>, CadWorld>,
    ) -> Self {
        self.on_click = Some(handler.into());
        self
    }
}

impl Composer<CadWorld> for ToolButton {
    type Style = IconLabelButtonStyle;
    type Element = Button;

    fn compose(
        self,
        _style: Self::Style,
        ctx: &mut FynixCtx<'_, '_, CadWorld>,
    ) -> ElementHandle<Button> {
        let ToolButton {
            label,
            is_active,
            tool,
            on_click,
        } = self;

        let hovered_tool = ctx.world.hovered_tool;
        let hovering = tool.is_some() && hovered_tool == tool;

        let handle = ctx.compose(IconLabelButton {
            label,
            icon_text: None,
            tool_icon: tool,
            accent: tool.is_some() && is_active,
            fill: unified_fill(is_active, hovering, true),
            fg: unified_fg(true),
            enabled: true,
            on_click,
            on_enter: tool.map(tool_enter_handler),
            on_leave: tool.is_some().then(tool_leave_handler),
        });
        if let Some(t) = tool {
            handle
                .bind(
                    hovered_tool_changed(hovered_tool),
                    move |w| {
                        let hovering = w.hovered_tool == Some(t);
                        unified_fill(is_active, hovering, true)
                    },
                    |b: &mut Button| &mut b.fill,
                )
                .handle()
        } else {
            handle.handle()
        }
    }
}

#[derive(Init)]
pub(crate) struct MenuBarItemStyle;

pub(crate) struct MenuBarItem {
    label: String,
}

impl MenuBarItem {
    pub(crate) fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

impl Composer<CadWorld> for MenuBarItem {
    type Style = MenuBarItemStyle;
    type Element = Button;

    fn compose(
        self,
        _style: MenuBarItemStyle,
        ctx: &mut FynixCtx<'_, '_, CadWorld>,
    ) -> ElementHandle<Button> {
        let label_id = ctx
            .add_with::<Label>(|l, _| {
                l.text = self.label;
            })
            .id();
        ctx.add_with::<Button>(|b, ctx| {
            b.set_child(
                ctx.add_with::<Pad>(|p, _| {
                    *p = Pad::symmetric(2.0, 6.0);
                    p.set_child(label_id);
                })
                .id(),
            );
        })
        .handle()
    }
}
