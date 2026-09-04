//! Command-line options.
//!
//! These are only the values the user passed explicitly, so each is an
//! `Option`; the environment, the saved config file, and the built-in defaults
//! fill in the rest (see [`crate::settings`]).

use std::fmt;

pub const MIN_SCALE: u32 = 1;
pub const MAX_SCALE: u32 = 8;
pub const DEFAULT_SCALE: u32 = 2;

pub const USAGE: &str = "\
Usage: clippy-app [OPTIONS]

Puts Microsoft Clippy on your desktop. Ask him things by pointing him at any
OpenAI-compatible server.

Options:
  --scale <N>          Size multiplier for the sprite, 1 to 8 (default: 2)
  --no-always-on-top   Let other windows cover Clippy
  --base-url <URL>     OpenAI-compatible server, e.g. http://localhost:11434/v1
  --model <NAME>       Model to chat with, e.g. llama3.2 or gpt-4o-mini
  --api-key <KEY>      Bearer token, if the server needs one
  -h, --help           Print this help
  -V, --version        Print the version

Settings can also be changed while Clippy is running (press F2) and are saved
to a config file. Command-line flags and the OPENAI_BASE_URL / OPENAI_MODEL /
OPENAI_API_KEY environment variables override the file. Chat stays disabled
until a base URL and a model are given; Clippy still keeps you company without
them.

Controls:
  Drag with the left mouse button   Move Clippy
  Left click                        Make Clippy do something
  Enter                             Open the chat balloon and ask him
  F2                                Open settings
  Right click                       Menu (Windows and macOS)
  Escape                            Close the balloon/settings, or quit
  Home                              Bring Clippy back to the corner
  Q                                 Quit";

/// Chat connection overrides passed on the command line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatArgs {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
}

/// What the user passed on the command line. Everything is optional; unset
/// values fall back to the environment, the config file, then the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CliArgs {
    /// Logical size multiplier applied to the sprite frame.
    pub scale: Option<u32>,
    pub always_on_top: Option<bool>,
    pub chat: ChatArgs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Run(CliArgs),
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
    let mut cli = CliArgs::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--no-always-on-top" => cli.always_on_top = Some(false),
            "--scale" => cli.scale = Some(parse_scale(&take_value(&mut args, "--scale")?)?),
            "--base-url" => cli.chat.base_url = Some(take_value(&mut args, "--base-url")?),
            "--model" => cli.chat.model = Some(take_value(&mut args, "--model")?),
            "--api-key" => cli.chat.api_key = Some(take_value(&mut args, "--api-key")?),
            other => {
                if let Some(value) = other.strip_prefix("--scale=") {
                    cli.scale = Some(parse_scale(value)?);
                } else if let Some(value) = other.strip_prefix("--base-url=") {
                    cli.chat.base_url = Some(value.to_string());
                } else if let Some(value) = other.strip_prefix("--model=") {
                    cli.chat.model = Some(value.to_string());
                } else if let Some(value) = other.strip_prefix("--api-key=") {
                    cli.chat.api_key = Some(value.to_string());
                } else {
                    return Err(UsageError(format!("unknown option {other:?}")));
                }
            }
        }
    }
    Ok(Command::Run(cli))
}

fn take_value<I: Iterator<Item = String>>(args: &mut I, flag: &str) -> Result<String, UsageError> {
    args.next()
        .ok_or_else(|| UsageError(format!("{flag} needs a value")))
}

fn parse_scale(value: &str) -> Result<u32, UsageError> {
    match value.parse::<u32>() {
        Ok(scale) if (MIN_SCALE..=MAX_SCALE).contains(&scale) => Ok(scale),
        _ => Err(UsageError(format!(
            "--scale must be a whole number from {MIN_SCALE} to {MAX_SCALE}, got {value:?}"
        ))),
    }
}
