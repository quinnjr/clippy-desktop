//! Platform-specific context menu implementations.
//!
//! Provides a cross-platform abstraction for displaying context menus
//! when the right-click event occurs on the Clippy window.

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// Shows a context menu at the given screen coordinates.
///
/// # Arguments
/// * `x` - Screen X coordinate (in physical pixels)
/// * `y` - Screen Y coordinate (in physical pixels)
///
/// # Returns
/// * `true` if user selected "Quit" from the menu
/// * `false` if menu was cancelled or closed without selection
pub fn show_context_menu(x: i32, y: i32) -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::show_context_menu(x, y)
    }

    #[cfg(target_os = "macos")]
    {
        macos::show_context_menu(x, y)
    }

    #[cfg(target_os = "linux")]
    {
        linux::show_context_menu(x, y)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        eprintln!("Context menu not supported on this platform");
        false
    }
}
