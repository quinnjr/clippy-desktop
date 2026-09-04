//! Command-line options.

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

Chat can also be configured from the environment:
  OPENAI_BASE_URL, OPENAI_MODEL, OPENAI_API_KEY
Command-line flags win over the environment. Chat stays disabled until a
base URL and a model are given; Clippy still keeps you company without them.

Controls:
  Drag with the left mouse button   Move Clippy
  Left click                        Make Clippy do something
  Enter                             Open the chat balloon and ask him
  Right click                       Menu (Windows and macOS)
  Escape                            Close the balloon, or quit
  Q                                 Quit";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChatArgs {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Logical size multiplier applied to the sprite frame.
    pub scale: u32,
    pub always_on_top: bool,
    pub chat: ChatArgs,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            always_on_top: true,
            chat: ChatArgs::default(),
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
            "--scale" => options.scale = parse_scale(&take_value(&mut args, "--scale")?)?,
            "--base-url" => options.chat.base_url = Some(take_value(&mut args, "--base-url")?),
            "--model" => options.chat.model = Some(take_value(&mut args, "--model")?),
            "--api-key" => options.chat.api_key = Some(take_value(&mut args, "--api-key")?),
            other => {
                if let Some(value) = other.strip_prefix("--scale=") {
                    options.scale = parse_scale(value)?;
                } else if let Some(value) = other.strip_prefix("--base-url=") {
                    options.chat.base_url = Some(value.to_string());
                } else if let Some(value) = other.strip_prefix("--model=") {
                    options.chat.model = Some(value.to_string());
                } else if let Some(value) = other.strip_prefix("--api-key=") {
                    options.chat.api_key = Some(value.to_string());
                } else {
                    return Err(UsageError(format!("unknown option {other:?}")));
                }
            }
        }
    }
    Ok(Command::Run(options))
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
