//! Windows-specific context menu implementation using Win32 API.
//!
//! Uses `CreatePopupMenu()` and `TrackPopupMenu()` from the Win32 API
//! to display a native Windows context menu with a "Quit" option.

use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, TrackPopupMenu, DestroyMenu, AppendMenuW,
    TPM_LEFTALIGN, TPM_TOPALIGN, TPM_RETURNCMD,
};
use windows_sys::Win32::Foundation::GetLastError;

const MENU_ID_QUIT: u32 = 1;

/// Shows a native Windows context menu at the specified coordinates.
///
/// # Arguments
/// * `x` - Screen X coordinate
/// * `y` - Screen Y coordinate
///
/// # Returns
/// * `true` if "Quit" was selected
/// * `false` otherwise (menu cancelled or error)
pub fn show_context_menu(x: i32, y: i32) -> bool {
    unsafe {
        // Create a popup menu
        let menu = CreatePopupMenu();
        if menu == 0 {
            eprintln!("Failed to create popup menu");
            return false;
        }

        // Add "Quit" menu item
        let quit_str = wide_string("Quit");
        let result = AppendMenuW(menu, 0, MENU_ID_QUIT, quit_str.as_ptr());
        if result == 0 {
            eprintln!("Failed to append menu item");
            let _ = DestroyMenu(menu);
            return false;
        }

        // Get the foreground window to use as owner
        use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let hwnd = GetForegroundWindow();

        // Track (display) the popup menu and get the selection
        let selected = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            x as i32,
            y as i32,
            0,
            hwnd,
            std::ptr::null(),
        );

        // Clean up
        let _ = DestroyMenu(menu);

        // Check if "Quit" was selected
        selected == MENU_ID_QUIT as i32
    }
}

/// Converts a Rust string to a null-terminated UTF-16 wide string.
fn wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
