//! The window, its OpenGL surface, the chat balloon, and the event loop.
//!
//! The window is borderless, transparent and (by default) always on top. When
//! the chat balloon is closed it is exactly Clippy's size; when it opens, the
//! window grows upward to hold the balloon above him and shrinks again when it
//! closes. Clippy is drawn by the sprite renderer into the bottom of the
//! window; the balloon is drawn by egui over the top, both on the same GL
//! context so the whole thing stays a single see-through window.

use crate::agent::Agent;
use crate::chat::{self, ChatEvent, ChatState, Status};
use crate::companion::Companion;
use crate::options::Options;
use crate::platform::{self, ContextMenuHandle, MenuAction};
use crate::renderer::Renderer;
use crate::sprite::SpriteSheet;
use crate::ui::{self, BalloonAction, BalloonLayout};
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

/// Pointer travel, in physical pixels, before a press becomes a drag.
const DRAG_THRESHOLD: f64 = 4.0;
/// Gap between Clippy and the screen edges when he first appears, in logical pixels.
const MARGIN_RIGHT: f64 = 48.0;
const MARGIN_BOTTOM: f64 = 96.0;

/// Balloon geometry, in logical pixels (egui points).
const BALLOON_W: f64 = 340.0;
const BALLOON_H: f64 = 250.0;
const TAIL: f64 = 18.0;
const BALLOON_PAD: f32 = 4.0;

/// Frame budget while the balloon is open, so the caret blinks and streamed
/// text appears promptly without spinning the CPU.
const OPEN_FRAME: Duration = Duration::from_millis(33);

/// Animations Clippy plays around a chat.
const THINKING: &str = "Thinking";
const WRITING: &str = "Writing";
const ANSWERED: &str = "GetAttention";
const FAILED: &str = "Alert";

#[derive(Debug, Clone)]
pub enum UserEvent {
    Menu(MenuAction),
    Chat { generation: u64, event: ChatEvent },
}

/// Runs Clippy until the user quits.
pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let agent = Agent::clippy();
    let sheet = SpriteSheet::clippy();
    agent.validate(sheet.width(), sheet.height())?;

    let chat_config = chat::resolve(&options.chat);
    if let Err(unconfigured) = &chat_config {
        eprintln!(
            "clippy-app: {}",
            unconfigured.message.replace('\n', "\nclippy-app: ")
        );
        eprintln!("clippy-app: (Clippy will still keep you company.)");
    }

    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        eprintln!(
            "clippy-app: Wayland session; the compositor places Clippy and decides stacking. \
             Drag him to move him."
        );
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let menu_proxy = Mutex::new(proxy.clone());
    platform::install_menu_event_handler(move |action| {
        if let Ok(proxy) = menu_proxy.lock() {
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

    let mut app = App::new(options, agent, sheet, menu, chat_config, proxy);
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
    egui_painter: egui_glow::Painter,
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
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    frame_pixels: Vec<u8>,
    /// Frame currently in the texture.
    uploaded: Option<(String, usize)>,
    cursor: PhysicalPosition<f64>,
    /// Where the left button went down, until it becomes a drag or a click.
    press: Option<PhysicalPosition<f64>>,
    chat: ChatState,
    balloon_open: bool,
    balloon_input: String,
    balloon_focus: bool,
    proxy: EventLoopProxy<UserEvent>,
    /// Bumped for each request so late events from an old one are ignored.
    generation: u64,
    /// Cancels the in-flight request's worker thread.
    cancel: Arc<AtomicBool>,
    exit_state: Result<(), Box<dyn Error>>,
}

impl App {
    fn new(
        options: Options,
        agent: Agent,
        sheet: SpriteSheet,
        menu: Option<ContextMenuHandle>,
        chat_config: Result<chat::ChatConfig, chat::Unconfigured>,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Self {
        // Ask for an alpha channel everywhere. Only macOS needs the explicit
        // transparency flag; the other platforms return several configs and
        // `pick_config` prefers the transparent one.
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(cfg!(target_os = "macos"));
        let egui_ctx = egui::Context::default();
        egui_ctx.set_visuals(egui::Visuals::light());
        Self {
            options,
            sheet,
            companion: Companion::new(agent, Instant::now()),
            menu,
            template,
            display: DisplayState::Pending(Box::new(DisplayBuilder::new())),
            state: None,
            gl_context: None,
            egui_ctx,
            egui_state: None,
            frame_pixels: Vec::new(),
            uploaded: None,
            cursor: PhysicalPosition::new(0.0, 0.0),
            press: None,
            chat: ChatState::new(chat_config),
            balloon_open: false,
            balloon_input: String::new(),
            balloon_focus: false,
            proxy,
            generation: 0,
            cancel: Arc::new(AtomicBool::new(false)),
            exit_state: Ok(()),
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, err: Box<dyn Error>) {
        self.exit_state = Err(err);
        event_loop.exit();
    }

    /// Clippy's on-screen size, in logical pixels.
    fn clippy_logical(&self) -> (f64, f64) {
        let (fw, fh) = self.companion.agent().frame_size();
        let scale = self.options.scale as f64;
        (fw as f64 * scale, fh as f64 * scale)
    }

    /// The window's logical size for the current balloon state.
    fn window_logical(&self) -> LogicalSize<f64> {
        let (clw, clh) = self.clippy_logical();
        if self.balloon_open {
            LogicalSize::new(clw.max(BALLOON_W), BALLOON_H + TAIL + clh)
        } else {
            LogicalSize::new(clw, clh)
        }
    }

    fn window_attributes(&self, event_loop: &ActiveEventLoop) -> WindowAttributes {
        let logical = self.window_logical();
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

    /// Opens or closes the balloon, resizing the window and keeping Clippy
    /// anchored where he is.
    fn set_balloon(&mut self, open: bool) {
        if self.balloon_open == open {
            return;
        }
        // Closing mid-reply abandons the request so Clippy does not keep
        // thinking with no balloon (and later react out of nowhere).
        if !open {
            self.cancel_request(Instant::now());
        }
        self.balloon_open = open;
        if open {
            self.balloon_focus = true;
        }

        let new_logical = self.window_logical();
        let Some(state) = &self.state else {
            return;
        };
        let scale_factor = state.window.scale_factor();
        let old = state.window.inner_size();
        // Without a readable position (e.g. Wayland) the compositor owns
        // placement; just ask for the new size.
        let Ok(old_pos) = state.window.outer_position() else {
            self.apply_inner_size(state.window.request_inner_size(new_logical));
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
            return;
        };
        let new_w = (new_logical.width * scale_factor).round() as i32;
        let new_h = (new_logical.height * scale_factor).round() as i32;
        let clippy_center_x = old_pos.x + old.width as i32 / 2;
        let clippy_bottom = old_pos.y + old.height as i32;
        let applied = state.window.request_inner_size(new_logical);
        state.window.set_outer_position(PhysicalPosition::new(
            clippy_center_x - new_w / 2,
            clippy_bottom - new_h,
        ));
        state.window.request_redraw();
        self.apply_inner_size(applied);
    }

    /// If `request_inner_size` was applied synchronously (no `Resized` event
    /// follows), resize the GL surface now so the next frame matches.
    fn apply_inner_size(&self, applied: Option<PhysicalSize<u32>>) {
        let (Some(size), Some(state), Some(gl_context)) =
            (applied, self.state.as_ref(), self.gl_context.as_ref())
        else {
            return;
        };
        if let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            state.surface.resize(gl_context, width, height);
        }
    }

    /// Cancels an in-flight chat request and stops Clippy thinking.
    fn cancel_request(&mut self, now: Instant) {
        if self.chat.is_busy() {
            self.cancel.store(true, Ordering::Relaxed);
            // Ignore any late events from the retired worker.
            self.generation += 1;
            self.chat.cancel();
            self.companion.release(now);
        }
    }

    /// Moves Clippy back to his starting corner (in case he was dragged away).
    fn recenter(&mut self, event_loop: &ActiveEventLoop) {
        let logical = self.window_logical();
        let Some(position) = initial_position(event_loop, logical) else {
            return;
        };
        if let Some(state) = &self.state {
            state.window.set_outer_position(position);
            state.window.request_redraw();
        }
    }

    fn right_click(&mut self) {
        if platform::HAS_NATIVE_MENU {
            if let (Some(menu), Some(state)) = (&self.menu, &self.state) {
                if platform::show_context_menu(menu, &state.window) {
                    return;
                }
            }
        }
        // No native menu (Linux), or it could not be shown: open the chat.
        self.set_balloon(true);
    }

    /// Sends a prompt to the server on a worker thread, and sets Clippy thinking.
    fn submit_prompt(&mut self, text: String) {
        let Some(config) = self.chat.config().cloned() else {
            self.balloon_focus = true;
            self.request_redraw();
            return;
        };
        // Retire any in-flight request.
        self.cancel.store(true, Ordering::Relaxed);
        self.generation += 1;
        let generation = self.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = cancel.clone();

        let messages = self.chat.begin_user(&text);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            chat::stream(&config, &messages, &cancel, |event| {
                let _ = proxy.send_event(UserEvent::Chat { generation, event });
            });
        });

        self.companion.hold_animation(THINKING, Instant::now());
        self.request_redraw();
    }

    fn on_chat_event(&mut self, event: ChatEvent) {
        let now = Instant::now();
        match event {
            ChatEvent::Delta(text) => {
                let was_waiting = matches!(self.chat.status(), Status::Waiting);
                self.chat.on_delta(&text);
                if was_waiting {
                    self.companion.hold_animation(WRITING, now);
                }
            }
            ChatEvent::Done => {
                let replied = self.chat.on_done();
                self.companion.release(now);
                // Only celebrate if a reply actually arrived.
                if replied {
                    self.companion.perform(ANSWERED, now);
                }
            }
            ChatEvent::Error(message) => {
                self.chat.on_error(message);
                self.companion.release(now);
                self.companion.perform(FAILED, now);
            }
        }
        self.request_redraw();
    }

    fn redraw(&mut self) {
        if self.gl_context.is_none() {
            return;
        }
        let balloon_open = self.balloon_open;
        let (clw, clh) = self.clippy_logical();
        let mut action = BalloonAction::None;

        {
            let Some(state) = self.state.as_mut() else {
                return;
            };
            let gl_context = self.gl_context.as_ref().expect("checked above");
            let WindowState {
                surface,
                renderer,
                egui_painter,
                window,
            } = state;

            // Upload the current sprite frame if it changed.
            let key = self.companion.frame_key();
            if key != self.uploaded {
                let (fw, fh) = self.companion.agent().frame_size();
                let cells: &[[u32; 2]] = self
                    .companion
                    .frame()
                    .map(|frame| frame.images.as_slice())
                    .unwrap_or(&[]);
                self.sheet.compose(cells, fw, fh, &mut self.frame_pixels);
                match renderer.upload(&self.frame_pixels) {
                    Ok(()) => self.uploaded = key,
                    Err(err) => eprintln!("clippy-app: {err}"),
                }
            }

            let inner = window.inner_size();
            let scale_factor = window.scale_factor() as f32;

            renderer.clear();
            // Clippy fills the window when closed, or its bottom-centre when open.
            let (clippy_w, clippy_h, x0) = if balloon_open {
                let cw = (clw as f32 * scale_factor).round() as i32;
                let ch = (clh as f32 * scale_factor).round() as i32;
                (cw, ch, (inner.width as i32 - cw) / 2)
            } else {
                (inner.width as i32, inner.height as i32, 0)
            };
            renderer.draw(x0, 0, clippy_w, clippy_h);

            if balloon_open {
                let egui_ctx = self.egui_ctx.clone();
                let egui_state = self.egui_state.as_mut().expect("created with the window");
                let raw_input = egui_state.take_egui_input(window);
                let layout = balloon_layout(inner, scale_factor, clh as f32);

                let chat = &self.chat;
                let input = &mut self.balloon_input;
                let focus = &mut self.balloon_focus;
                let full = egui_ctx.run(raw_input, |ctx| {
                    action = ui::show(ctx, &layout, chat, input, focus);
                });
                egui_state.handle_platform_output(window, full.platform_output);
                let primitives = egui_ctx.tessellate(full.shapes, full.pixels_per_point);
                egui_painter.paint_and_update_textures(
                    [inner.width, inner.height],
                    full.pixels_per_point,
                    &primitives,
                    &full.textures_delta,
                );
            }

            window.pre_present_notify();
            if let Err(err) = surface.swap_buffers(gl_context) {
                eprintln!("clippy-app: could not present the frame: {err}");
            }
        }

        match action {
            BalloonAction::Submit(text) => self.submit_prompt(text),
            BalloonAction::Close => self.set_balloon(false),
            BalloonAction::None => {}
        }
    }

    fn start_drag(&mut self) {
        self.press = None;
        if let Some(state) = &self.state {
            // Unsupported on some setups; a failed drag is not worth reporting.
            let _ = state.window.drag_window();
        }
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

        // Load GL once and share it between the sprite renderer and egui.
        let gl = Arc::new(unsafe {
            glow::Context::from_loader_function_cstr(|symbol| gl_display.get_proc_address(symbol))
        });

        let (frame_width, frame_height) = self.companion.agent().frame_size();
        let renderer = match Renderer::new(gl.clone(), frame_width, frame_height) {
            Ok(renderer) => renderer,
            Err(err) => return self.fail(event_loop, err.into()),
        };
        let egui_painter = match egui_glow::Painter::new(gl, "", None, false) {
            Ok(painter) => painter,
            Err(err) => return self.fail(event_loop, format!("egui painter: {err}").into()),
        };
        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(egui_painter.max_texture_side()),
        );

        if let Err(err) = surface.set_swap_interval(gl_context, SwapInterval::Wait(NonZeroU32::MIN))
        {
            eprintln!("clippy-app: could not enable vsync: {err}");
        }

        self.uploaded = None;
        self.egui_state = Some(egui_state);
        self.state = Some(WindowState {
            surface,
            renderer,
            egui_painter,
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
        // Feed egui first while the balloon is open.
        let consumed = if let (true, Some(egui_state), Some(state)) = (
            self.balloon_open,
            self.egui_state.as_mut(),
            self.state.as_ref(),
        ) {
            let response = egui_state.on_window_event(&state.window, &event);
            if response.repaint {
                state.window.request_redraw();
            }
            response.consumed
        } else {
            false
        };

        match event {
            WindowEvent::Resized(size) if size.width != 0 && size.height != 0 => {
                if let (Some(state), Some(gl_context)) = (&self.state, &self.gl_context) {
                    state.surface.resize(
                        gl_context,
                        NonZeroU32::new(size.width).expect("checked above"),
                        NonZeroU32::new(size.height).expect("checked above"),
                    );
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } if !consumed => {
                self.on_key(event_loop, event);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                if !consumed {
                    if let Some(press) = self.press {
                        let dx = position.x - press.x;
                        let dy = position.y - press.y;
                        if (dx * dx + dy * dy).sqrt() >= DRAG_THRESHOLD {
                            self.start_drag();
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } if !consumed => {
                self.on_mouse(state, button);
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(MenuAction::Animate) => self.poke(),
            UserEvent::Menu(MenuAction::Chat) => self.set_balloon(true),
            UserEvent::Menu(MenuAction::Quit) => event_loop.exit(),
            UserEvent::Chat { generation, event } if generation == self.generation => {
                self.on_chat_event(event)
            }
            // A late event from a retired request.
            UserEvent::Chat { .. } => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            return;
        }
        if self.companion.advance(Instant::now()) {
            self.request_redraw();
        }
        let mut deadline = self.companion.next_deadline();
        if self.balloon_open {
            let soon = Instant::now() + OPEN_FRAME;
            deadline = Some(deadline.map_or(soon, |existing| existing.min(soon)));
        }
        let flow = match deadline {
            Some(deadline) => ControlFlow::WaitUntil(deadline),
            None => ControlFlow::Wait,
        };
        event_loop.set_control_flow(flow);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.cancel.store(true, Ordering::Relaxed);
        // GL objects and the surface go first, while the context is current.
        if let Some(mut state) = self.state.take() {
            state.egui_painter.destroy();
        }
        self.egui_state = None;
        if let Some(gl_context) = self.gl_context.take() {
            let gl_display = gl_context.display();
            drop(gl_context);
            // The NVIDIA driver crashes if the Wayland connection closes before
            // the EGL display, so terminate the display explicitly.
            #[cfg(all(any(windows, unix), not(any(target_os = "macos", target_os = "ios"))))]
            if let glutin::display::Display::Egl(egl_display) = gl_display {
                // SAFETY: every context and surface from this display was dropped above.
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

impl App {
    fn on_key(&mut self, event_loop: &ActiveEventLoop, event: KeyEvent) {
        if event.state != ElementState::Pressed || event.repeat {
            return;
        }
        match event.logical_key {
            Key::Named(NamedKey::Escape) => {
                if self.balloon_open {
                    self.set_balloon(false);
                } else {
                    event_loop.exit();
                }
            }
            Key::Named(NamedKey::Enter) if !self.balloon_open => self.set_balloon(true),
            Key::Named(NamedKey::Space) if !self.balloon_open => self.poke(),
            Key::Named(NamedKey::Home) if !self.balloon_open => self.recenter(event_loop),
            Key::Character(text) if !self.balloon_open && text.eq_ignore_ascii_case("q") => {
                event_loop.exit()
            }
            _ => {}
        }
    }

    fn on_mouse(&mut self, state: ElementState, button: MouseButton) {
        match (state, button) {
            (ElementState::Pressed, MouseButton::Left) => self.press = Some(self.cursor),
            (ElementState::Released, MouseButton::Left) => {
                if self.press.take().is_some() {
                    self.poke();
                }
            }
            (ElementState::Released, MouseButton::Right) => self.right_click(),
            _ => {}
        }
    }
}

/// Places the balloon in the top of the window, its tail pointing at Clippy's
/// centre. All values in egui points (== logical pixels).
fn balloon_layout(
    inner: PhysicalSize<u32>,
    scale_factor: f32,
    clippy_logical_h: f32,
) -> BalloonLayout {
    let win_w = inner.width as f32 / scale_factor;
    let win_h = inner.height as f32 / scale_factor;
    let body_h = (win_h - clippy_logical_h - TAIL as f32 - BALLOON_PAD).max(48.0);
    BalloonLayout {
        pos: egui::pos2(BALLOON_PAD, BALLOON_PAD),
        size: egui::vec2(win_w - 2.0 * BALLOON_PAD, body_h),
        tail_x: win_w / 2.0,
        tail_y: BALLOON_PAD + body_h,
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

/// Prefers a config with real transparency, then the fewest multisample buffers.
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
