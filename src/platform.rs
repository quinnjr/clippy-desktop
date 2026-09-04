//! Native right-click menu.
//!
//! Windows and macOS get a real context menu through `muda`. On Linux winit
//! owns the window without a GTK counterpart, so there is no native popup;
//! the keyboard (Escape or Q) quits instead.

use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// "Animate!": play a random gesture.
    Animate,
    Quit,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native {
    use super::MenuAction;
    use muda::{ContextMenu, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    const ANIMATE_ID: &str = "animate";
    const QUIT_ID: &str = "quit";

    pub struct ContextMenuHandle {
        menu: Menu,
    }

    impl ContextMenuHandle {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let menu = Menu::new();
            let animate = MenuItem::with_id(ANIMATE_ID, "Animate!", true, None);
            let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
            menu.append_items(&[&animate, &PredefinedMenuItem::separator(), &quit])?;
            Ok(Self { menu })
        }

        /// Pops the menu up at the pointer. Returns `false` when no native
        /// menu could be shown.
        pub fn show(&self, window: &Window) -> bool {
            let Ok(handle) = window.window_handle() else {
                return false;
            };
            match handle.as_raw() {
                #[cfg(target_os = "windows")]
                RawWindowHandle::Win32(handle) => {
                    // SAFETY: the handle comes from a live winit window on this thread.
                    unsafe {
                        self.menu
                            .show_context_menu_for_hwnd(handle.hwnd.get(), None)
                    }
                }
                #[cfg(target_os = "macos")]
                RawWindowHandle::AppKit(handle) => {
                    // SAFETY: the handle is the live NSView of a winit window on the main thread.
                    unsafe {
                        self.menu
                            .show_context_menu_for_nsview(handle.ns_view.as_ptr() as *const _, None)
                    }
                }
                _ => false,
            }
        }
    }

    /// Routes menu selections to `deliver`, which runs on muda's thread.
    pub fn install_menu_event_handler<F>(deliver: F)
    where
        F: Fn(MenuAction) + Send + Sync + 'static,
    {
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id.0.as_str() {
                ANIMATE_ID => MenuAction::Animate,
                QUIT_ID => MenuAction::Quit,
                _ => return,
            };
            deliver(action);
        }));
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod native {
    use super::MenuAction;
    use winit::window::Window;

    pub struct ContextMenuHandle;

    impl ContextMenuHandle {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            Ok(Self)
        }

        /// No native popup is available here.
        pub fn show(&self, _window: &Window) -> bool {
            false
        }
    }

    pub fn install_menu_event_handler<F>(_deliver: F)
    where
        F: Fn(MenuAction) + Send + Sync + 'static,
    {
    }
}

pub use native::{install_menu_event_handler, ContextMenuHandle};

/// Whether this build has a native right-click menu.
pub const HAS_NATIVE_MENU: bool = cfg!(any(target_os = "windows", target_os = "macos"));

/// Convenience wrapper so callers need not name the handle type.
pub fn show_context_menu(menu: &ContextMenuHandle, window: &Window) -> bool {
    menu.show(window)
}
