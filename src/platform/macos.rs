//! macOS-specific context menu implementation using Cocoa NSMenu.
//!
//! Uses NSMenu and NSPopUpButton from the Cocoa framework to display
//! a native macOS context menu with a "Quit" option.

use cocoa::appkit::{NSMenu, NSMenuItem, NSPopUpButton};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSString, NSRect, NSPoint};
use objc::runtime::Object;
use objc::{msg_send, sel, sel_impl};

const MENU_ID_QUIT: i64 = 1;

/// Shows a native macOS context menu at the specified coordinates.
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
        // Create the menu
        let menu = NSMenu::new(nil);
        if menu == nil {
            eprintln!("Failed to create NSMenu");
            return false;
        }

        // Add the "Quit" menu item
        let quit_title = NSString::from_str("Quit");
        let quit_item = NSMenuItem::new(nil);
        quit_item.setTitle_(quit_title);
        quit_item.setTag_(MENU_ID_QUIT);
        menu.addItem_(quit_item);

        // Create a popup button to host and display the menu
        let frame = NSRect::new(NSPoint::new(x as f64, y as f64), cocoa::foundation::NSSize::new(0.0, 0.0));
        let popup = NSPopUpButton::alloc(nil);
        let popup = NSPopUpButton::initWithFrame_pullsDown_(popup, frame, cocoa::appkit::objc_bool(false));

        if popup == nil {
            eprintln!("Failed to create NSPopUpButton");
            let _: () = msg_send![menu, release];
            return false;
        }

        // Set the menu on the popup button
        popup.setMenu_(menu);

        // Display the menu as a popup at the current location
        // Get the event from NSApp's currentEvent or create a synthetic one
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let current_event: id = msg_send![app, currentEvent];

        // If we have a current event, use its location; otherwise use our coordinates
        let event_point = if current_event != nil {
            let event_location: NSPoint = msg_send![current_event, locationInWindow];
            event_location
        } else {
            NSPoint::new(x as f64, y as f64)
        };

        // Display popup menu
        let _: () = msg_send![popup, performClickAtPoint:event_point];

        // Check which item was selected
        let selected_tag: i64 = msg_send![popup, selectedTag];
        let should_quit = selected_tag == MENU_ID_QUIT;

        // Clean up
        let _: () = msg_send![popup, release];
        let _: () = msg_send![menu, release];

        should_quit
    }
}

// Helper macro to get a class by name
#[allow(non_snake_case)]
unsafe fn class(name: &str) -> *mut Object {
    objc::runtime::Class::get(name)
        .expect("Class not found")
        .as_ptr() as *mut Object
}
