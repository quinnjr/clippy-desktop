//! Where Clippy sends chat requests.
//!
//! The base URL and model can come from the command line or the environment;
//! command-line flags win. Chat stays disabled until both are present, and the
//! reason (with instructions) is carried in [`Unconfigured`] so it can be shown
//! in the balloon.

use crate::options::ChatArgs;

/// A resolved, ready-to-use chat target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatConfig {
    /// Base URL including the version path, e.g. `http://localhost:11434/v1`,
    /// with no trailing slash.
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
}

impl ChatConfig {
    /// Full URL of the chat completions endpoint.
    pub fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

/// Chat could not be configured; `message` explains how to fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unconfigured {
    pub message: String,
}

const HELP: &str = "Point Clippy at any OpenAI-compatible server:\n  \
    clippy-app --base-url <url> --model <name> [--api-key <key>]\n\
    or set OPENAI_BASE_URL and OPENAI_MODEL (and OPENAI_API_KEY if it needs one).";

/// Resolves the chat target from command-line flags and the environment.
pub fn resolve(args: &ChatArgs) -> Result<ChatConfig, Unconfigured> {
    resolve_from(args, |name| std::env::var(name).ok())
}

/// The resolution logic, with the environment injected for testing.
pub fn resolve_from(
    args: &ChatArgs,
    env: impl Fn(&str) -> Option<String>,
) -> Result<ChatConfig, Unconfigured> {
    let base_url = first_non_empty(args.base_url.clone(), || env("OPENAI_BASE_URL"));
    let model = first_non_empty(args.model.clone(), || env("OPENAI_MODEL"));
    let api_key = first_non_empty(args.api_key.clone(), || env("OPENAI_API_KEY"));

    match (base_url, model) {
        (Some(base_url), Some(model)) => Ok(ChatConfig {
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            api_key,
        }),
        (base_url, model) => {
            let mut missing = Vec::new();
            if base_url.is_none() {
                missing.push("a base URL (--base-url or OPENAI_BASE_URL)");
            }
            if model.is_none() {
                missing.push("a model (--model or OPENAI_MODEL)");
            }
            Ok(Err(Unconfigured {
                message: format!("Chat needs {}.\n{HELP}", missing.join(" and ")),
            })?)
        }
    }
}

fn first_non_empty(
    primary: Option<String>,
    fallback: impl FnOnce() -> Option<String>,
) -> Option<String> {
    primary
        .or_else(fallback)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
