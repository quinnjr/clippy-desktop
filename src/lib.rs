//! Microsoft Clippy on your desktop.
//!
//! A transparent, borderless, always-on-top window shows the original Office
//! Assistant sprites and plays their original animations. Rendering goes
//! through OpenGL so the window has a real alpha channel on Windows, macOS,
//! X11 and Wayland.

pub mod agent;
pub mod animator;
pub mod app;
pub mod companion;
pub mod options;
pub mod platform;
pub mod renderer;
pub mod sprite;

pub use app::run;
