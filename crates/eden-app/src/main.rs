//! Eden — the application binary.
//!
//! Phase 1 ("The Surface"): the window now hosts the editor chrome — title bar,
//! sidebar, tab strip, editor canvas, and status bar — laid out by taffy, drawn
//! through vello, and themed. Motion is live: `B` toggles the sidebar (its width
//! springs), `T` crossfades between the three built-in themes, and hovering the
//! sidebar or tabs eases in a highlight. With `ControlFlow::Wait` the app idles
//! at zero cost and only schedules frames while a spring is in motion.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context as _, Result};
use eden_motion::MotionPrefs;
use eden_ui::Chrome;
use vello::kurbo::Point;
use vello::peniko::Color;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

// design: the clear colour shows only on frames the scene doesn't fully cover
// (it always does), so a neutral paper white matching the default Eden Day theme
// avoids any first-frame flash.
const CLEAR: Color = Color::from_rgb8(0xFB, 0xF8, 0xF3);

fn main() -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("eden=info,warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let event_loop = EventLoop::new().context("create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new();
    event_loop.run_app(&mut app).context("run event loop")?;
    app.into_result()
}

/// Top-level application state, driven by winit's [`ApplicationHandler`].
struct App {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: WindowState,
    scene: Scene,
    chrome: Option<Chrome>,
    prefs: MotionPrefs,
    last_frame: Instant,
    fatal: Option<anyhow::Error>,
}

enum WindowState {
    Suspended,
    Active(Box<ActiveWindow>),
}

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
            chrome: None,
            prefs: MotionPrefs::from_env(),
            last_frame: Instant::now(),
            fatal: None,
        }
    }

    fn into_result(self) -> Result<()> {
        match self.fatal {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

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
            wgpu::PresentMode::AutoVsync,
        ))
        .context("create surface")?;

        self.ensure_renderer(surface.dev_id)?;
        Ok(ActiveWindow { window, surface })
    }

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
                    antialiasing_support: AaSupport::area_only(),
                    num_init_threads: std::num::NonZeroUsize::new(1),
                    pipeline_cache: None,
                },
            )
            .map_err(|e| anyhow::anyhow!("initialise vello renderer: {e:?}"))?;
            self.renderers[dev_id] = Some(renderer);
        }
        Ok(())
    }

    fn request_redraw(&self) {
        if let WindowState::Active(active) = &self.state {
            active.window.request_redraw();
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let WindowState::Active(active) = &mut self.state else {
            return;
        };
        self.context.resize_surface(&mut active.surface, width, height);
        let scale = active.window.scale_factor();
        if let Some(chrome) = &mut self.chrome {
            chrome.resize(f64::from(width), f64::from(height), scale);
        }
        active.window.request_redraw();
    }

    /// Handles a key press. Returns `true` if it changed anything visible.
    fn on_key(&mut self, code: KeyCode) -> bool {
        let Some(chrome) = &mut self.chrome else {
            return false;
        };
        match code {
            KeyCode::KeyB => {
                chrome.toggle_sidebar();
                true
            }
            KeyCode::KeyT => {
                let next = chrome.active_theme_name().to_owned();
                chrome.cycle_theme();
                tracing::info!(from = %next, to = %chrome.active_theme_name(), "theme cycled");
                true
            }
            _ => false,
        }
    }

    fn render(&mut self) -> Result<()> {
        // Advance animations by the real elapsed time since the last frame.
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        let animating = self.chrome.as_mut().is_some_and(|c| c.step(dt));

        let WindowState::Active(active) = &mut self.state else {
            return Ok(());
        };
        let width = active.surface.config.width;
        let height = active.surface.config.height;
        let dev_id = active.surface.dev_id;

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
        if let Some(chrome) = &mut self.chrome {
            chrome.paint(&mut self.scene);
        }

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
                    base_color: CLEAR,
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| anyhow::anyhow!("vello render: {e:?}"))?;

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

        // Keep the frame loop alive only while something is still moving.
        if animating {
            active.window.request_redraw();
        }
        Ok(())
    }

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
        let active = match self.activate(event_loop) {
            Ok(active) => active,
            Err(err) => return self.fail(event_loop, err),
        };

        if self.chrome.is_none() {
            let size = active.window.inner_size();
            let scale = active.window.scale_factor();
            match Chrome::new(
                f64::from(size.width.max(1)),
                f64::from(size.height.max(1)),
                scale,
                self.prefs,
            ) {
                Ok(chrome) => self.chrome = Some(chrome),
                Err(err) => return self.fail(event_loop, anyhow::anyhow!("build chrome: {err}")),
            }
        }

        self.last_frame = Instant::now();
        active.window.request_redraw();
        self.state = WindowState::Active(Box::new(active));
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
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
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_hover(Some(Point::new(position.x, position.y)));
                }
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(chrome) = &mut self.chrome {
                    chrome.set_hover(None);
                }
                self.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                let changed = self.on_key(code);
                if changed {
                    self.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = self.render() {
                    self.fail(event_loop, err);
                }
            }
            _ => {}
        }
    }
}
