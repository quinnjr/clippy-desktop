//! Tests for the chat subsystem: the SSE parser, configuration resolution,
//! conversation state, and the streaming client against a local mock server.

use clippy_app::chat::client::{stream, ChatEvent};
use clippy_app::chat::config::{resolve_from, ChatConfig};
use clippy_app::chat::sse::{self, Sse};
use clippy_app::chat::{ChatState, Message, Status};
use clippy_app::options::ChatArgs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

// ---------- SSE parsing ----------

#[test]
fn parse_line_reads_deltas_done_and_ignores_noise() {
    assert_eq!(
        sse::parse_line(r#"data: {"choices":[{"delta":{"content":"Hi"}}]}"#),
        Sse::Delta("Hi".to_string())
    );
    // The trailing carriage return of a real SSE line is tolerated.
    assert_eq!(
        sse::parse_line("data: {\"choices\":[{\"delta\":{\"content\":\" there\"}}]}\r"),
        Sse::Delta(" there".to_string())
    );
    assert_eq!(sse::parse_line("data: [DONE]"), Sse::Done);
    assert_eq!(sse::parse_line(""), Sse::Ignore);
    assert_eq!(sse::parse_line(": keep-alive"), Sse::Ignore);
    assert_eq!(sse::parse_line("event: message"), Sse::Ignore);
    // A role-only opening chunk carries no text.
    assert_eq!(
        sse::parse_line(r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#),
        Sse::Ignore
    );
    // A half-received chunk must not crash the stream.
    assert_eq!(sse::parse_line("data: {not json"), Sse::Ignore);
}

#[test]
fn parse_full_reads_a_non_streaming_reply_and_errors() {
    let body = r#"{"choices":[{"message":{"role":"assistant","content":"Full reply."}}]}"#;
    assert_eq!(sse::parse_full(body).unwrap(), "Full reply.");

    let error = r#"{"error":{"message":"model not found"}}"#;
    assert_eq!(sse::parse_full(error).unwrap_err(), "model not found");

    assert!(sse::parse_full("{}").is_err());
    assert_eq!(sse::error_message(error), "model not found");
}

// ---------- configuration ----------

fn args(base: Option<&str>, model: Option<&str>, key: Option<&str>) -> ChatArgs {
    ChatArgs {
        base_url: base.map(String::from),
        model: model.map(String::from),
        api_key: key.map(String::from),
    }
}

#[test]
fn config_prefers_flags_then_environment() {
    let env = |name: &str| match name {
        "OPENAI_BASE_URL" => Some("http://env-host/v1".to_string()),
        "OPENAI_MODEL" => Some("env-model".to_string()),
        "OPENAI_API_KEY" => Some("env-key".to_string()),
        _ => None,
    };

    // Flags win outright.
    let config = resolve_from(
        &args(Some("http://flag/v1/"), Some("flag-model"), None),
        env,
    )
    .unwrap();
    assert_eq!(config.base_url, "http://flag/v1"); // trailing slash trimmed
    assert_eq!(config.model, "flag-model");
    assert_eq!(config.api_key.as_deref(), Some("env-key")); // key falls back to env
    assert_eq!(config.completions_url(), "http://flag/v1/chat/completions");

    // Everything from the environment.
    let config = resolve_from(&args(None, None, None), env).unwrap();
    assert_eq!(config.base_url, "http://env-host/v1");
    assert_eq!(config.model, "env-model");
}

#[test]
fn config_is_unconfigured_when_a_piece_is_missing() {
    let empty = |_: &str| None;
    let err = resolve_from(&args(Some("http://h/v1"), None, None), empty).unwrap_err();
    assert!(err.message.contains("model"), "{}", err.message);
    assert!(err.message.contains("--base-url") || err.message.contains("OPENAI"));

    let err = resolve_from(&args(None, None, None), empty).unwrap_err();
    assert!(err.message.contains("base URL") && err.message.contains("model"));

    // Blank values count as missing.
    let err = resolve_from(&args(Some("   "), Some("m"), None), empty).unwrap_err();
    assert!(err.message.contains("base URL"));
}

// ---------- conversation state ----------

#[test]
fn chat_state_tracks_a_turn_from_prompt_to_reply() {
    let config = ChatConfig {
        base_url: "http://h/v1".into(),
        model: "m".into(),
        api_key: None,
    };
    let mut chat = ChatState::new(Ok(config));
    assert!(chat.is_configured());
    assert!(chat.unconfigured_message().is_none());

    let sent = chat.begin_user("hello");
    assert_eq!(sent.first().unwrap().role, "system");
    assert_eq!(sent.last().unwrap(), &Message::user("hello"));
    assert_eq!(chat.status(), &Status::Waiting);
    assert!(chat.is_busy());

    chat.on_delta("Hi ");
    chat.on_delta("there");
    assert_eq!(chat.status(), &Status::Streaming);
    assert_eq!(chat.streaming(), "Hi there");

    chat.on_done();
    assert_eq!(chat.status(), &Status::Idle);
    assert_eq!(chat.streaming(), "");
    let turns: Vec<_> = chat.turns().collect();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0], &Message::user("hello"));
    assert_eq!(turns[1], &Message::assistant("Hi there"));
}

fn roles(messages: &[Message]) -> Vec<&str> {
    messages.iter().map(|m| m.role.as_str()).collect()
}

fn configured() -> ChatState {
    ChatState::new(Ok(ChatConfig {
        base_url: "http://h/v1".into(),
        model: "m".into(),
        api_key: None,
    }))
}

#[test]
fn a_failed_turn_does_not_send_two_user_messages() {
    let mut chat = configured();
    chat.begin_user("q1");
    chat.on_delta("a1");
    chat.on_done();

    // q2 fails, so the user retries with q3 while q2 is still in history.
    chat.begin_user("q2");
    chat.on_error("boom".to_string());
    let sent = chat.begin_user("q3");

    assert_eq!(roles(&sent), vec!["system", "user", "assistant", "user"]);
    let last = &sent.last().unwrap().content;
    assert!(last.contains("q2") && last.contains("q3"), "{last}");
    // The transcript still shows every turn the user typed.
    assert_eq!(chat.turns().count(), 4);
}

#[test]
fn a_long_conversation_still_begins_with_a_user_turn() {
    let mut chat = configured();
    for i in 0..30 {
        chat.begin_user(&format!("q{i}"));
        chat.on_delta(&format!("a{i}"));
        chat.on_done();
    }
    let sent = chat.begin_user("final");
    assert_eq!(sent[0].role, "system");
    assert_eq!(
        sent[1].role, "user",
        "the window must not start on an assistant turn"
    );
    for pair in sent.windows(2) {
        assert_ne!(
            pair[0].role, pair[1].role,
            "no two consecutive turns share a role"
        );
    }
}

#[test]
fn chat_state_reports_errors_and_missing_config() {
    let mut chat = ChatState::new(Ok(ChatConfig {
        base_url: "http://h/v1".into(),
        model: "m".into(),
        api_key: None,
    }));
    chat.begin_user("hi");
    chat.on_error("boom".to_string());
    assert_eq!(chat.status(), &Status::Error("boom".to_string()));
    assert_eq!(chat.streaming(), "");
    // The failed reply is not added to the transcript.
    assert_eq!(chat.turns().count(), 1);

    let unconfigured = resolve_from(&args(None, None, None), |_| None);
    let chat = ChatState::new(unconfigured);
    assert!(!chat.is_configured());
    assert!(chat.config().is_none());
    assert!(chat.unconfigured_message().unwrap().contains("base URL"));
}

// ---------- streaming client against a mock server ----------

/// Serves one HTTP request with the given raw response, returning the base URL
/// and a handle that yields the full request text the client sent.
fn serve(response: Vec<u8>) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut buf = Vec::new();
        let mut tmp = [0u8; 2048];
        let header_end = loop {
            match stream.read(&mut tmp) {
                Ok(0) => break None,
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(pos) = find(&buf, b"\r\n\r\n") {
                        break Some(pos);
                    }
                }
                Err(_) => break None,
            }
        };
        if let Some(pos) = header_end {
            let headers = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
            if headers.contains("expect: 100-continue") {
                stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").ok();
            }
            let content_length = content_length(&headers);
            let mut body = buf[pos + 4..].to_vec();
            while body.len() < content_length {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => body.extend_from_slice(&tmp[..n]),
                }
            }
            buf.truncate(pos + 4);
            buf.extend_from_slice(&body);
        }
        stream.write_all(&response).ok();
        stream.flush().ok();
        String::from_utf8_lossy(&buf).to_string()
    });
    (format!("http://127.0.0.1:{port}/v1"), handle)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn content_length(lower_headers: &str) -> usize {
    lower_headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

fn config(base_url: String) -> ChatConfig {
    ChatConfig {
        base_url,
        model: "test-model".to_string(),
        api_key: Some("secret".to_string()),
    }
}

fn collect(base_url: String, cancel: &AtomicBool) -> Vec<ChatEvent> {
    let cfg = config(base_url);
    let messages = vec![Message::system("s"), Message::user("hello")];
    let mut events = Vec::new();
    stream(&cfg, &messages, cancel, |event| events.push(event));
    events
}

#[test]
fn client_streams_sse_deltas_then_done() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
data: {\"choices\":[{\"delta\":{\"content\":\"lo!\"}}]}\n\n\
data: [DONE]\n\n"
            .to_vec();
    let (base_url, server) = serve(response);
    let events = collect(base_url, &AtomicBool::new(false));
    assert_eq!(
        events,
        vec![
            ChatEvent::Delta("Hel".to_string()),
            ChatEvent::Delta("lo!".to_string()),
            ChatEvent::Done,
        ]
    );

    // The client sent a well-formed streaming request with our auth and model.
    let request = server.join().unwrap();
    assert!(
        request.starts_with("POST /v1/chat/completions "),
        "{request}"
    );
    let lower = request.to_ascii_lowercase();
    assert!(lower.contains("authorization: bearer secret"));
    assert!(lower.contains("content-type: application/json"));
    assert!(request.contains("\"model\":\"test-model\""));
    assert!(request.contains("\"stream\":true"));
    assert!(request.contains("\"content\":\"hello\""));
}

#[test]
fn client_finishes_even_without_an_explicit_done() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"delta\":{\"content\":\"only\"}}]}\n\n"
            .to_vec();
    let (base_url, server) = serve(response);
    let events = collect(base_url, &AtomicBool::new(false));
    assert_eq!(
        events,
        vec![ChatEvent::Delta("only".to_string()), ChatEvent::Done]
    );
    server.join().unwrap();
}

#[test]
fn client_falls_back_to_a_non_streaming_reply() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"All at once.\"}}]}"
            .to_vec();
    let (base_url, server) = serve(response);
    let events = collect(base_url, &AtomicBool::new(false));
    assert_eq!(
        events,
        vec![
            ChatEvent::Delta("All at once.".to_string()),
            ChatEvent::Done,
        ]
    );
    server.join().unwrap();
}

#[test]
fn client_reports_a_server_error_with_its_message() {
    let response =
        b"HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
{\"error\":{\"message\":\"invalid api key\"}}"
            .to_vec();
    let (base_url, server) = serve(response);
    let events = collect(base_url, &AtomicBool::new(false));
    assert_eq!(events.len(), 1);
    match &events[0] {
        ChatEvent::Error(message) => {
            assert!(message.contains("401"), "{message}");
            assert!(message.contains("invalid api key"), "{message}");
        }
        other => panic!("expected an error, got {other:?}"),
    }
    server.join().unwrap();
}

#[test]
fn client_reports_a_friendly_error_when_the_server_is_unreachable() {
    // Nothing is listening on this port.
    let cfg = config("http://127.0.0.1:1/v1".to_string());
    let messages = vec![Message::user("hi")];
    let mut events = Vec::new();
    stream(&cfg, &messages, &AtomicBool::new(false), |event| {
        events.push(event)
    });
    assert_eq!(events.len(), 1);
    match &events[0] {
        ChatEvent::Error(message) => assert!(message.contains("could not reach"), "{message}"),
        other => panic!("expected an error, got {other:?}"),
    }
}

#[test]
fn a_cancelled_request_emits_nothing() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"delta\":{\"content\":\"ignored\"}}]}\n\n\
data: [DONE]\n\n"
            .to_vec();
    let (base_url, server) = serve(response);
    let cancel = AtomicBool::new(true);
    cancel.store(true, Ordering::Relaxed);
    let events = collect(base_url, &cancel);
    assert!(events.is_empty(), "cancelled request produced {events:?}");
    server.join().unwrap();
}
