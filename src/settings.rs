//! Effective settings, resolved from the command line, the environment, a
//! saved config file, and the built-in defaults, in that order of precedence.
//!
//! The file lives in the platform config directory (e.g.
//! `~/.config/clippy-app/config.toml` on Linux). It is written by the settings
//! modal; the command line and the `OPENAI_*` environment variables still win
//! over it, so nothing passed at launch is lost.

use crate::chat::config::{build, clean, ChatConfig, Unconfigured};
use crate::options::{CliArgs, DEFAULT_SCALE, MAX_SCALE, MIN_SCALE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The on-disk config. Every field is optional so a partial file still loads
/// and only overrides what it names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileConfig {
    pub scale: Option<u32>,
    pub always_on_top: Option<bool>,
    #[serde(default)]
    pub chat: ChatFile,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFile {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
}

/// The resolved settings the app actually runs with.
pub struct Settings {
    pub scale: u32,
    pub always_on_top: bool,
    pub chat: Result<ChatConfig, Unconfigured>,
}

/// Resolves settings using the real environment.
pub fn resolve(cli: &CliArgs, file: &FileConfig) -> Settings {
    resolve_with_env(cli, file, |name| std::env::var(name).ok())
}

/// Resolution with the environment injected, for testing. Precedence, highest
/// first: command line, environment, config file, default.
pub fn resolve_with_env(
    cli: &CliArgs,
    file: &FileConfig,
    env: impl Fn(&str) -> Option<String>,
) -> Settings {
    let scale = cli
        .scale
        .or(file.scale)
        .unwrap_or(DEFAULT_SCALE)
        .clamp(MIN_SCALE, MAX_SCALE);
    let always_on_top = cli.always_on_top.or(file.always_on_top).unwrap_or(true);
    let base_url = pick(
        &cli.chat.base_url,
        env("OPENAI_BASE_URL"),
        &file.chat.base_url,
    );
    let model = pick(&cli.chat.model, env("OPENAI_MODEL"), &file.chat.model);
    let api_key = pick(&cli.chat.api_key, env("OPENAI_API_KEY"), &file.chat.api_key);
    Settings {
        scale,
        always_on_top,
        chat: build(base_url, model, api_key),
    }
}

/// First non-empty of command line, environment, file.
fn pick(cli: &Option<String>, env: Option<String>, file: &Option<String>) -> Option<String> {
    clean(cli.clone())
        .or_else(|| clean(env))
        .or_else(|| clean(file.clone()))
}

/// Builds a [`FileConfig`] from the settings modal's fields.
pub fn file_from(
    scale: u32,
    always_on_top: bool,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> FileConfig {
    FileConfig {
        scale: Some(scale),
        always_on_top: Some(always_on_top),
        chat: ChatFile {
            base_url: non_empty(base_url),
            model: non_empty(model),
            api_key: non_empty(api_key),
        },
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Path of the config file, if a config directory exists on this platform.
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("clippy-app").join("config.toml"))
}

/// A short, displayable form of the config path (for the modal).
pub fn config_path_display() -> String {
    config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "(no config directory)".to_string())
}

/// Loads the config file, treating a missing or invalid file as empty.
pub fn load() -> FileConfig {
    let Some(path) = config_path() else {
        return FileConfig::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_else(|err| {
            eprintln!("clippy-app: ignoring invalid {}: {err}", path.display());
            FileConfig::default()
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => FileConfig::default(),
        Err(err) => {
            eprintln!("clippy-app: could not read {}: {err}", path.display());
            FileConfig::default()
        }
    }
}

/// Writes the config file, creating the directory if needed. Returns the path.
pub fn save(file: &FileConfig) -> Result<PathBuf, String> {
    let path = config_path().ok_or_else(|| "no config directory is available".to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let text = toml::to_string_pretty(file).map_err(|err| err.to_string())?;
    std::fs::write(&path, text).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(path)
}
