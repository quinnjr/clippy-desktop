//! The on-screen chat balloon, drawn with egui in the same window as Clippy.

pub mod balloon;

pub use balloon::{show, BalloonAction, BalloonLayout};
