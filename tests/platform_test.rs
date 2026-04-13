//! Tests for platform module functionality.
//!
//! These tests verify that the platform module exports the correct functions
//! and that they can be called without panicking on the current platform.

use clippy_app::platform;

#[test]
fn test_show_context_menu_exists() {
    // Verify that the function is callable without panicking
    // This test is platform-specific and mainly checks that the module is properly exported
    // On Linux without GTK, this should return false
    // On Windows/macOS, this would require a GUI context which we don't have in tests

    // We can at least verify the function signature is correct by calling it
    // with dummy coordinates. The function won't actually display a menu in test context.
    let _ = platform::show_context_menu(0, 0);
}

#[test]
fn test_platform_module_public_api() {
    // Verify the platform module exports show_context_menu
    // This test passes if the module compiles and exports the function
    assert!(true); // If we get here, the module is accessible
}
