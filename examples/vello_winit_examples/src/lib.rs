use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fynix::prelude::*;
use fynix_elements::WindowSize;
use fynix_input::{
    HitTest, PointerButton, PointerId, PointerRecognizer, RawInput,
    RawInputKind, is_hit_target,
};
use imaging_vello::VelloSceneSink;
use vello::kurbo::Rect;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::{
    AaConfig, RenderParams, Renderer, RendererOptions, Scene, wgpu,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

pub trait FynixDemo {
    /// World state passed to [`Self::build`] and advanced each frame
    /// in [`Self::update`]. Reactive scopes read from it.
    type World: Default + 'static;

    fn window_title(&self) -> &'static str {
        "Fynix"
    }

    fn initial_logical_size(&self) -> (f64, f64) {
        (800.0, 600.0)
    }

    fn init(&mut self, _fynix: &mut Fynix) {}

    /// Advances world state before reactive scopes are updated.
    /// `dt` is the time elapsed since the previous frame.
    fn update(&mut self, _world: &mut Self::World, _dt: Duration) {}

    /// Drains UI events emitted by interaction handlers into the
    /// world, run once per frame before reactives update.
    fn apply_events(
        &mut self,
        _world: &mut Self::World,
        _events: &mut Events,
    ) {
    }

    fn build(&mut self, ctx: &mut FynixCtx<Self::World>)
    -> ElementId;
}

pub struct VelloWinitApp<'s, D: FynixDemo> {
    fynix: Fynix,
    root_id: ElementId,
    demo: D,
    world: D::World,
    last_frame: Instant,
    /// Start of the app, used to stamp raw input with a monotonic
    /// millisecond timestamp.
    start: Instant,
    recognizer: PointerRecognizer,
    /// Last known cursor position, in physical pixels. winit's
    /// `MouseInput` carries no position, so it is paired with the
    /// most recent `CursorMoved`.
    cursor: (f64, f64),
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
        let mut fynix = Fynix::new();
        demo.init(&mut fynix);

        let mut world = D::World::default();
        let root_id = {
            let mut ctx = fynix.root_ctx(&mut world);
            ctx.add_with::<WindowSize>(|w, ctx| {
                w.set_child(demo.build(ctx));
            })
            .id()
        };

        Self {
            fynix,
            root_id,
            demo,
            world,
            last_frame: Instant::now(),
            start: Instant::now(),
            recognizer: PointerRecognizer::new(),
            cursor: (0.0, 0.0),
            context: RenderContext::new(),
            renderer: None,
            state: RenderState::Suspended(None),
            scene: Scene::new(),
        }
    }

    fn render(&mut self) {
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

        // Resize the root only when the window size changes, marking
        // it dirty so the whole tree re-lays out then (and on the
        // first frame).
        let size = Size::new(phys.width as f32, phys.height as f32);
        let resized = self
            .fynix
            .elements
            .get_typed_mut::<WindowSize>(&self.root_id)
            .is_some_and(|root| {
                let changed = root.size.width != size.width
                    || root.size.height != size.height;
                root.size = size;
                changed
            });
        if resized {
            self.fynix.elements.mark_dirty(self.root_id);
        }

        // Advance world state with this frame's delta, then rebuild
        // any reactives whose inputs changed, before layout.
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame);
        self.last_frame = now;
        self.demo.update(&mut self.world, dt);
        self.demo
            .apply_events(&mut self.world, &mut self.fynix.events);
        self.fynix.update_reactives::<D::World>(&mut self.world);

        self.fynix.layout();

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

    /// Lowers a winit mouse button event into [`RawInput`] at the
    /// last known cursor position and feeds it through the
    /// recognizer.
    fn handle_mouse_input(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) {
        let (x, y) = self.cursor;
        let button = match button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Middle,
            MouseButton::Other(n) => PointerButton::Other(n),
            _ => PointerButton::Other(u16::MAX),
        };
        let kind = match state {
            ElementState::Pressed => RawInputKind::PointerDown {
                pointer: PointerId::MOUSE,
                button,
                x,
                y,
            },
            ElementState::Released => RawInputKind::PointerUp {
                pointer: PointerId::MOUSE,
                button,
                x,
                y,
            },
        };
        self.feed_input(kind);
    }

    /// Builds a [`HitTest`] over the current (previous-frame) layout
    /// and runs `kind` through the recognizer, dispatching any
    /// interactions it produces.
    ///
    /// Only elements with a pointer handler are indexed, so the
    /// spatree stays small. A redraw is requested so the result is
    /// visible promptly.
    fn feed_input(&mut self, kind: RawInputKind) {
        let input = RawInput {
            time: self.start.elapsed().as_millis() as u64,
            kind,
        };
        let hit = HitTest::build(
            &self.fynix.elements,
            &self.root_id,
            |id| is_hit_target(&self.fynix.interactions, id),
        );
        self.recognizer.handle(&mut self.fynix, &hit, &input);

        if let RenderState::Active { window, .. } = &self.state {
            window.request_redraw();
        }
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
        let surface = pollster::block_on(surface_future)
            .expect("create surface");

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
                self.render();
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                self.feed_input(RawInputKind::PointerMoved {
                    pointer: PointerId::MOUSE,
                    x: position.x,
                    y: position.y,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_input(state, button);
            }
            _ => {}
        }
    }
}
