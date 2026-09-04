//! The streaming HTTP call to an OpenAI-compatible chat completions endpoint.
//!
//! [`stream`] is blocking and free of any UI types, so it runs on a worker
//! thread and is tested directly against a local mock server. It reports
//! progress through a `sink` callback.

use super::config::ChatConfig;
use super::sse::{self, Sse};
use super::Message;
use serde::Serialize;
use std::io::{self, BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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

    // Bound the connect and the wait for the response to *start* so a server
    // that accepts the socket but never answers cannot pin Clippy in the
    // "thinking" state forever; the stream body itself is left untimed, as it
    // may legitimately take a while. Read error responses ourselves so the
    // server's own message reaches the balloon.
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
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

    // Whether the reply streams (SSE) or arrives as one JSON object is decided
    // from the body itself, not just the Content-Type, so a mislabelled server
    // still works.
    stream_reply(response.into_body().into_reader(), cancel, &mut sink);
}

/// Reads the reply, telling an SSE stream from a single JSON object by looking
/// at the first line of the body.
fn stream_reply(reader: impl Read, cancel: &AtomicBool, sink: &mut impl FnMut(ChatEvent)) {
    let mut reader = BufReader::new(reader);
    let first = loop {
        match read_line_lossy(&mut reader) {
            Ok(None) => return sink(ChatEvent::Done),
            Ok(Some(line)) if line.trim().is_empty() => {}
            Ok(Some(line)) => break line,
            Err(err) => return sink(ChatEvent::Error(format!("could not read the reply: {err}"))),
        }
    };

    let head = first.trim_start();
    let looks_like_sse =
        head.starts_with("data:") || head.starts_with("event:") || head.starts_with(':');

    if looks_like_sse {
        if !feed(&first, sink) {
            return;
        }
        loop {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            match read_line_lossy(&mut reader) {
                Ok(None) => break,
                Ok(Some(line)) => {
                    if !feed(&line, sink) {
                        return;
                    }
                }
                Err(err) => {
                    return sink(ChatEvent::Error(format!("the connection dropped: {err}")))
                }
            }
        }
        // The body ended without an explicit [DONE]; treat that as a clean finish.
        sink(ChatEvent::Done);
    } else {
        // One JSON object (a server that ignored `stream: true`).
        let mut rest = Vec::new();
        let _ = reader.read_to_end(&mut rest);
        let body = format!("{first}{}", String::from_utf8_lossy(&rest));
        match sse::parse_full(&body) {
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

/// Handles one SSE line, returning `false` once the stream is done.
fn feed(line: &str, sink: &mut impl FnMut(ChatEvent)) -> bool {
    match sse::parse_line(line) {
        Sse::Delta(text) => {
            sink(ChatEvent::Delta(text));
            true
        }
        Sse::Done => {
            sink(ChatEvent::Done);
            false
        }
        Sse::Ignore => true,
    }
}

/// Reads one line, replacing invalid UTF-8 rather than failing, with the
/// trailing newline stripped. `Ok(None)` marks the end of the stream.
fn read_line_lossy(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    if reader.read_until(b'\n', &mut buf)? == 0 {
        return Ok(None);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
    }
    Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
}

fn send_error(url: &str, err: &ureq::Error) -> String {
    format!("could not reach {url}: {err}")
}
