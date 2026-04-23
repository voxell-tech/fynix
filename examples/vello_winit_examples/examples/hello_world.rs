use std::sync::Arc;

use fynix::Fynix;
use fynix::ctx::FynixCtx;
use fynix::element::ElementId;
use fynix::style::path;
use fynix_elements::{
    Button, Horizontal, Label, Pad, TextContext, Vertical,
};
use parley::FontStyle;
use parley::fontique::{Blob, GenericFamily};
use vello::peniko::Color;
use vello::peniko::color::palette::css;
use vello_winit_examples::{FynixDemo, VelloWinitApp};
use winit::event_loop::EventLoop;

const FONT: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

fynix::register_element!(EmptyBtn, Button<()>);

struct HelloWorld;

impl FynixDemo for HelloWorld {
    fn window_title(&self) -> &'static str {
        "Hello, Fynix!"
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

    fn build(&mut self, ctx: &mut FynixCtx<()>) -> ElementId {
        ctx.set(path!(<EmptyBtn>::corner_radius), 8.0);

        ctx.add_with::<Pad>(|p, ctx| {
            *p = Pad::all(20.0);
            p.set_child(ctx.add_with::<Vertical>(|v, ctx| {
                ctx.set(path!(<Label>::fill), Color::WHITE.into());

                v.add(ctx.add_with::<Label>(|label, _ctx| {
                    label.text = "Hello, Fynix!".into();
                    label.font_size = 24.0;
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

                v.add(ctx.add_with::<Horizontal>(|v, ctx| {
                    ctx.set(path!(<Label>::font_size), 16.0);
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

                v.add(ctx.add_with::<EmptyBtn>(|b, ctx| {
                    b.set_child(ctx.add_with::<Pad>(|p, ctx| {
                        *p = Pad::symmetric(8.0, 16.0);
                        p.set_child(ctx.add_with::<Label>(|l, _| {
                            l.text = "Press me!".into();
                        }));
                    }));
                }));
            }));
        })
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = VelloWinitApp::new(HelloWorld);
    event_loop.run_app(&mut app).unwrap();
}
