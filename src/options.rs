//! Command-line options.

use std::fmt;

pub const MIN_SCALE: u32 = 1;
pub const MAX_SCALE: u32 = 8;
pub const DEFAULT_SCALE: u32 = 2;

pub const USAGE: &str = "\
Usage: clippy-app [OPTIONS]

Puts Microsoft Clippy on your desktop.

Options:
  --scale <N>          Size multiplier for the sprite, 1 to 8 (default: 2)
  --no-always-on-top   Let other windows cover Clippy
  -h, --help           Print this help
  -V, --version        Print the version

Controls:
  Drag with the left mouse button   Move Clippy
  Left click, Space or Enter        Make Clippy do something
  Right click                       Menu (Windows and macOS)
  Escape or Q                       Quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Logical size multiplier applied to the sprite frame.
    pub scale: u32,
    pub always_on_top: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            always_on_top: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(Options),
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for UsageError {}

/// Parses the arguments that follow the program name.
pub fn parse<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator<Item = String>,
{
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--no-always-on-top" => options.always_on_top = false,
            "--scale" => {
                let value = args
                    .next()
                    .ok_or_else(|| UsageError("--scale needs a value".to_string()))?;
                options.scale = parse_scale(&value)?;
            }
            other => {
                if let Some(value) = other.strip_prefix("--scale=") {
                    options.scale = parse_scale(value)?;
                } else {
                    return Err(UsageError(format!("unknown option {other:?}")));
                }
            }
        }
    }
    Ok(Command::Run(options))
}

fn parse_scale(value: &str) -> Result<u32, UsageError> {
    match value.parse::<u32>() {
        Ok(scale) if (MIN_SCALE..=MAX_SCALE).contains(&scale) => Ok(scale),
        _ => Err(UsageError(format!(
            "--scale must be a whole number from {MIN_SCALE} to {MAX_SCALE}, got {value:?}"
        ))),
    }
}
