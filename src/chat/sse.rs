//! Parsing of an OpenAI-style chat completion response.
//!
//! Streaming responses arrive as Server-Sent Events: lines like
//! `data: {json chunk}` ending with `data: [DONE]`. Non-streaming responses
//! (a server that ignores `stream: true`) are a single JSON object. Both are
//! parsed here, kept free of any I/O so they are easy to test.

use serde::Deserialize;

/// The meaning of one SSE `data:` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sse {
    /// A piece of the assistant's reply.
    Delta(String),
    /// The stream is finished.
    Done,
    /// A keep-alive, a blank line, or a chunk with no text; skip it.
    Ignore,
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Delta,
}

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

/// Interprets one line of a text/event-stream body.
pub fn parse_line(line: &str) -> Sse {
    let line = line.trim_end_matches('\r');
    let Some(rest) = line.strip_prefix("data:") else {
        // Comments (`: ...`), `event:` lines and blanks carry no text.
        return Sse::Ignore;
    };
    let payload = rest.trim();
    if payload.is_empty() {
        return Sse::Ignore;
    }
    if payload == "[DONE]" {
        return Sse::Done;
    }
    match serde_json::from_str::<StreamChunk>(payload) {
        Ok(chunk) => {
            let text: String = chunk
                .choices
                .into_iter()
                .filter_map(|choice| choice.delta.content)
                .collect();
            if text.is_empty() {
                Sse::Ignore
            } else {
                Sse::Delta(text)
            }
        }
        // A half-received chunk or an unfamiliar shape is not fatal.
        Err(_) => Sse::Ignore,
    }
}

#[derive(Deserialize)]
struct FullResponse {
    #[serde(default)]
    choices: Vec<FullChoice>,
    #[serde(default)]
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct FullChoice {
    #[serde(default)]
    message: FullMessage,
}

#[derive(Deserialize, Default)]
struct FullMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(default)]
    message: String,
}

/// Extracts the reply from a whole (non-streaming) completion body.
pub fn parse_full(body: &str) -> Result<String, String> {
    let response: FullResponse =
        serde_json::from_str(body).map_err(|err| format!("could not read the reply: {err}"))?;
    if let Some(error) = response.error {
        if !error.message.is_empty() {
            return Err(error.message);
        }
    }
    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| "the server sent an empty reply".to_string())
}

/// Pulls a human-readable message out of an error body, falling back to the
/// raw text.
pub fn error_message(body: &str) -> String {
    if let Ok(response) = serde_json::from_str::<FullResponse>(body) {
        if let Some(error) = response.error {
            if !error.message.is_empty() {
                return error.message;
            }
        }
    }
    body.trim().to_string()
}
