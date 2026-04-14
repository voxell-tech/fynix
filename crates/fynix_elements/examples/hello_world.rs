use std::num::NonZeroUsize;
use std::sync::Arc;

use field_path::field_accessor;
use fynix::Fynix;
use fynix::element::ElementId;
use fynix::rectree;
use fynix_elements::{
    Horizontal, Label, TextContext, Vertical, WindowSize,
};
use imaging_vello::VelloSceneSink;
use parley::FontStyle;
use parley::fontique::{Blob, GenericFamily};
use pollster::block_on;
use vello::kurbo::Rect;
use vello::peniko::Color;
use vello::peniko::color::palette::css;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{
    AaConfig, RenderParams, Renderer, RendererOptions, Scene,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::Window;

const FONT: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = HelloWorldApp::new();
    event_loop.run_app(&mut app).unwrap();
}

struct HelloWorldApp<'s> {
    fynix: Fynix,
    root_id: ElementId,
    context: RenderContext,
    renderer: Option<Renderer>,
    state: RenderState<'s>,
    scene: Scene,
}

enum RenderState<'s> {
    Suspended(Option<Arc<Window>>),
    Active {
        surface: Box<RenderSurface<'s>>,
        window: Arc<Window>,
    },
}

impl HelloWorldApp<'_> {
    fn new() -> Self {
        let mut fynix = Fynix::new();
        fynix_elements::init_resources(&mut fynix);

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

        let mut world = ();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            let content = ctx.add_with::<Vertical>(|v, ctx| {
                ctx.set(field_accessor!(<Label>::font_size), 24.0);
                ctx.set(
                    field_accessor!(<Label>::fill),
                    Color::WHITE.into(),
                );

                v.add(ctx.add_with::<Label>(|label, _ctx| {
                    label.text = "Hello, Fynix!".into();
                }));
                v.add(ctx.add_with::<Label>(|label, _ctx| {
                    label.text = "Lorem ipsum dolor sit amet consectetur adipiscing elit. Placerat in id cursus mi pretium tellus duis. Urna tempor pulvinar vivamus fringilla lacus nec metus. Integer nunc posuere ut hendrerit semper vel class. Conubia nostra inceptos himenaeos orci varius natoque penatibus. Mus donec rhoncus eros lobortis nulla molestie mattis. Purus est efficitur laoreet mauris pharetra vestibulum fusce. Sodales consequat magna ante condimentum neque at luctus. Ligula congue sollicitudin erat viverra ac tincidunt nam. Lectus commodo augue arcu dignissim velit aliquam imperdiet.".into();
                }));

                v.add(ctx.add_with::<Horizontal>(|v, ctx| {
                    ctx.set(
                        field_accessor!(<Label>::font_size),
                        16.0,
                    );
                    ctx.set(
                        field_accessor!(<Label>::fill),
                        css::AQUA.into(),
                    );

                    v.add(ctx.add_with::<Label>(|label, _ctx| {
                        label.text = "Hello, Fynix!".into();
                    }));

                    ctx.set(
                        field_accessor!(<Label>::font_style),
                        FontStyle::Italic,
                    );

                    v.add(ctx.add_with::<Label>(|label, _ctx| {
                        label.text = "Horizontal continuation.".into();
                    }));
                    v.add(ctx.add_with::<Label>(|label, _ctx| {
                        label.text = "Another label!".into();
                    }));
                }));
            });

            ctx.add_with::<WindowSize>(|f, _ctx| {
                f.set_child(content);
            })
        };

        Self {
            fynix,
            root_id,
            context: RenderContext::new(),
            renderer: None,
            state: RenderState::Suspended(None),
            scene: Scene::new(),
        }
    }

    fn render_frame(&mut self) {
        let (surface, window) = match &mut self.state {
            RenderState::Active { surface, window } => {
                (surface, window)
            }
            _ => return,
        };

        self.scene.reset();

        let phys = window.inner_size();
        if phys.width == 0 || phys.height == 0 {
            return;
        }

        if surface.config.width != phys.width
            || surface.config.height != phys.height
        {
            self.context.resize_surface(
                surface,
                phys.width,
                phys.height,
            );
        }

        // TODO(nixon): This should be mutated via signals!
        if let Some(fixed) = self
            .fynix
            .elements
            .get_typed_mut::<WindowSize>(&self.root_id)
        {
            fixed.size = rectree::Size::new(
                phys.width as f32,
                phys.height as f32,
            );
        }

        if let Some(meta) =
            self.fynix.elements.metas.get_mut(&self.root_id)
        {
            meta.node.state.reset();
        }

        self.fynix.layout(&self.root_id);

        let bounds = Rect::new(
            0.0,
            0.0,
            phys.width as f64,
            phys.height as f64,
        );
        let mut sink = VelloSceneSink::new(&mut self.scene, bounds);
        self.fynix.render(&self.root_id, &mut sink);
        sink.finish().unwrap();

        let dev = &self.context.devices[surface.dev_id];
        let texture = match surface.surface.get_current_texture() {
            Ok(t) => t,
            Err(
                wgpu::SurfaceError::Lost
                | wgpu::SurfaceError::Outdated,
            ) => {
                self.context.resize_surface(
                    surface,
                    phys.width,
                    phys.height,
                );
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
            Err(wgpu::SurfaceError::OutOfMemory) => {
                panic!("GPU out of memory")
            }
            Err(wgpu::SurfaceError::Other) => return,
        };

        self.renderer
            .as_mut()
            .unwrap()
            .render_to_texture(
                &dev.device,
                &dev.queue,
                &self.scene,
                &surface.target_view,
                &RenderParams {
                    base_color: Color::from_rgb8(20, 20, 30),
                    width: surface.config.width,
                    height: surface.config.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .unwrap();

        let mut enc = dev.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: None },
        );
        surface.blitter.copy(
            &dev.device,
            &mut enc,
            &surface.target_view,
            &texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        dev.queue.submit([enc.finish()]);
        texture.present();
    }
}

impl ApplicationHandler for HelloWorldApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state
        else {
            return;
        };

        let window = cached_window.take().unwrap_or_else(|| {
            let attr = Window::default_attributes()
                .with_inner_size(LogicalSize::new(
                    800.0_f64, 600.0_f64,
                ))
                .with_title("Hello, Fynix!");
            Arc::new(event_loop.create_window(attr).unwrap())
        });

        let phys = window.inner_size();
        let surface_future = self.context.create_surface(
            window.clone(),
            phys.width,
            phys.height,
            wgpu::PresentMode::AutoVsync,
        );
        let surface =
            block_on(surface_future).expect("create surface");

        let device_handle = &self.context.devices[surface.dev_id];
        surface
            .surface
            .configure(&device_handle.device, &surface.config);

        if self.renderer.is_none() {
            self.renderer = Some(
                Renderer::new(
                    &device_handle.device,
                    RendererOptions {
                        use_cpu: false,
                        antialiasing_support:
                            vello::AaSupport::area_only(),
                        num_init_threads: NonZeroUsize::new(1),
                        pipeline_cache: None,
                    },
                )
                .unwrap(),
            );
        }

        self.state = RenderState::Active {
            surface: Box::new(surface),
            window,
        };
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => {
                self.render_frame();
                if let RenderState::Active { window, .. } =
                    &self.state
                {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(_) => {
                if let RenderState::Active { window, .. } =
                    &self.state
                {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
