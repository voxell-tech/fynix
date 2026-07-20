use fynix::imaging::kurbo::Stroke;
use fynix::prelude::*;
use fynix_elements::{Button, Frame, Label};
use fynix_interaction::prelude::PointerId;
use vello::peniko::color::palette::css;
use vello::peniko::{Brush, Color};

use crate::CadWorld;

pub(crate) const FONT: &[u8] =
    include_bytes!("../../assets/Inter-Regular.ttf");

pub(crate) mod colors {
    use super::{Color, css};

    pub(crate) const PANEL_BG: Color = Color::from_rgb8(45, 45, 48);
    pub(crate) const INACTIVE_BG: Color =
        Color::from_rgb8(58, 58, 62);
    pub(crate) const DISABLED_BG: Color =
        Color::from_rgb8(40, 40, 43);
    pub(crate) const HOVERED_INACTIVE_BG: Color =
        Color::from_rgb8(74, 74, 78);
    pub(crate) const SEPARATOR: Color = Color::from_rgb8(62, 62, 66);
    pub(crate) const ACTIVE_BG: Color = css::CORNFLOWER_BLUE;
    pub(crate) const HOVERED_ACTIVE_BG: Color =
        Color::from_rgb8(130, 179, 255);
    pub(crate) const ACCENT: Color = css::CORNFLOWER_BLUE;
    pub(crate) const MUTED_TEXT: Color = css::DIM_GRAY;
    pub(crate) const DISABLED_TEXT: Color = css::GRAY;
}

pub(crate) mod metrics {
    pub const CORNER_RADIUS: f64 = 4.0;
    pub const FONT_SIZE: f32 = 12.0;
    pub const HEADER_FONT: f32 = 10.0;
    pub const PAD_V: f32 = 8.0;
    pub const PAD_H: f32 = 10.0;
    pub const GAP: f32 = 8.0;
    pub const OUTER_PAD: f32 = 8.0;
    pub const GAP_V: f32 = 6.0;
}

pub(crate) use colors::*;
pub(crate) use metrics::*;

pub(crate) const DRAW_TOOLS: &[Tool] =
    &[Tool::Line, Tool::Rectangle, Tool::Circle, Tool::Arc];

pub(crate) const MODIFY_TOOLS: &[Tool] = &[Tool::Move, Tool::Erase];

pub(crate) type Mouse = PointerId<(), ()>;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub(crate) enum Tool {
    #[default]
    Select,
    Line,
    Rectangle,
    Circle,
    Arc,
    Move,
    Erase,
}

impl Tool {
    pub(crate) fn display_label(&self) -> &'static str {
        match self {
            Tool::Select => "Select (S)",
            Tool::Line => "Line (L)",
            Tool::Rectangle => "Rect",
            Tool::Circle => "Circle",
            Tool::Arc => "Arc",
            Tool::Move => "Move (M)",
            Tool::Erase => "Erase",
        }
    }

    pub(crate) fn hint(&self) -> &'static str {
        match self {
            Tool::Select => "Select, move, and modify objects",
            Tool::Line => "Click to place start point of a line",
            Tool::Rectangle => "Click to place a rectangle",
            Tool::Circle => "Click to place a circle",
            Tool::Arc => "Click to place an arc",
            Tool::Move => "Click to move selected objects",
            Tool::Erase => "Click to erase objects",
        }
    }

    pub(crate) fn from_key(ch: char) -> Option<Tool> {
        match ch.to_ascii_lowercase() {
            's' => Some(Tool::Select),
            'l' => Some(Tool::Line),
            'r' => Some(Tool::Rectangle),
            'c' => Some(Tool::Circle),
            'a' => Some(Tool::Arc),
            'm' => Some(Tool::Move),
            'e' => Some(Tool::Erase),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ToolbarState {
    pub active_tool: Tool,
    pub snap_enabled: bool,
    pub grid_enabled: bool,
    pub ortho_enabled: bool,
    pub expanded_dropdown: Option<DropdownId>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DropdownId {
    Draw,
    Modify,
}

pub(crate) fn apply_base(ctx: &mut FynixCtx<CadWorld>) {
    ctx.set(path!(<Label>::font_size), FONT_SIZE);
    ctx.set(path!(<Button>::corner_radius), CORNER_RADIUS);
    ctx.set(path!(<Button>::stroke), Stroke::new(0.0));
    ctx.set(path!(<Frame>::corner_radius), CORNER_RADIUS);
    ctx.set(path!(<Frame>::fill), Brush::Solid(SEPARATOR));
}

pub(crate) fn apply_menu_bar(ctx: &mut FynixCtx<CadWorld>) {
    ctx.set(path!(<Label>::font_size), HEADER_FONT);
    ctx.set(path!(<Label>::fill), Brush::Solid(MUTED_TEXT));
    ctx.set(path!(<Button>::fill), Brush::Solid(Color::TRANSPARENT));
    ctx.set(path!(<Button>::corner_radius), 0.0);
    ctx.set(path!(<Button>::stroke), Stroke::new(0.0));
}

pub(crate) fn apply_toolbar_toggle(ctx: &mut FynixCtx<CadWorld>) {
    ctx.set(path!(<Button>::corner_radius), 3.0);
}
