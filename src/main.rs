pub mod animation;
pub mod sprite;
pub mod companion;
pub mod renderer;
pub mod platform;

use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, MouseButton, ElementState};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId, WindowAttributes};
use winit::dpi::PhysicalPosition;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sprite::SpriteSheet;
use companion::CompanionState;
use renderer::Renderer;

const TARGET_FPS: u64 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS);

struct ClippyApp {
    window: Option<Arc<Window>>,
    sprite_sheet: SpriteSheet,
    companion_state: Option<CompanionState>,
    renderer: Option<Renderer>,
    last_update: Instant,
}

impl ClippyApp {
    fn new() -> Self {
        // Load sprite sheet at startup
        let sprite_sheet = SpriteSheet::load_embedded();

        Self {
            window: None,
            sprite_sheet,
            companion_state: None,
            renderer: None,
            last_update: Instant::now(),
        }
    }

    fn update_and_render(&mut self) {
        let window = match &self.window {
            Some(w) => w,
            None => return,
        };

        let companion_state = match &mut self.companion_state {
            Some(cs) => cs,
            None => return,
        };

        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };

        // Get sprite dimensions
        let (sprite_width, sprite_height) = self.sprite_sheet.sprite_dimensions();

        // Get primary monitor for screen bounds
        let monitor = window.primary_monitor().or_else(|| window.available_monitors().next());
        let (screen_width, screen_height) = if let Some(mon) = monitor {
            let size = mon.size();
            (size.width as i32, size.height as i32)
        } else {
            // Fallback to reasonable defaults if no monitor info
            (1920, 1080)
        };

        // Update companion behavior and state
        companion_state.update_behavior();
        companion_state.update_position(screen_width, screen_height, sprite_width as i32);

        // Get current frame for the animation state
        let frame_count = match companion_state.animation_state {
            animation::AnimationState::Idle => self.sprite_sheet.idle_frames.len(),
            animation::AnimationState::Walking { .. } => self.sprite_sheet.walk_left_frames.len(),
        };
        companion_state.update_animation_frame(frame_count);

        // Get the current sprite frame to render
        let sprite = self.sprite_sheet.get_frame(
            &companion_state.animation_state,
            companion_state.current_frame,
        );

        if let Some(sprite) = sprite {
            // Update window position to match companion position
            let (x, y) = companion_state.position;
            let position = PhysicalPosition::new(x, y);
            window.set_outer_position(position);

            // Render the sprite
            let _ = renderer.render(sprite, (sprite_width, sprite_height));
        }

        // Request next redraw
        window.request_redraw();
    }
}

impl ApplicationHandler for ClippyApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            // Get sprite dimensions for window size
            let (sprite_width, sprite_height) = self.sprite_sheet.sprite_dimensions();

            let window_attrs = WindowAttributes::default()
                .with_title("Clippy Companion")
                .with_inner_size(winit::dpi::LogicalSize::new(sprite_width, sprite_height))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false);

            let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

            // Initialize renderer
            let renderer = match Renderer::new(window.clone()) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to create renderer: {}", e);
                    event_loop.exit();
                    return;
                }
            };

            // Get screen dimensions for companion state initialization
            let monitor = window.primary_monitor().or_else(|| window.available_monitors().next());
            let (screen_width, screen_height) = if let Some(mon) = monitor {
                let size = mon.size();
                (size.width as i32, size.height as i32)
            } else {
                (1920, 1080)
            };

            // Initialize companion state
            let companion_state = CompanionState::new(
                screen_width,
                screen_height,
                sprite_width as i32,
            );

            self.window = Some(window);
            self.renderer = Some(renderer);
            self.companion_state = Some(companion_state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                // Handle right-click to show context menu
                if button == MouseButton::Right && state == ElementState::Released {
                    if let Some(window) = &self.window {
                        let position = window.outer_position().unwrap_or_default();
                        let should_quit = platform::show_context_menu(position.x, position.y);
                        if should_quit {
                            event_loop.exit();
                        }
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // Handle Escape key for fallback quit (especially useful on Linux without GTK)
                if event.state == ElementState::Released {
                    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Rate limiting for 60 FPS
                let now = Instant::now();
                let elapsed = now.duration_since(self.last_update);

                if elapsed >= FRAME_DURATION {
                    self.update_and_render();
                    self.last_update = now;
                } else {
                    // Request another redraw soon
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraws to maintain frame rate
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    // Detect Wayland and warn about positioning limitations
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            eprintln!("Warning: Running on Wayland without layer-shell support.");
            eprintln!("Window positioning is controlled by compositor - wandering behavior limited.");
            eprintln!("Run under XWayland for full positioning control.");
        }
    }

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ClippyApp::new();

    let _ = event_loop.run_app(&mut app);
}
