use std::num::NonZeroUsize;
use std::sync::Arc;

use fynix::Fynix;
use fynix::ctx::FynixCtx;
use fynix::element::ElementId;
use fynix::rectree;
use fynix_elements::WindowSize;
use imaging_vello::VelloSceneSink;
use vello::kurbo::Rect;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

pub trait FynixDemo {
    fn window_title(&self) -> &'static str {
        "Fynix"
    }

    fn initial_logical_size(&self) -> (f64, f64) {
        (800.0, 600.0)
    }

    fn init(&mut self, _fynix: &mut Fynix) {}

    fn build(&mut self, ctx: &mut FynixCtx<()>) -> ElementId;
}

pub struct VelloWinitApp<'s, D: FynixDemo> {
    fynix: Fynix,
    root_id: ElementId,
    demo: D,
    context: RenderContext,
    renderer: Option<Renderer>,
    state: RenderState<'s>,
    scene: Scene,
}

pub enum RenderState<'s> {
    Suspended(Option<Arc<Window>>),
    Active {
        surface: Box<RenderSurface<'s>>,
        window: Arc<Window>,
    },
}

impl<D: FynixDemo> VelloWinitApp<'_, D> {
    pub fn new(mut demo: D) -> Self {
        fynix::init();

        let mut fynix = Fynix::new();
        demo.init(&mut fynix);

        let mut world = ();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add_with::<WindowSize>(|w, ctx| {
                w.set_child(demo.build(ctx));
            })
        };

        Self {
            fynix,
            root_id,
            demo,
            context: RenderContext::new(),
            renderer: None,
            state: RenderState::Suspended(None),
            scene: Scene::new(),
        }
    }

    fn render(&mut self) {
        let (surface, window) = match &mut self.state {
            RenderState::Active { surface, window } => (surface, window),
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
            self.context
                .resize_surface(surface, phys.width, phys.height);
        }

        if let Some(root) = self
            .fynix
            .elements
            .get_typed_mut::<WindowSize>(&self.root_id)
        {
            root.size = rectree::Size::new(
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

impl<D: FynixDemo> ApplicationHandler for VelloWinitApp<'_, D> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state
        else {
            return;
        };

        let (w, h) = self.demo.initial_logical_size();
        let window = cached_window.take().unwrap_or_else(|| {
            let attr = Window::default_attributes()
                .with_inner_size(LogicalSize::new(w, h))
                .with_title(self.demo.window_title());
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
            pollster::block_on(surface_future).expect("create surface");

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
                        antialiasing_support: vello::AaSupport::area_only(),
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
            self.state =
                RenderState::Suspended(Some(window.clone()));
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
                self.render();
                if let RenderState::Active { window, .. } = &self.state
                {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(_) => {
                if let RenderState::Active { window, .. } = &self.state
                {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
