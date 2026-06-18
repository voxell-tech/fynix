use std::sync::Arc;
use std::time::Duration;

use fynix::interaction::{Handler, HandlerFn};
use fynix::prelude::*;
use fynix_elements::parley::fontique::{Blob, GenericFamily};
use fynix_elements::{
    Button, Horizontal, Label, Pad, TextContext, Vertical, WindowSize,
};
use fynix_interaction::prelude::*;
use vello::peniko::color::palette::css;
use vello::peniko::{Brush, Color};
use vello_winit_examples::{DemoWorld, VelloWinitApp};
use winit::event_loop::EventLoop;
use winit::window::CursorIcon;

const FONT: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

/// The demo's pointer identity: a single mouse, matching the id the
/// backend feeds in.
type Mouse = PointerId<(), ()>;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(HelloWorld::default());
    event_loop.run_app(&mut app).unwrap();
}

/// Wraps `child` in symmetric padding, for spacing between items.
fn padded(
    ctx: &mut FynixCtx<HelloWorld>,
    v: f32,
    h: f32,
    child: ElementId,
) -> ElementId {
    ctx.add_with::<Pad>(|p, _| {
        *p = Pad::symmetric(v, h);
        p.set_child(child);
    })
    .id()
}

/// The hover button's fill: warm when hovered, cool when not.
fn hover_fill(hovered: bool) -> Brush {
    if hovered {
        css::VIOLET.into()
    } else {
        css::STEEL_BLUE.into()
    }
}

fn main_layout(ctx: &mut FynixCtx<HelloWorld>) -> ElementId {
    ctx.add_with::<Pad>(|p, ctx| {
        *p = Pad::all(36.0);
        p.set_child(ctx.add_with::<Vertical>(|v, ctx| {
            // Header.
            let title = ctx
                .add_with::<Label>(|l, _| {
                    l.text = "Fynix Interactions".into();
                    l.font_size = 34.0;
                })
                .id();
            v.add(title);

            let fps = ctx
                .add_with::<Label>(|l, ctx| {
                    l.text = format!("FPS: {}", ctx.world.fps);
                    l.font_size = 13.0;
                    l.fill = css::SLATE_GRAY.into();
                })
                .bind(
                    |w| w.fps_changed,
                    |w| format!("FPS: {}", w.fps),
                    |l| &mut l.text,
                )
                .id();
            v.add(padded(ctx, 6.0, 0.0, fps));

            // The counter value, bound to the world's count.
            let count = ctx
                .add_with::<Label>(|l, _| {
                    l.text = "0".into();
                    l.font_size = 64.0;
                    l.fill = css::AQUA.into();
                })
                .bind(
                    |w| w.count_changed,
                    |w| format!("{}", w.count),
                    |l| &mut l.text,
                )
                .id();
            v.add(padded(ctx, 12.0, 0.0, count));

            // Counter controls.
            let row = ctx
                .add_with::<Horizontal>(|h, ctx| {
                    let dec = ctx
                        .compose_with(
                            ActionButton::new("-")
                                .on_click(|_, res| {
                                    res.count -= 1;
                                    res.status = "Decremented";
                                })
                                .on_right_click(|_, res| {
                                    res.count -= 10;
                                    res.status = "Decremented 10 \
                                                  (right-click)";
                                }),
                            |s| s.color = css::TOMATO,
                        )
                        .id();
                    h.add(padded(ctx, 0.0, 6.0, dec));

                    let reset = ctx
                        .compose_with(
                            ActionButton::new("Reset").on_click(
                                |_, res| {
                                    res.count = 0;
                                    res.status = "Reset to zero";
                                },
                            ),
                            |s| s.color = css::SLATE_GRAY,
                        )
                        .id();
                    h.add(padded(ctx, 0.0, 6.0, reset));

                    // The plus button handles both buttons: left adds
                    // one, right (secondary) adds ten.
                    let inc = ctx
                        .compose_with(
                            ActionButton::new("+")
                                .on_click(|_, res| {
                                    res.count += 1;
                                    res.status = "Incremented";
                                })
                                .on_right_click(|_, res| {
                                    res.count += 10;
                                    res.status = "Incremented 10 \
                                                  (right-click)";
                                }),
                            |s| s.color = css::MEDIUM_SEA_GREEN,
                        )
                        .id();
                    h.add(padded(ctx, 0.0, 6.0, inc));
                })
                .id();
            v.add(padded(ctx, 12.0, 0.0, row));

            // A button that changes colour while the pointer is over
            // it, driven by enter/leave handlers and a fill binding.
            let hover = ctx
                .compose_with(
                    ActionButton::new("Hover over me")
                        .on_enter(|_, res| res.hovered = true)
                        .on_leave(|_, res| res.hovered = false),
                    |s| s.color = css::STEEL_BLUE,
                )
                .bind(
                    |w| w.hovered_changed,
                    |w| hover_fill(w.hovered),
                    |b| &mut b.fill,
                )
                .id();
            v.add(padded(ctx, 12.0, 0.0, hover));

            // Status line, bound to the last action taken.
            let status = ctx
                .add_with::<Label>(|l, _| {
                    l.text = "Click or hover the controls.".into();
                    l.font_size = 15.0;
                    l.fill = css::LIGHT_STEEL_BLUE.into();
                })
                .bind(
                    |w| w.status_changed,
                    |w| w.status.to_string(),
                    |l| &mut l.text,
                )
                .id();
            v.add(padded(ctx, 14.0, 0.0, status));
        }));
    })
    .id()
}

/// Demo world: a click-driven counter, a hover flag, and a status
/// line, alongside the FPS and window size. Each reactive value keeps
/// a `prev_*` copy so [`HelloWorld::update`] can flip its `*_changed`
/// flag by diffing once per frame, which the bindings read.
#[derive(Default)]
struct HelloWorld {
    /// The current fps of the app.
    fps: u32,
    /// True only on the frame `fps` changed.
    fps_changed: bool,
    /// Latest window size pushed in by the backend.
    window_size: Size,
    /// True when `window_size` changed.
    window_size_changed: bool,

    /// The counter value driven by the buttons.
    count: i32,
    prev_count: i32,
    count_changed: bool,

    /// Whether the pointer is over the hover button.
    hovered: bool,
    prev_hovered: bool,
    hovered_changed: bool,

    /// The most recent action, shown in the status line.
    status: &'static str,
    prev_status: &'static str,
    status_changed: bool,

    cursor_icon: CursorIcon,
}

impl DemoWorld for HelloWorld {
    fn window_title(&self) -> &'static str {
        "Fynix Interactions"
    }

    fn update(&mut self, dt: Duration) {
        self.fps_changed = false;
        let secs = dt.as_secs_f32();
        if secs > 0.0 {
            let fps = (1.0 / secs).round() as u32;
            if fps != self.fps {
                self.fps = fps;
                self.fps_changed = true;
            }
        }

        // Detect interaction-driven changes by diffing against last
        // frame, so handlers only need to set the value.
        self.count_changed = self.count != self.prev_count;
        self.prev_count = self.count;
        self.hovered_changed = self.hovered != self.prev_hovered;
        self.prev_hovered = self.hovered;
        self.status_changed = self.status != self.prev_status;
        self.prev_status = self.status;
    }

    fn set_window_size(&mut self, size: Size) {
        self.window_size = size;
        self.window_size_changed = true;
    }

    fn init(&mut self, fynix: &mut Fynix<Self>) {
        fynix_elements::init_resources(fynix);
        if let Some(text_cx) =
            fynix.resources.get_mut::<TextContext>()
        {
            let blob = Blob::new(Arc::new(FONT));
            let ids =
                text_cx.font_cx.collection.register_fonts(blob, None);
            text_cx.font_cx.collection.set_generic_families(
                GenericFamily::SansSerif,
                ids.into_iter().map(|(f, _)| f),
            );
        }
    }

    fn build(ctx: &mut FynixCtx<Self>) -> ElementId {
        let size = ctx.world.window_size;
        ctx.world.window_size_changed = false;
        ctx.add_with::<WindowSize>(|win, ctx| {
            win.size = size;
            win.set_child(main_layout(ctx));
        })
        .bind(
            |w| w.window_size_changed,
            |w| w.window_size,
            |win| &mut win.size,
        )
        .id()
    }

    fn cursor(&self) -> CursorIcon {
        self.cursor_icon
    }
}

#[derive(Init)]
struct ActionButtonStyle {
    #[init(css::WHEAT)]
    color: Color,
}

#[derive(Default)]
struct ActionButton {
    label: String,
    on_click: Option<Handler<PrimaryClick<Mouse>, HelloWorld>>,
    on_right_click:
        Option<Handler<SecondaryClick<Mouse>, HelloWorld>>,
    on_enter: Option<Handler<PointerEnter<Mouse>, HelloWorld>>,
    on_leave: Option<Handler<PointerLeave<Mouse>, HelloWorld>>,
}

impl ActionButton {
    /// Starts a button carrying `label`.
    fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Default::default()
        }
    }

    /// Sets the primary (left) click handler.
    fn on_click(
        mut self,
        handler: impl HandlerFn<PrimaryClick<Mouse>, HelloWorld>,
    ) -> Self {
        self.on_click = Some(handler.into());
        self
    }

    /// Sets the secondary (right) click handler.
    fn on_right_click(
        mut self,
        handler: impl HandlerFn<SecondaryClick<Mouse>, HelloWorld>,
    ) -> Self {
        self.on_right_click = Some(Handler::new(handler));
        self
    }

    /// Sets the pointer-enter handler, run after the cursor change.
    fn on_enter(
        mut self,
        handler: impl HandlerFn<PointerEnter<Mouse>, HelloWorld>,
    ) -> Self {
        self.on_enter = Some(Handler::new(handler));
        self
    }

    /// Sets the pointer-leave handler, run after the cursor change.
    fn on_leave(
        mut self,
        handler: impl HandlerFn<PointerLeave<Mouse>, HelloWorld>,
    ) -> Self {
        self.on_leave = Some(Handler::new(handler));
        self
    }
}

impl Composer<HelloWorld> for ActionButton {
    type Style = ActionButtonStyle;

    type Element = Button;

    fn compose(
        self,
        style: Self::Style,
        ctx: &mut FynixCtx<'_, '_, HelloWorld>,
    ) -> fynix::element::storage::ElementHandle<Self::Element>
    where
        Self: Sized,
    {
        let ActionButton {
            label,
            on_click,
            on_right_click,
            on_enter,
            on_leave,
        } = self;

        let mut button = ctx
            .add_with::<Button>(|b, ctx| {
                b.corner_radius = 10.0;
                b.fill = style.color.into();
                b.set_child(ctx.add_with::<Pad>(|p, ctx| {
                    *p = Pad::symmetric(12.0, 24.0);
                    p.set_child(ctx.add_with::<Label>(|l, _| {
                        l.text = label;
                        l.font_size = 22.0;
                    }));
                }));
            })
            // Enter/leave set the cursor, then run any user handler.
            .interact::<PointerEnter<Mouse>>(move |i, res| {
                res.cursor_icon = CursorIcon::Pointer;
                if let Some(on_enter) = &on_enter {
                    on_enter.call(i, res);
                }
            })
            .interact::<PointerLeave<Mouse>>(move |i, res| {
                res.cursor_icon = CursorIcon::Default;
                if let Some(on_leave) = &on_leave {
                    on_leave.call(i, res);
                }
            });

        // Forward the built click handlers directly.
        if let Some(on_click) = on_click {
            button = button.interact_raw(on_click);
        }
        if let Some(on_right_click) = on_right_click {
            button = button.interact_raw(on_right_click);
        }

        button.handle()
    }
}
