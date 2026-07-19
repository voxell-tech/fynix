use core::cell::Cell;

use fynix::interaction::Handler;
use fynix::prelude::*;
use fynix_elements::{Button, Frame, Label, Pad};
use fynix_interaction::prelude::*;
use vello::peniko::{Brush, Color};

use crate::CadWorld;
use crate::theme::*;

pub(crate) fn cell_diff<T: PartialEq + Copy>(
    prev: &Cell<T>,
    current: T,
) -> bool {
    let changed = prev.get() != current;
    prev.set(current);
    changed
}

pub(crate) fn add_separator(
    ctx: &mut FynixCtx<CadWorld>,
) -> ElementId {
    ctx.add_with::<Frame>(|line, _| {
        line.fill = Brush::Solid(SEPARATOR);
        line.corner_radius = 0.0;
        line.top = 0.5;
        line.bottom = 0.5;
    })
    .id()
}

pub(crate) fn add_vertical_separator(
    ctx: &mut FynixCtx<CadWorld>,
) -> ElementId {
    ctx.add_with::<Frame>(|f, _| {
        f.fill = Brush::Solid(SEPARATOR);
        f.corner_radius = 0.0;
        f.top = 0.0;
        f.bottom = 0.0;
        f.left = 0.5;
        f.right = 0.5;
    })
    .id()
}

pub(crate) fn vertical_separator(
    ctx: &mut FynixCtx<CadWorld>,
) -> ElementId {
    let line = add_vertical_separator(ctx);

    ctx.add_with::<Pad>(|p, _| {
        p.top = 4.0;
        p.bottom = 4.0;
        p.left = 6.0;
        p.right = 6.0;
        p.set_child(line);
    })
    .id()
}

pub(crate) fn panel_frame(
    ctx: &mut FynixCtx<CadWorld>,
    content: ElementId,
) -> ElementId {
    ctx.add_with::<Frame>(|bg, _| {
        bg.fill = Brush::Solid(PANEL_BG);
        bg.top = GAP_V;
        bg.bottom = GAP_V;
        bg.left = OUTER_PAD;
        bg.right = OUTER_PAD;
        bg.set_child(content);
    })
    .id()
}

pub(crate) fn dismiss_dropdown_handler()
-> Handler<PrimaryClick<Mouse>, CadWorld> {
    Handler::<PrimaryClick<Mouse>, _>::new(
        |_, res: &mut Response<'_, CadWorld>| {
            res.expanded_dropdown = None;
        },
    )
}

pub(crate) fn dismiss_on_click(
    ctx: &mut FynixCtx<CadWorld>,
    content: ElementId,
) -> ElementId {
    ctx.add_with::<Button>(|b, _| {
        b.fill = Brush::Solid(Color::TRANSPARENT);
        b.corner_radius = 0.0;
        b.set_child(content);
    })
    .interact_raw(dismiss_dropdown_handler())
    .id()
}

pub(crate) fn hovered_tool_changed(
    initial: Option<Tool>,
) -> impl Fn(&CadWorld) -> bool + 'static {
    let prev = Cell::new(initial);
    move |w: &CadWorld| cell_diff(&prev, w.hovered_tool)
}

pub(crate) fn hint_changed(
    world: &CadWorld,
) -> impl Fn(&CadWorld) -> bool + 'static {
    let prev = Cell::new(world.hovered_hint);
    move |w: &CadWorld| cell_diff(&prev, w.hovered_hint)
}

pub(crate) fn tight_spacer(
    ctx: &mut FynixCtx<CadWorld>,
) -> ElementId {
    ctx.add_with::<Pad>(|sp, _| {
        *sp = Pad::horizontal(2.0);
    })
    .id()
}

pub(crate) fn header_label(
    ctx: &mut FynixCtx<CadWorld>,
    text: impl Into<String>,
) -> ElementId {
    ctx.add_with::<Label>(|l, _| {
        l.text = text.into();
        l.font_size = HEADER_FONT;
        l.fill = Brush::Solid(MUTED_TEXT);
    })
    .id()
}

pub(crate) fn unified_fill(
    active: bool,
    hovered: bool,
    enabled: bool,
) -> Brush {
    Brush::Solid(if !enabled {
        DISABLED_BG
    } else {
        match (active, hovered) {
            (true, true) => HOVERED_ACTIVE_BG,
            (true, false) => ACTIVE_BG,
            (false, true) => HOVERED_INACTIVE_BG,
            (false, false) => INACTIVE_BG,
        }
    })
}

pub(crate) fn unified_fg(enabled: bool) -> Brush {
    Brush::Solid(if enabled { Color::WHITE } else { DISABLED_TEXT })
}
