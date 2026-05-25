//! Eden — the application binary.
//!
//! Phase 0 ("Skeleton"): open a window via winit, bring up a wgpu device through
//! vello's [`RenderContext`], and render a single rounded rectangle in the brand
//! colour. This is the "hello, GPU" proof that the rendering spine is alive; the
//! widget tree, motion system, and editor are layered on in later phases.

use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use vello::kurbo::{Affine, RoundedRect};
use vello::peniko::{Color, Fill};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

// design: Eden Day palette (§6). A warm paper-white field is the canvas; the
// kingfisher-blue brand accent is the single shape. No neon, nothing above 75%
// saturation — the same restraint the whole product is held to.
const EDEN_DAY_PAPER: Color = Color::from_rgb8(0xFB, 0xF8, 0xF3);
const EDEN_KINGFISHER: Color = Color::from_rgb8(0x2A, 0x6B, 0xC8);

fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("eden=info,warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let event_loop = EventLoop::new().context("create event loop")?;
    // design: Wait, not Poll. A static surface should idle at 0% CPU; later
    // phases drive redraws explicitly from the motion system, not a busy loop.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app).context("run event loop")?;
    app.into_result()
}

/// Top-level application state, driven by winit's [`ApplicationHandler`].
struct App {
    context: RenderContext,
    /// One renderer per wgpu device, indexed by `RenderSurface::dev_id`.
    renderers: Vec<Option<Renderer>>,
    state: WindowState,
    scene: Scene,
    /// Captures the first fatal error so `main` can surface it after the loop.
    fatal: Option<anyhow::Error>,
}

/// Whether a surface currently exists. winit may suspend/resume the app, so the
/// renderable surface is created lazily in `resumed` and torn down on suspend.
enum WindowState {
    Suspended,
    // design: boxed to keep the enum small — `Active` is far larger than
    // `Suspended`, which would otherwise trip `clippy::large_enum_variant`.
    Active(Box<ActiveWindow>),
}

/// A live window together with its configured render surface.
struct ActiveWindow {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
}

impl App {
    fn new() -> Self {
        Self {
            context: RenderContext::new(),
            renderers: Vec::new(),
            state: WindowState::Suspended,
            scene: Scene::new(),
            fatal: None,
        }
    }

    fn into_result(self) -> Result<()> {
        match self.fatal {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Creates the window and its render surface. Separated out so the fallible
    /// setup can use `?` and report a single error to `resumed`.
    fn activate(&mut self, event_loop: &ActiveEventLoop) -> Result<ActiveWindow> {
        let attributes = WindowAttributes::default()
            .with_title("Eden")
            .with_inner_size(LogicalSize::new(1100.0, 720.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .context("create window")?,
        );

        let size = window.inner_size();
        let surface = pollster::block_on(self.context.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            // design: AutoVsync is the 60Hz floor; the 120Hz target on capable
            // displays is a Phase 7 concern (present-mode negotiation).
            wgpu::PresentMode::AutoVsync,
        ))
        .context("create surface")?;

        self.ensure_renderer(surface.dev_id)?;
        Ok(ActiveWindow { window, surface })
    }

    /// Lazily builds a vello renderer for the given device if one is missing.
    fn ensure_renderer(&mut self, dev_id: usize) -> Result<()> {
        if self.renderers.len() <= dev_id {
            self.renderers.resize_with(dev_id + 1, || None);
        }
        if self.renderers[dev_id].is_none() {
            let device = &self.context.devices[dev_id].device;
            let renderer = Renderer::new(
                device,
                RendererOptions {
                    use_cpu: false,
                    // design: we only ever request `AaConfig::Area`, so compile
                    // just that pipeline permutation and keep startup lean (§9).
                    antialiasing_support: AaSupport::area_only(),
                    num_init_threads: NonZeroUsize::new(1),
                    pipeline_cache: None,
                },
            )
            .map_err(|e| anyhow::anyhow!("initialise vello renderer: {e:?}"))?;
            self.renderers[dev_id] = Some(renderer);
        }
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let WindowState::Active(active) = &mut self.state {
            self.context.resize_surface(&mut active.surface, width, height);
            active.window.request_redraw();
        }
    }

    fn render(&mut self) -> Result<()> {
        let WindowState::Active(active) = &mut self.state else {
            return Ok(());
        };
        let width = active.surface.config.width;
        let height = active.surface.config.height;
        let dev_id = active.surface.dev_id;

        // wgpu 29 returns a status enum rather than a `Result`. Skip transient
        // frames; reconfigure and retry on the recoverable surface-loss cases.
        use wgpu::CurrentSurfaceTexture as Acquired;
        let surface_texture = match active.surface.surface.get_current_texture() {
            Acquired::Success(texture) | Acquired::Suboptimal(texture) => texture,
            Acquired::Timeout | Acquired::Occluded => return Ok(()),
            Acquired::Outdated | Acquired::Lost => {
                self.context.configure_surface(&active.surface);
                active.window.request_redraw();
                return Ok(());
            }
            Acquired::Validation => anyhow::bail!("surface texture validation error"),
        };

        self.scene.reset();
        draw_brand_mark(&mut self.scene, f64::from(width), f64::from(height));

        let device_handle = &self.context.devices[dev_id];
        let renderer = self.renderers[dev_id]
            .as_mut()
            .context("no renderer for surface device")?;

        renderer
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &active.surface.target_view,
                &RenderParams {
                    base_color: EDEN_DAY_PAPER,
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| anyhow::anyhow!("vello render: {e:?}"))?;

        // vello renders to a storage texture; blit it onto the swapchain image.
        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("eden.surface_blit"),
                });
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(active.surface.format),
                ..Default::default()
            });
        active.surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &active.surface.target_view,
            &surface_view,
        );
        device_handle.queue.submit([encoder.finish()]);
        active.window.pre_present_notify();
        surface_texture.present();
        Ok(())
    }

    /// Records the first fatal error and asks the loop to exit cleanly.
    fn fail(&mut self, event_loop: &ActiveEventLoop, err: anyhow::Error) {
        tracing::error!("{err:#}");
        if self.fatal.is_none() {
            self.fatal = Some(err);
        }
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if matches!(self.state, WindowState::Active(_)) {
            return;
        }
        match self.activate(event_loop) {
            Ok(active) => {
                active.window.request_redraw();
                self.state = WindowState::Active(Box::new(active));
            }
            Err(err) => self.fail(event_loop, err),
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // Drop the surface (and its window) but keep renderers — the device
        // outlives a suspend/resume cycle, so we can rebuild a surface cheaply.
        self.state = WindowState::Suspended;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render() {
                    self.fail(event_loop, err);
                }
            }
            _ => {}
        }
    }
}

/// Draws the Phase 0 brand mark: a single rounded rectangle in kingfisher blue,
/// inset from the edges of a paper-white field.
fn draw_brand_mark(scene: &mut Scene, width: f64, height: f64) {
    // design: corner radius 16, the top of the §6 radius scale (4/8/12/16).
    // The inset scales with the smaller dimension so the mark stays centred and
    // proportional as the window resizes, clamped to sane bounds.
    let inset = (width.min(height) * 0.18).clamp(24.0, 160.0);
    let rect = RoundedRect::new(inset, inset, width - inset, height - inset, 16.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, EDEN_KINGFISHER, None, &rect);
}
