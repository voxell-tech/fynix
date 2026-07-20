mod builders;
mod elements;
mod helpers;
mod theme;

use core::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use fynix::prelude::*;
use fynix_elements::parley::fontique::{Blob, GenericFamily};
use fynix_elements::{TextContext, WindowSize};
use helpers::cell_diff;
use theme::{DropdownId, Tool, ToolbarState};
use vello_winit_examples::{DemoWorld, VelloWinitApp};
use winit::event::{KeyEvent, Modifiers};
use winit::event_loop::EventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::CursorIcon;

#[derive(Default)]
pub(crate) struct CadWorld {
    pub(crate) active_tool: Tool,
    pub(crate) expanded_dropdown: Option<DropdownId>,
    pub(crate) undo_stack: Vec<ToolbarState>,
    pub(crate) redo_stack: Vec<ToolbarState>,
    pub(crate) window_size: Size,
    pub(crate) hovered_tool: Option<Tool>,
    pub(crate) snap_enabled: bool,
    pub(crate) grid_enabled: bool,
    pub(crate) ortho_enabled: bool,
    pub(crate) undo_hovered: bool,
    pub(crate) redo_hovered: bool,
    pub(crate) hovered_hint: Option<&'static str>,
    pub(crate) cursor_icon: CursorIcon,
}

impl CadWorld {
    fn snapshot(&self) -> ToolbarState {
        ToolbarState {
            active_tool: self.active_tool,
            snap_enabled: self.snap_enabled,
            grid_enabled: self.grid_enabled,
            ortho_enabled: self.ortho_enabled,
            expanded_dropdown: self.expanded_dropdown,
        }
    }

    fn apply(&mut self, state: ToolbarState) {
        self.active_tool = state.active_tool;
        self.snap_enabled = state.snap_enabled;
        self.grid_enabled = state.grid_enabled;
        self.ortho_enabled = state.ortho_enabled;
        self.expanded_dropdown = state.expanded_dropdown;
    }

    pub(crate) fn push_undo_state(&mut self) {
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    pub(crate) fn select_tool(&mut self, tool: Tool) {
        self.push_undo_state();
        self.active_tool = tool;
        self.expanded_dropdown = None;
    }

    pub(crate) fn undo(&mut self) {
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack.push(self.snapshot());
        self.apply(prev);
    }

    pub(crate) fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack.push(self.snapshot());
        self.apply(next);
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(CadWorld::default());
    event_loop.run_app(&mut app).unwrap();
}

impl DemoWorld for CadWorld {
    fn window_title(&self) -> &'static str {
        "Fynix CAD Toolbar Example"
    }

    fn initial_logical_size(&self) -> (f64, f64) {
        (1100.0, 600.0)
    }

    fn init(&mut self, fynix: &mut Fynix<Self>) {
        fynix_elements::init_resources(fynix);

        if let Some(text_cx) =
            fynix.resources.get_mut::<TextContext>()
        {
            let blob = Blob::new(Arc::new(theme::FONT));
            let ids =
                text_cx.font_cx.collection.register_fonts(blob, None);
            text_cx.font_cx.collection.set_generic_families(
                GenericFamily::SansSerif,
                ids.into_iter().map(|(f, _)| f),
            );
        }
    }

    fn on_keyboard(&mut self, event: &KeyEvent, mods: &Modifiers) {
        if event.logical_key == Key::Named(NamedKey::Escape)
            && self.expanded_dropdown.is_some()
        {
            self.expanded_dropdown = None;
            return;
        }

        let ctrl = mods.state().control_key();
        let shift = mods.state().shift_key();

        if ctrl && shift {
            if let Key::Character(ch) = &event.logical_key {
                if ch.as_str().eq_ignore_ascii_case("z") {
                    self.redo();
                    return;
                }
            }
        }

        if ctrl {
            if let Key::Character(ch) = &event.logical_key {
                if ch.as_str().eq_ignore_ascii_case("z") {
                    self.undo();
                    return;
                }
            }
        }

        if !ctrl && !shift {
            if let Some(text) = &event.text {
                if let Some(ch) = text.chars().next() {
                    if let Some(tool) = Tool::from_key(ch) {
                        self.select_tool(tool);
                    }
                }
            }
        }
    }

    fn update(&mut self, _dt: Duration) {}

    fn cursor(&self) -> CursorIcon {
        self.cursor_icon
    }

    fn set_window_size(&mut self, size: Size) {
        self.window_size = size;
    }

    fn build(ctx: &mut FynixCtx<Self>) -> ElementId {
        let size = ctx.world.window_size;
        let prev_window_size = Cell::new(size);
        ctx.add_with::<WindowSize>(|win, ctx| {
            win.size = size;
            win.set_child(builders::build_app(ctx));
        })
        .bind(
            move |w| cell_diff(&prev_window_size, w.window_size),
            |w| w.window_size,
            |win| &mut win.size,
        )
        .id()
    }
}
