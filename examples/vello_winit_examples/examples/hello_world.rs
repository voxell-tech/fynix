use std::sync::Arc;
use std::time::Duration;

use fynix::prelude::*;
use fynix_elements::parley::FontStyle;
use fynix_elements::parley::fontique::{Blob, GenericFamily};
use fynix_elements::{
    Button, Horizontal, Label, Pad, TextContext, Vertical,
};
use fynix_interactions::{Click, PointerEnter, PointerLeave};
use vello::peniko::Color;
use vello::peniko::color::palette::css;
use vello_winit_examples::{FynixDemo, VelloWinitApp};
use winit::event_loop::EventLoop;

const FONT: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(HelloWorld);
    event_loop.run_app(&mut app).unwrap();
}

struct HelloWorld;

/// Demo world: the current FPS, recomputed each frame from the
/// frame delta.
#[derive(Default)]
struct DemoWorld {
    fps: u32,
    /// True only on the frame `fps` changed, so the reactive counter
    /// rebuilds just then.
    fps_changed: bool,
    /// Bumped by clicking the increment button.
    count: u32,
    /// True only on the frame `count` changed, so its reactive label
    /// rebuilds just then.
    count_changed: bool,
    /// Whether the pointer is currently over the increment button.
    hovering: bool,
    /// True only on the frame `hovering` changed.
    hovering_changed: bool,
}

/// Emitted by the increment button's click handler, drained into
/// [`DemoWorld::count`] each frame.
struct Increment;

/// Emitted by the increment button's enter/leave handlers, carrying
/// the new hover state. Pushed into a single queue so order is kept.
struct Hover(bool);

impl FynixDemo for HelloWorld {
    type World = DemoWorld;

    fn window_title(&self) -> &'static str {
        "Hello, Fynix!"
    }

    fn update(&mut self, world: &mut DemoWorld, dt: Duration) {
        world.fps_changed = false;
        world.count_changed = false;
        world.hovering_changed = false;
        let secs = dt.as_secs_f32();
        if secs > 0.0 {
            let fps = (1.0 / secs).round() as u32;
            if fps != world.fps {
                world.fps = fps;
                world.fps_changed = true;
            }
        }
    }

    fn apply_events(
        &mut self,
        world: &mut DemoWorld,
        events: &mut Events,
    ) {
        let clicks = events.drain::<Increment>().count();
        if clicks > 0 {
            world.count += clicks as u32;
            world.count_changed = true;
        }

        // The last hover event of the frame is the resting state.
        if let Some(Hover(hovering)) = events.drain::<Hover>().last()
            && hovering != world.hovering
        {
            world.hovering = hovering;
            world.hovering_changed = true;
        }
    }

    fn init(&mut self, fynix: &mut Fynix) {
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

    fn build(&mut self, ctx: &mut FynixCtx<DemoWorld>) -> ElementId {
        ctx.add_with::<Pad>(|p, ctx| {
            ctx.set(path!(<Label>::fill), Color::WHITE.into());
            ctx.set(path!(<Label>::font_size), 42.0);

            *p = Pad::all(20.0);
            p.set_child(ctx.add_with::<Vertical>(|v, ctx| {
                // Reactive FPS counter: rebuilt only when the value
                // changes (see `DemoWorld::update`).
                v.add(ctx.reactive(
                    |w| w.fps_changed,
                    |ctx| {
                        let fps = ctx.world.fps;
                        Some(
                            ctx.add_with::<Label>(|label, _| {
                                label.text = format!("FPS: {fps}");
                            })
                            .id(),
                        )
                    },
                ));

                v.add(ctx.add_with::<Label>(|label, _ctx| {
                    label.text = "Hello, Fynix!".into();
                    label.font_size = 64.0;
                }));
                v.add(ctx.add_with::<Label>(|label, _ctx| {
                    label.text =
                        "Lorem ipsum dolor sit amet consectetur \
                         adipiscing elit. Placerat in id cursus mi \
                         pretium tellus duis. Urna tempor pulvinar \
                         vivamus fringilla lacus nec metus. Integer \
                         nunc posuere ut hendrerit semper vel class."
                            .into();
                }));

                // ctx.set(path!(<Label>::font_size), 42.0);

                v.add(ctx.add_with::<Horizontal>(|v, ctx| {
                    ctx.set(path!(<Label>::fill), css::AQUA.into());

                    v.add(ctx.add_with::<Label>(|label, _ctx| {
                        label.text = "Hello, Fynix!".into();
                    }));

                    ctx.set(
                        path!(<Label>::font_style),
                        FontStyle::Italic,
                    );

                    v.add(ctx.add_with::<Label>(|label, _ctx| {
                        label.text =
                            "Horizontal continuation.".into();
                    }));
                    v.add(ctx.add_with::<Label>(|label, _ctx| {
                        label.text = "Another label!".into();
                    }));
                }));

                // Click counter: the button emits `Increment`, which
                // is drained into `world.count`, and this label
                // rebuilds whenever the count changes.
                ctx.set(path!(<Label>::font_size), 42.0);
                v.add(ctx.reactive(
                    |w| w.count_changed,
                    |ctx| {
                        let count = ctx.world.count;
                        Some(
                            ctx.add_with::<Label>(|label, _| {
                                label.text =
                                    format!("Count: {count}");
                                label.fill = css::RED.into();
                            })
                            .id(),
                        )
                    },
                ));

                // Hover state: the button's enter/leave handlers emit
                // `Hover`, drained into `world.hovering`, and this
                // label rebuilds when it changes.
                v.add(ctx.reactive(
                    |w| w.hovering_changed,
                    |ctx| {
                        let hovering = ctx.world.hovering;
                        Some(
                            ctx.add_with::<Label>(|label, _| {
                                label.text = if hovering {
                                    "Hovering: yes".into()
                                } else {
                                    "Hovering: no".into()
                                };
                            })
                            .id(),
                        )
                    },
                ));

                v.add(
                    ctx.compose(TextButton { label: "Increment" })
                        .on::<Click>(|_, events| {
                            events.push(Increment);
                        })
                        .on::<PointerEnter>(|_, events| {
                            events.push(Hover(true));
                        })
                        .on::<PointerLeave>(|_, events| {
                            events.push(Hover(false));
                        }),
                );
            }));
        })
        .id()
    }
}

#[derive(Init)]
struct TextButtonStyle {
    #[init(8.0)]
    pub corner_radius: f64,
    #[init(8.0)]
    pub pad_v: f32,
    #[init(16.0)]
    pub pad_h: f32,
    pub bg_color: Option<Color>,
}

struct TextButton<'a> {
    pub label: &'a str,
}

impl Composer<DemoWorld> for TextButton<'_> {
    type Style = TextButtonStyle;

    fn compose(
        self,
        style: TextButtonStyle,
        ctx: &mut FynixCtx<'_, '_, DemoWorld>,
    ) -> ElementId {
        ctx.add_with::<Button>(|b, ctx| {
            b.corner_radius = style.corner_radius;
            if let Some(bg_color) = style.bg_color {
                b.fill = bg_color.into();
            }

            b.set_child(ctx.add_with::<Pad>(|p, ctx| {
                *p = Pad::symmetric(style.pad_v, style.pad_h);
                p.set_child(ctx.add_with::<Label>(|l, _| {
                    l.text = self.label.into();
                }));
            }));
        })
        .id()
    }
}
