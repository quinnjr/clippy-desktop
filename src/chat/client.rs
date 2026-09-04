//! The streaming HTTP call to an OpenAI-compatible chat completions endpoint.
//!
//! [`stream`] is blocking and free of any UI types, so it runs on a worker
//! thread and is tested directly against a local mock server. It reports
//! progress through a `sink` callback.

use super::config::ChatConfig;
use super::sse::{self, Sse};
use super::Message;
use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};

/// Something that happened while talking to the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEvent {
    /// A piece of the assistant's reply.
    Delta(String),
    /// The reply is complete.
    Done,
    /// The request failed; the string is a human-readable reason.
    Error(String),
}

#[derive(Serialize)]
struct CompletionRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
    temperature: f32,
}

/// How much of an error body to keep, so a misbehaving server cannot flood us.
const MAX_ERROR_BYTES: u64 = 8 * 1024;

/// Sends `messages` and drives `sink` with the reply. Returns after emitting a
/// terminal [`ChatEvent::Done`] or [`ChatEvent::Error`], or early (silently) if
/// `cancel` is set. Never emits anything after a terminal event.
pub fn stream(
    config: &ChatConfig,
    messages: &[Message],
    cancel: &AtomicBool,
    mut sink: impl FnMut(ChatEvent),
) {
    let url = config.completions_url();
    let request = CompletionRequest {
        model: &config.model,
        messages,
        stream: true,
        temperature: 0.7,
    };
    let body = match serde_json::to_string(&request) {
        Ok(body) => body,
        Err(err) => {
            return sink(ChatEvent::Error(format!(
                "could not build the request: {err}"
            )))
        }
    };

    // Read error responses ourselves instead of letting ureq turn a 4xx/5xx
    // into an opaque error, so the server's own message reaches the balloon.
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build(),
    );
    let mut builder = agent
        .post(&url)
        .content_type("application/json")
        .header("accept", "text/event-stream");
    if let Some(key) = &config.api_key {
        builder = builder.header("authorization", format!("Bearer {key}"));
    }

    let response = match builder.send(body) {
        Ok(response) => response,
        Err(err) => return sink(ChatEvent::Error(send_error(&url, &err))),
    };

    if cancel.load(Ordering::Relaxed) {
        return;
    }

    let status = response.status();
    let is_stream = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("event-stream"))
        .unwrap_or(false);

    if !status.is_success() {
        let mut text = String::new();
        let _ = response
            .into_body()
            .into_reader()
            .take(MAX_ERROR_BYTES)
            .read_to_string(&mut text);
        let reason = sse::error_message(&text);
        let reason = if reason.is_empty() {
            status
                .canonical_reason()
                .unwrap_or("request failed")
                .to_string()
        } else {
            reason
        };
        return sink(ChatEvent::Error(format!(
            "server error ({}): {reason}",
            status.as_u16()
        )));
    }

    if is_stream {
        stream_sse(response.into_body().into_reader(), cancel, &mut sink);
    } else {
        // A server that ignored `stream: true` and sent one JSON object.
        let mut text = String::new();
        if let Err(err) = response.into_body().into_reader().read_to_string(&mut text) {
            return sink(ChatEvent::Error(format!("could not read the reply: {err}")));
        }
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        match sse::parse_full(&text) {
            Ok(content) => {
                if !content.is_empty() {
                    sink(ChatEvent::Delta(content));
                }
                sink(ChatEvent::Done);
            }
            Err(reason) => sink(ChatEvent::Error(reason)),
        }
    }
}

fn stream_sse(reader: impl Read, cancel: &AtomicBool, sink: &mut impl FnMut(ChatEvent)) {
    let reader = BufReader::new(reader);
    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let line = match line {
            Ok(line) => line,
            Err(err) => return sink(ChatEvent::Error(format!("the connection dropped: {err}"))),
        };
        match sse::parse_line(&line) {
            Sse::Delta(text) => sink(ChatEvent::Delta(text)),
            Sse::Done => return sink(ChatEvent::Done),
            Sse::Ignore => {}
        }
    }
    // The body ended without an explicit [DONE]; treat that as a clean finish.
    sink(ChatEvent::Done);
}

fn send_error(url: &str, err: &ureq::Error) -> String {
    format!("could not reach {url}: {err}")
}
