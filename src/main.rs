pub mod animation;
pub mod sprite;
pub mod companion;
pub mod renderer;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowAttributes};
use std::sync::Arc;

struct ClippyApp {
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for ClippyApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attrs = WindowAttributes::default()
                .with_title("Clippy Companion")
                .with_inner_size(winit::dpi::LogicalSize::new(128, 128))
                .with_decorations(false)
                .with_transparent(true);

            let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
            self.window = Some(window);
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
            WindowEvent::RedrawRequested => {
                // TODO: Render sprite
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ClippyApp { window: None };

    let _ = event_loop.run_app(&mut app);
}
