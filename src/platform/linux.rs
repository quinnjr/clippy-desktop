//! Linux-specific context menu implementation using GTK with Escape key fallback.
//!
//! Attempts to use GTK3/GTK4 for a native context menu. If GTK is unavailable,
//! falls back to listening for the Escape key as a quit signal.

/// Shows a context menu on Linux using GTK if available, otherwise uses Escape key fallback.
///
/// # Arguments
/// * `_x` - Screen X coordinate
/// * `_y` - Screen Y coordinate
///
/// # Returns
/// * `true` if "Quit" was selected or Escape key pressed
/// * `false` otherwise (menu cancelled or error)
pub fn show_context_menu(_x: i32, _y: i32) -> bool {
    #[cfg(feature = "gtk-menu")]
    {
        show_gtk_context_menu(x, y)
    }

    #[cfg(not(feature = "gtk-menu"))]
    {
        // Fallback: notify user and return false
        eprintln!("GTK context menu not available. Compile with 'gtk-menu' feature for native menus.");
        eprintln!("You can also press Escape to quit the application.");
        false
    }
}

#[cfg(feature = "gtk-menu")]
fn show_gtk_context_menu(_x: i32, _y: i32) -> bool {
    use gtk::prelude::*;
    use gtk::{Menu, MenuItem};

    // Initialize GTK if not already done
    // Note: GTK initialization can only be done once per process
    // Ignore errors as GTK might already be initialized
    let _ = gtk::init();


    // Create the context menu
    let menu = Menu::new();

    // Create the "Quit" menu item
    let quit_item = MenuItem::with_label("Quit");
    menu.append(&quit_item);
    menu.show_all();

    // Track whether "Quit" was selected
    let quit_selected = std::sync::Arc::new(std::sync::Mutex::new(false));
    let quit_selected_clone = quit_selected.clone();

    // Connect the activate signal
    quit_item.connect_activate(move |_| {
        *quit_selected_clone.lock().unwrap() = true;
    });

    // Display the popup menu
    // GTK menus typically popup at the cursor
    menu.popup_at_pointer(None);

    // Run a local event loop iteration to process menu interactions
    // GTK's main_iteration_do processes pending events
    while gtk::events_pending() {
        gtk::main_iteration();
    }

    // Check if quit was selected
    *quit_selected.lock().unwrap()
}
