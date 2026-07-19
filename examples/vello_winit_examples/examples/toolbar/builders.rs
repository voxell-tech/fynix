use core::cell::Cell;
use std::rc::Rc;

use fynix::interaction::{Handler, HandlerFn};
use fynix::prelude::*;
use fynix::reactive::ChangedFn;
use fynix_elements::{
    Align, Button, Frame, Horizontal, Label, OverlayComposer, Pad,
    Side, Vertical,
};
use fynix_interaction::prelude::*;
use vello::peniko::{Brush, Color};
use winit::window::CursorIcon;

use crate::CadWorld;
use crate::elements::{
    IconLabelButton, ToolButton, ToolIcon, ToolbarRow,
};
use crate::helpers::*;
use crate::theme::*;

fn arrow_button(
    ctx: &mut FynixCtx<CadWorld>,
    id: DropdownId,
    is_expanded: bool,
) -> ElementId {
    ctx.compose(ToolButton::new("▾", is_expanded).on_click(
        move |_, res| {
            res.push_undo_state();
            res.expanded_dropdown = match res.expanded_dropdown {
                Some(x) if x == id => None,
                _ => Some(id),
            };
        },
    ))
    .id()
}

fn split_button(
    ctx: &mut FynixCtx<CadWorld>,
    id: DropdownId,
    default_tool: Tool,
    is_expanded: bool,
    group_has_active: bool,
) -> (ElementId, ElementId) {
    let arrow_id = arrow_button(ctx, id, is_expanded);
    let split_id = ctx
        .add_with::<Horizontal>(|h, ctx| {
            h.add(
                ctx.compose(
                    ToolButton::for_tool(
                        default_tool,
                        group_has_active,
                    )
                    .on_click(move |_, res| {
                        res.select_tool(default_tool);
                    }),
                ),
            );
            h.add(arrow_id);
        })
        .id();
    (split_id, arrow_id)
}

fn dropdown_panel(
    ctx: &mut FynixCtx<CadWorld>,
    tools: &[Tool],
    active: Tool,
) -> ElementId {
    let content_id = ctx
        .add_with::<Vertical>(|col, ctx| {
            for (i, tool) in tools.iter().enumerate() {
                if i > 0 {
                    col.add(ctx.add_with::<Pad>(|p, _| {
                        *p = Pad::vertical(4.0);
                    }));
                }
                let tool_val = *tool;
                col.add(
                    ctx.compose(
                        ToolButton::for_tool(
                            tool_val,
                            tool_val == active,
                        )
                        .on_click(
                            move |_, res| {
                                res.select_tool(tool_val);
                            },
                        ),
                    ),
                );
            }
        })
        .id();
    panel_frame(ctx, content_id)
}

fn toggle_flag(
    ctx: &mut FynixCtx<CadWorld>,
    label: &str,
    get: fn(&CadWorld) -> bool,
    toggle: fn(&mut CadWorld),
) -> ElementId {
    let initial = get(ctx.world);
    let prev = Cell::new(initial);
    let prev_hovered = Cell::new(false);
    let hovered = Rc::new(Cell::new(false));
    let fg = unified_fg(true);

    ctx.add_with::<Button>(|b, ctx| {
        b.fill = unified_fill(initial, false, true);
        b.set_child(ctx.add_with::<Pad>(|p, ctx| {
            *p = Pad::symmetric(2.0, 6.0);
            p.set_child(ctx.add_with::<Label>(|l, _| {
                l.text = label.into();
                l.fill = fg;
            }));
        }));
    })
    .bind(
        {
            let hovered = Rc::clone(&hovered);
            move |w| {
                let on = get(w);
                let h = hovered.get();
                let on_changed = cell_diff(&prev, on);
                let hover_changed = cell_diff(&prev_hovered, h);
                on_changed || hover_changed
            }
        },
        {
            let hovered = Rc::clone(&hovered);
            move |w| {
                let on = get(w);
                unified_fill(on, hovered.get(), true)
            }
        },
        |b: &mut Button| &mut b.fill,
    )
    .interact_raw(Handler::<PointerEnter<Mouse>, _>::new({
        let hovered = Rc::clone(&hovered);
        move |_, res: &mut Response<'_, CadWorld>| {
            hovered.set(true);
            res.cursor_icon = CursorIcon::Pointer;
        }
    }))
    .interact_raw(Handler::<PointerLeave<Mouse>, _>::new({
        let hovered = Rc::clone(&hovered);
        move |_, res: &mut Response<'_, CadWorld>| {
            hovered.set(false);
            res.cursor_icon = CursorIcon::Default;
        }
    }))
    .interact_raw(Handler::new(
        move |_: PrimaryClick<Mouse>,
              res: &mut Response<'_, CadWorld>| {
            toggle(res);
        },
    ))
    .id()
}

fn shortcut_button<GetE, GetH>(
    ctx: &mut FynixCtx<CadWorld>,
    icon: &str,
    label: &str,
    get_enabled: GetE,
    get_hovered: GetH,
    get_changed: impl ChangedFn<CadWorld>,
    set_hovered: impl Fn(&mut CadWorld, bool) + 'static + Copy,
    on_click: impl HandlerFn<PrimaryClick<Mouse>, CadWorld>,
) -> ElementId
where
    GetE: Fn(&CadWorld) -> bool + 'static + Copy,
    GetH: Fn(&CadWorld) -> bool + 'static + Copy,
{
    let enabled = get_enabled(ctx.world);
    let hovered = get_hovered(ctx.world);

    ctx.compose(IconLabelButton {
        label: label.into(),
        icon_text: Some(icon.into()),
        tool_icon: None,
        accent: false,
        fill: unified_fill(false, hovered, enabled),
        fg: unified_fg(enabled),
        enabled,
        on_click: Some(Handler::new(on_click)),
        on_enter: Some(Handler::new(move |_, res| {
            set_hovered(res, true);
            res.cursor_icon = CursorIcon::Pointer;
        })),
        on_leave: Some(Handler::new(move |_, res| {
            set_hovered(res, false);
            res.cursor_icon = CursorIcon::Default;
        })),
    })
    .bind(
        get_changed,
        move |w| {
            let e = get_enabled(w);
            let h = get_hovered(w);
            unified_fill(false, h, e)
        },
        |b: &mut Button| &mut b.fill,
    )
    .id()
}

fn undo_redo_buttons(ctx: &mut FynixCtx<CadWorld>) -> ElementId {
    let prev_undo = Cell::new((
        ctx.world.undo_hovered,
        ctx.world.undo_stack.len(),
    ));
    let prev_redo = Cell::new((
        ctx.world.redo_hovered,
        ctx.world.redo_stack.len(),
    ));
    ctx.add_with::<Horizontal>(|h, ctx| {
        h.add(shortcut_button(
            ctx,
            "\u{21A9}",
            "Undo",
            |w| !w.undo_stack.is_empty(),
            |w| w.undo_hovered,
            move |w| {
                let current = (w.undo_hovered, w.undo_stack.len());
                let changed = cell_diff(&prev_undo, current);
                changed
            },
            |w, v| w.undo_hovered = v,
            move |_, res| {
                res.undo();
            },
        ));
        h.add(ctx.add_with::<Pad>(|sp, _| {
            *sp = Pad::horizontal(GAP / 2.0);
        }));
        h.add(shortcut_button(
            ctx,
            "\u{21AA}",
            "Redo",
            |w| !w.redo_stack.is_empty(),
            |w| w.redo_hovered,
            move |w| {
                let current = (w.redo_hovered, w.redo_stack.len());
                let changed = cell_diff(&prev_redo, current);
                changed
            },
            |w, v| w.redo_hovered = v,
            move |_, res| {
                res.redo();
            },
        ));
    })
    .id()
}

fn build_menu_bar(ctx: &mut FynixCtx<CadWorld>) -> ElementId {
    ctx.add_with::<Horizontal>(|h, ctx| {
        apply_menu_bar(ctx);

        let mut first = true;
        for item in [
            "File", "Edit", "View", "Insert", "Format", "Tools",
            "Draw", "Modify",
        ] {
            if !first {
                h.add(vertical_separator(ctx));
            }
            first = false;

            h.add(
                ctx.add_with::<Button>(|b, ctx| {
                    let label_text = item.to_string();
                    let label_id = ctx
                        .add_with::<Label>(|l, _| {
                            l.text = label_text;
                        })
                        .id();
                    b.set_child(
                        ctx.add_with::<Pad>(|p, _| {
                            *p = Pad::symmetric(2.0, 6.0);
                            p.set_child(label_id);
                        })
                        .id(),
                    );
                })
                .id(),
            );
        }
    })
    .id()
}

fn status_bar_icon_row(
    ctx: &mut FynixCtx<CadWorld>,
    active: Tool,
) -> ElementId {
    ctx.add_with::<Horizontal>(|h, ctx| {
        h.add(ctx.add_with::<ToolIcon>(|icon, _| {
            icon.tool = Some(active);
            icon.color = Brush::Solid(ACCENT);
            icon.size = 14.0;
        }));
        h.add(ctx.add_with::<Pad>(|sp, _| {
            *sp = Pad::horizontal(4.0);
        }));
        h.add(ctx.add_with::<Label>(|l, _| {
            l.text = active.display_label().into();
            l.fill = Brush::Solid(MUTED_TEXT);
        }));
        h.add(tight_spacer(ctx));
        apply_toolbar_toggle(ctx);
        h.add(toggle_flag(
            ctx,
            "Snap",
            |w| w.snap_enabled,
            |w| w.snap_enabled = !w.snap_enabled,
        ));
        h.add(tight_spacer(ctx));
        h.add(toggle_flag(
            ctx,
            "Grid",
            |w| w.grid_enabled,
            |w| w.grid_enabled = !w.grid_enabled,
        ));
        h.add(tight_spacer(ctx));
        h.add(toggle_flag(
            ctx,
            "Ortho",
            |w| w.ortho_enabled,
            |w| w.ortho_enabled = !w.ortho_enabled,
        ));
    })
    .id()
}

fn status_bar_hint_row(
    ctx: &mut FynixCtx<CadWorld>,
    active: Tool,
) -> ElementId {
    header_label(ctx, active.hint())
}

fn build_status_bar(
    ctx: &mut FynixCtx<CadWorld>,
    active: Tool,
) -> ElementId {
    ctx.add_with::<Vertical>(|v, ctx| {
        v.add(add_separator(ctx));
        v.add(ctx.add_with::<Pad>(|p, ctx| {
            *p = Pad::new(6.0, 12.0, 6.0, 12.0);
            p.set_child(ctx.add_with::<Vertical>(|v, ctx| {
                v.add(status_bar_icon_row(ctx, active));
                v.add(ctx.add_with::<Pad>(|sp, _| {
                    *sp = Pad::vertical(1.0);
                }));
                v.add(status_bar_hint_row(ctx, active));
            }));
        }));
    })
    .id()
}

fn build_toolbar_content(
    ctx: &mut FynixCtx<CadWorld>,
    active: Tool,
    draw_split: ElementId,
    modify_split: ElementId,
) -> ElementId {
    let row_id = ctx
        .add_with::<Horizontal>(|row, ctx| {
            let select_id = ctx
                .compose(
                    ToolButton::for_tool(
                        Tool::Select,
                        active == Tool::Select,
                    )
                    .on_click(|_, res| {
                        res.select_tool(Tool::Select);
                    }),
                )
                .id();
            row.add(toolbar_group(ctx, "SELECT", select_id));

            row.add(vertical_separator(ctx));

            row.add(toolbar_group(ctx, "DRAW", draw_split));

            row.add(vertical_separator(ctx));

            row.add(toolbar_group(ctx, "MODIFY", modify_split));

            row.add(vertical_separator(ctx));

            let undo_id = undo_redo_buttons(ctx);
            row.add(toolbar_group(ctx, "UNDO", undo_id));
        })
        .id();

    panel_frame(ctx, row_id)
}

fn toolbar_group(
    ctx: &mut FynixCtx<'_, '_, CadWorld>,
    header: &str,
    content: ElementId,
) -> ElementId {
    ctx.add_with::<Vertical>(|v, ctx| {
        let header_id = header_label(ctx, header);

        let separator_id = ctx
            .add_with::<Frame>(|f, _| {
                f.fill = Brush::Solid(SEPARATOR);
                f.corner_radius = 0.0;
                f.top = 2.0;
                f.bottom = 1.0;
                f.left = PAD_H;
                f.right = PAD_H;
            })
            .id();

        let spacer_id = ctx
            .add_with::<Pad>(|p, _| {
                *p = Pad::vertical(4.0);
            })
            .id();

        v.add(header_id)
            .add(separator_id)
            .add(spacer_id)
            .add(content);
    })
    .id()
}

fn build_toolbar(ctx: &mut FynixCtx<CadWorld>) -> ElementId {
    let active = ctx.world.active_tool;
    let expanded = ctx.world.expanded_dropdown;

    let (draw_split, draw_arrow) = split_button(
        ctx,
        DropdownId::Draw,
        Tool::Line,
        expanded == Some(DropdownId::Draw),
        DRAW_TOOLS.contains(&active),
    );
    let (modify_split, modify_arrow) = split_button(
        ctx,
        DropdownId::Modify,
        Tool::Move,
        expanded == Some(DropdownId::Modify),
        MODIFY_TOOLS.contains(&active),
    );

    let frame_id =
        build_toolbar_content(ctx, active, draw_split, modify_split);

    if let Some(id) = expanded {
        let tools = match id {
            DropdownId::Draw => DRAW_TOOLS,
            DropdownId::Modify => MODIFY_TOOLS,
        };
        let dropdown = dropdown_panel(ctx, tools, active);
        let arrow_id = match id {
            DropdownId::Draw => draw_arrow,
            DropdownId::Modify => modify_arrow,
        };
        ctx.compose(
            OverlayComposer::new()
                .content(frame_id)
                .overlay(dropdown)
                .anchor(arrow_id)
                .side(Side::Bottom)
                .h_align(Align::Start)
                .gap(GAP_V),
        )
        .id()
    } else {
        frame_id
    }
}

fn build_hint_bar(ctx: &mut FynixCtx<CadWorld>) -> ElementId {
    let hint_changed = hint_changed(ctx.world);
    let initial_hint: String =
        ctx.world.hovered_hint.unwrap_or("").into();
    let hint_label = ctx
        .add_with::<Label>(|l, _| {
            l.text = initial_hint;
            l.font_size = HEADER_FONT;
            l.fill = Brush::Solid(Color::WHITE);
        })
        .bind(
            hint_changed,
            |w| w.hovered_hint.unwrap_or("").into(),
            |l: &mut Label| &mut l.text,
        )
        .id();

    let hint_content = ctx
        .add_with::<Pad>(|p, _| {
            p.left = OUTER_PAD;
            p.right = OUTER_PAD;
            p.top = 4.0;
            p.bottom = 4.0;
            p.set_child(hint_label);
        })
        .id();

    dismiss_on_click(ctx, hint_content)
}

pub(crate) fn build_app(ctx: &mut FynixCtx<CadWorld>) -> ElementId {
    let prev_watcher = Cell::new((
        ctx.world.active_tool,
        ctx.world.expanded_dropdown,
        ctx.world.undo_stack.len(),
        ctx.world.redo_stack.len(),
    ));
    ctx.add_with::<Vertical>(|app, ctx| {
        app.add(build_menu_bar(ctx));
        app.add(
            ctx.watch(
                move |w| {
                    let current = (
                        w.active_tool,
                        w.expanded_dropdown,
                        w.undo_stack.len(),
                        w.redo_stack.len(),
                    );
                    cell_diff(&prev_watcher, current)
                },
                |ctx| {
                    let active = ctx.world.active_tool;

                    Some(
                        ctx.add_with::<Vertical>(|content, ctx| {
                            apply_base(ctx);

                            content.add(ctx.add_with::<ToolbarRow>(
                                |row, ctx| {
                                    row.children
                                        .push(build_toolbar(ctx));
                                    row.children.push(
                                        build_status_bar(ctx, active),
                                    );
                                },
                            ));

                            content.add(add_separator(ctx));

                            content.add(build_hint_bar(ctx));
                        })
                        .id(),
                    )
                },
            )
            .id(),
        );
    })
    .id()
}
