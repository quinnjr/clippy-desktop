//! The on-screen chat balloon and settings modal, drawn with egui in the same
//! window as Clippy.

pub mod balloon;
pub mod settings;

pub use balloon::{BalloonAction, BalloonLayout};
pub use settings::{SettingsAction, SettingsDraft};
