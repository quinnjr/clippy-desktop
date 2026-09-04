//! Talking to an OpenAI-compatible server, and the state of the conversation.

pub mod client;
pub mod config;
pub mod sse;

pub use client::{stream, ChatEvent};
pub use config::{resolve, ChatConfig, Unconfigured};

use serde::Serialize;

/// One message in the conversation. Serialized straight into the request body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.into(),
        }
    }
    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }
}

/// Clippy's persona.
pub const SYSTEM_PROMPT: &str = "You are Clippy, the Microsoft Office Assistant, \
now living on the user's desktop. You are genuinely helpful, warm, and concise. \
You may be a little playful, but never annoying. Prefer short, direct answers; \
use a short list only when it truly helps.";

/// How many recent messages (besides the system prompt) to send, to bound the
/// request size over a long conversation.
const MAX_SENT_MESSAGES: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Nothing in flight.
    Idle,
    /// Request sent, no tokens yet.
    Waiting,
    /// Tokens are arriving.
    Streaming,
    /// The last request failed.
    Error(String),
}

/// The whole conversation and what Clippy is currently doing with it.
pub struct ChatState {
    config: Result<ChatConfig, Unconfigured>,
    /// The transcript, with the system prompt at index 0.
    history: Vec<Message>,
    /// The assistant reply currently being streamed, if any.
    streaming: String,
    status: Status,
}

impl ChatState {
    pub fn new(config: Result<ChatConfig, Unconfigured>) -> Self {
        Self {
            config,
            history: vec![Message::system(SYSTEM_PROMPT)],
            streaming: String::new(),
            status: Status::Idle,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_ok()
    }

    /// The configuration, if chat is set up.
    pub fn config(&self) -> Option<&ChatConfig> {
        self.config.as_ref().ok()
    }

    /// Why chat is unavailable, if it is not configured.
    pub fn unconfigured_message(&self) -> Option<&str> {
        self.config.as_ref().err().map(|u| u.message.as_str())
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    pub fn is_busy(&self) -> bool {
        matches!(self.status, Status::Waiting | Status::Streaming)
    }

    /// The reply text arriving right now (empty unless streaming).
    pub fn streaming(&self) -> &str {
        &self.streaming
    }

    /// The visible transcript: every message except the system prompt.
    pub fn turns(&self) -> impl Iterator<Item = &Message> {
        self.history.iter().skip(1)
    }

    /// Records a new user message and returns the messages to send (the system
    /// prompt plus the most recent turns).
    pub fn begin_user(&mut self, text: &str) -> Vec<Message> {
        self.history.push(Message::user(text));
        self.streaming.clear();
        self.status = Status::Waiting;

        let mut to_send = Vec::new();
        to_send.push(self.history[0].clone());
        let recent = self.history.len().saturating_sub(1).min(MAX_SENT_MESSAGES);
        let start = self.history.len() - recent;
        to_send.extend(self.history[start..].iter().cloned());
        to_send
    }

    pub fn on_delta(&mut self, text: &str) {
        self.status = Status::Streaming;
        self.streaming.push_str(text);
    }

    /// Finalises the streamed reply into the transcript.
    pub fn on_done(&mut self) {
        let reply = std::mem::take(&mut self.streaming);
        if !reply.trim().is_empty() {
            self.history.push(Message::assistant(reply));
        }
        self.status = Status::Idle;
    }

    pub fn on_error(&mut self, message: String) {
        self.streaming.clear();
        self.status = Status::Error(message);
    }
}
