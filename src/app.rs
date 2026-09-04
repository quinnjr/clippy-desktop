//! The window, its OpenGL surface, and the event loop.
//!
//! The window is borderless, transparent and (by default) always on top. It is
//! created through `glutin-winit` so that, on X11, it gets the 32-bit visual of
//! the alpha-capable OpenGL config; on Wayland, Windows and macOS the OpenGL
//! surface carries the alpha channel itself.

use crate::agent::Agent;
use crate::companion::Companion;
use crate::options::Options;
use crate::platform::{self, ContextMenuHandle, MenuAction};
use crate::renderer::Renderer;
use crate::sprite::SpriteSheet;
use glutin::config::{Config, ConfigTemplateBuilder, GetGlConfig};
use glutin::context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext, Version,
};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Mutex;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

/// Pointer travel, in physical pixels, before a press becomes a drag.
const DRAG_THRESHOLD: f64 = 4.0;
/// Gap between Clippy and the screen edges when he first appears, in logical pixels.
const MARGIN_RIGHT: f64 = 48.0;
const MARGIN_BOTTOM: f64 = 96.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserEvent {
    Menu(MenuAction),
}

/// Runs Clippy until the user quits.
pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let agent = Agent::clippy();
    let sheet = SpriteSheet::clippy();
    agent.validate(sheet.width(), sheet.height())?;

    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        eprintln!(
            "Wayland session: the compositor decides where Clippy appears and whether he stays on top. \
             Drag him to move him; press Escape or Q to quit."
        );
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = Mutex::new(event_loop.create_proxy());
    platform::install_menu_event_handler(move |action| {
        if let Ok(proxy) = proxy.lock() {
            let _ = proxy.send_event(UserEvent::Menu(action));
        }
    });
    let menu = match ContextMenuHandle::new() {
        Ok(menu) => Some(menu),
        Err(err) => {
            eprintln!("clippy-app: no context menu: {err}");
            None
        }
    };

    let mut app = App::new(options, agent, sheet, menu);
    event_loop.run_app(&mut app)?;
    app.exit_state
}

enum DisplayState {
    Pending(Box<DisplayBuilder>),
    Created,
}

struct WindowState {
    surface: Surface<WindowSurface>,
    renderer: Renderer,
    /// Declared last: everything above borrows the window's native handle.
    window: Window,
}

struct App {
    options: Options,
    sheet: SpriteSheet,
    companion: Companion,
    menu: Option<ContextMenuHandle>,
    template: ConfigTemplateBuilder,
    display: DisplayState,
    /// Declared before `gl_context`: the surface and GL objects need the context alive.
    state: Option<WindowState>,
    gl_context: Option<PossiblyCurrentContext>,
    frame_pixels: Vec<u8>,
    /// Frame currently in the texture.
    uploaded: Option<(String, usize)>,
    cursor: PhysicalPosition<f64>,
    /// Where the left button went down, until it becomes a drag or a click.
    press: Option<PhysicalPosition<f64>>,
    exit_state: Result<(), Box<dyn Error>>,
}

impl App {
    fn new(
        options: Options,
        agent: Agent,
        sheet: SpriteSheet,
        menu: Option<ContextMenuHandle>,
    ) -> Self {
        // Ask for an alpha channel everywhere. Only macOS needs the explicit
        // transparency flag (it can only report one config at a time); the
        // other platforms return several configs and `pick_config` prefers
        // the transparent one.
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(cfg!(target_os = "macos"));
        Self {
            options,
            sheet,
            companion: Companion::new(agent, Instant::now()),
            menu,
            template,
            display: DisplayState::Pending(Box::new(DisplayBuilder::new())),
            state: None,
            gl_context: None,
            frame_pixels: Vec::new(),
            uploaded: None,
            cursor: PhysicalPosition::new(0.0, 0.0),
            press: None,
            exit_state: Ok(()),
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: Box<dyn Error>) {
        self.exit_state = Err(err);
        event_loop.exit();
    }

    fn window_attributes(&self, event_loop: &ActiveEventLoop) -> WindowAttributes {
        let (frame_width, frame_height) = self.companion.agent().frame_size();
        let scale = self.options.scale;
        let logical = LogicalSize::new((frame_width * scale) as f64, (frame_height * scale) as f64);
        let level = if self.options.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        };
        let mut attributes = Window::default_attributes()
            .with_title("Clippy")
            .with_inner_size(logical)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(level);
        if let Some(position) = initial_position(event_loop, logical) {
            attributes = attributes.with_position(position);
        }
        attributes
    }

    fn request_redraw(&self) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }

    fn poke(&mut self) {
        self.companion.poke(Instant::now());
        self.request_redraw();
    }

    fn redraw(&mut self) {
        let Some(state) = self.state.as_ref() else {
            return;
        };
        let Some(gl_context) = self.gl_context.as_ref() else {
            return;
        };
        let key = self.companion.frame_key();
        if key != self.uploaded {
            let (frame_width, frame_height) = self.companion.agent().frame_size();
            let cells: &[[u32; 2]] = self
                .companion
                .frame()
                .map(|frame| frame.images.as_slice())
                .unwrap_or(&[]);
            self.sheet
                .compose(cells, frame_width, frame_height, &mut self.frame_pixels);
            match state.renderer.upload(&self.frame_pixels) {
                Ok(()) => self.uploaded = key,
                Err(err) => eprintln!("clippy-app: {err}"),
            }
        }
        state.renderer.draw();
        state.window.pre_present_notify();
        if let Err(err) = state.surface.swap_buffers(gl_context) {
            eprintln!("clippy-app: could not present the frame: {err}");
        }
    }

    fn start_drag(&mut self) {
        self.press = None;
        if let Some(state) = &self.state {
            // Unsupported on some setups; a failed drag is not an error worth reporting.
            let _ = state.window.drag_window();
        }
    }

    fn right_click(&mut self) {
        if platform::HAS_NATIVE_MENU {
            if let (Some(menu), Some(state)) = (&self.menu, &self.state) {
                platform::show_context_menu(menu, &state.window);
                return;
            }
        }
        self.poke();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = self.window_attributes(event_loop);
        let (window, gl_config) = match std::mem::replace(&mut self.display, DisplayState::Created)
        {
            DisplayState::Pending(builder) => {
                let built = builder.with_window_attributes(Some(attributes)).build(
                    event_loop,
                    self.template.clone(),
                    pick_config,
                );
                match built {
                    Ok((Some(window), gl_config)) => {
                        match create_gl_context(&window, &gl_config) {
                            Ok(context) => {
                                self.gl_context = Some(context.treat_as_possibly_current())
                            }
                            Err(err) => return self.fail(event_loop, err),
                        }
                        (window, gl_config)
                    }
                    Ok((None, _)) => return self.fail(event_loop, "no window was created".into()),
                    Err(err) => return self.fail(event_loop, err),
                }
            }
            DisplayState::Created => {
                let Some(gl_context) = self.gl_context.as_ref() else {
                    return self.fail(event_loop, "OpenGL context is missing".into());
                };
                let gl_config = gl_context.config();
                match glutin_winit::finalize_window(event_loop, attributes, &gl_config) {
                    Ok(window) => (window, gl_config),
                    Err(err) => return self.fail(event_loop, err.into()),
                }
            }
        };

        let surface_attributes = match window.build_surface_attributes(Default::default()) {
            Ok(attributes) => attributes,
            Err(err) => return self.fail(event_loop, err.into()),
        };
        let gl_display = gl_config.display();
        // SAFETY: the attributes were built from this very window, which outlives the surface.
        let surface =
            match unsafe { gl_display.create_window_surface(&gl_config, &surface_attributes) } {
                Ok(surface) => surface,
                Err(err) => return self.fail(event_loop, err.into()),
            };
        let gl_context = self.gl_context.as_ref().expect("context is created above");
        if let Err(err) = gl_context.make_current(&surface) {
            return self.fail(event_loop, err.into());
        }

        let (frame_width, frame_height) = self.companion.agent().frame_size();
        // SAFETY: the context was made current just above and stays current
        // for the life of the window; the renderer is dropped before it.
        let renderer = match unsafe {
            Renderer::new(
                |symbol| gl_display.get_proc_address(symbol),
                frame_width,
                frame_height,
            )
        } {
            Ok(renderer) => renderer,
            Err(err) => return self.fail(event_loop, err.into()),
        };
        if let Err(err) = surface.set_swap_interval(gl_context, SwapInterval::Wait(NonZeroU32::MIN))
        {
            eprintln!("clippy-app: could not enable vsync: {err}");
        }
        let size = window.inner_size();
        renderer.resize(size.width, size.height);

        self.uploaded = None;
        self.state = Some(WindowState {
            surface,
            renderer,
            window,
        });
        self.redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) if size.width != 0 && size.height != 0 => {
                if let (Some(state), Some(gl_context)) = (&self.state, &self.gl_context) {
                    state.surface.resize(
                        gl_context,
                        NonZeroU32::new(size.width).expect("checked above"),
                        NonZeroU32::new(size.height).expect("checked above"),
                    );
                    state.renderer.resize(size.width, size.height);
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        repeat: false,
                        ..
                    },
                ..
            } => match logical_key {
                Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Character(text) if text.eq_ignore_ascii_case("q") => event_loop.exit(),
                Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter) => self.poke(),
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                if let Some(press) = self.press {
                    let dx = position.x - press.x;
                    let dy = position.y - press.y;
                    if (dx * dx + dy * dy).sqrt() >= DRAG_THRESHOLD {
                        self.start_drag();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => self.press = Some(self.cursor),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.press.take().is_some() {
                    self.poke();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Right,
                ..
            } => self.right_click(),
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(MenuAction::Animate) => self.poke(),
            UserEvent::Menu(MenuAction::Quit) => event_loop.exit(),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            return;
        }
        if self.companion.advance(Instant::now()) {
            self.request_redraw();
        }
        let flow = match self.companion.next_deadline() {
            Some(deadline) => ControlFlow::WaitUntil(deadline),
            None => ControlFlow::Wait,
        };
        event_loop.set_control_flow(flow);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // GL objects and the surface go first, while the context is current.
        self.state = None;
        if let Some(gl_context) = self.gl_context.take() {
            let gl_display = gl_context.display();
            drop(gl_context);
            // The NVIDIA driver crashes if the Wayland connection is closed
            // before the EGL display, so terminate the display explicitly.
            #[cfg(all(any(windows, unix), not(any(target_os = "macos", target_os = "ios"))))]
            if let glutin::display::Display::Egl(egl_display) = gl_display {
                // SAFETY: every context and surface created from this display was dropped above.
                unsafe { egl_display.terminate() };
            }
            #[cfg(not(all(
                any(windows, unix),
                not(any(target_os = "macos", target_os = "ios"))
            )))]
            drop(gl_display);
        }
    }
}

/// Bottom-right corner of the primary monitor, with a margin; `None` where the
/// system does not let applications place windows (Wayland).
fn initial_position(
    event_loop: &ActiveEventLoop,
    window: LogicalSize<f64>,
) -> Option<PhysicalPosition<i32>> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())?;
    let scale = monitor.scale_factor();
    let screen = monitor.size();
    if screen.width == 0 || screen.height == 0 {
        return None;
    }
    let origin = monitor.position();
    let size: PhysicalSize<u32> = window.to_physical(scale);
    let x = origin.x as f64 + screen.width as f64 - size.width as f64 - MARGIN_RIGHT * scale;
    let y = origin.y as f64 + screen.height as f64 - size.height as f64 - MARGIN_BOTTOM * scale;
    Some(PhysicalPosition::new(
        x.max(origin.x as f64).round() as i32,
        y.max(origin.y as f64).round() as i32,
    ))
}

/// Prefers a config with real transparency, then the one with the fewest
/// multisample buffers (pixel art needs none).
fn pick_config(configs: Box<dyn Iterator<Item = Config> + '_>) -> Config {
    configs
        .reduce(|best, candidate| {
            let best_alpha = best.supports_transparency().unwrap_or(false);
            let candidate_alpha = candidate.supports_transparency().unwrap_or(false);
            let better = (candidate_alpha && !best_alpha)
                || (candidate_alpha == best_alpha && candidate.num_samples() < best.num_samples());
            if better {
                candidate
            } else {
                best
            }
        })
        .expect("glutin offers at least one matching config")
}

/// Creates the newest OpenGL context available, falling back to OpenGL ES and
/// then to OpenGL 2.1 for old drivers.
fn create_gl_context(
    window: &Window,
    gl_config: &Config,
) -> Result<NotCurrentContext, Box<dyn Error>> {
    let raw_window_handle = window.window_handle().ok().map(|handle| handle.as_raw());
    let attempts = [
        ContextAttributesBuilder::new().build(raw_window_handle),
        ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(raw_window_handle),
        ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(Some(Version::new(2, 1))))
            .build(raw_window_handle),
    ];
    let gl_display = gl_config.display();
    let mut last_error = None;
    for attributes in &attempts {
        // SAFETY: the raw window handle belongs to `window`, which is alive.
        match unsafe { gl_display.create_context(gl_config, attributes) } {
            Ok(context) => return Ok(context),
            Err(err) => last_error = Some(err),
        }
    }
    Err(Box::new(last_error.expect("at least one attempt was made")))
}
