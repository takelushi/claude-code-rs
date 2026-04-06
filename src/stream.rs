use std::pin::Pin;

use crate::types::{ClaudeResponse, strip_ansi};
use async_stream::stream;
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio_stream::Stream;

/// Event emitted from a stream-json response.
///
/// Events come from two sources:
///
/// - **Delta variants** ([`Text`](Self::Text), [`Thinking`](Self::Thinking), etc.) — real-time
///   token-level chunks from `stream_event`. Requires
///   [`crate::ClaudeConfigBuilder::include_partial_messages`] to be enabled.
/// - **Assistant variants** ([`AssistantText`](Self::AssistantText),
///   [`AssistantThinking`](Self::AssistantThinking)) — complete messages from `assistant` events.
///   Always sent regardless of `include_partial_messages`.
///
/// When `include_partial_messages` is enabled, both delta and assistant variants are emitted.
/// Use delta variants for real-time display and assistant variants for the final complete text.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamEvent {
    /// Session initialization info.
    SystemInit {
        /// Session ID.
        session_id: String,
        /// Model name.
        model: String,
    },
    /// Thinking delta chunk from real-time streaming (`stream_event` / `thinking_delta`).
    ///
    /// Only emitted when [`crate::ClaudeConfigBuilder::include_partial_messages`] is enabled.
    Thinking(String),
    /// Text delta chunk from real-time streaming (`stream_event` / `text_delta`).
    ///
    /// Only emitted when [`crate::ClaudeConfigBuilder::include_partial_messages`] is enabled.
    Text(String),
    /// Complete thinking text from `assistant` event. Always emitted.
    AssistantThinking(String),
    /// Complete text from `assistant` event. Always emitted.
    AssistantText(String),
    /// Tool invocation by the model.
    ToolUse {
        /// Tool use ID.
        id: String,
        /// Tool name.
        name: String,
        /// Tool input as JSON value.
        input: serde_json::Value,
    },
    /// Tool execution result.
    ToolResult {
        /// ID of the tool use this result belongs to.
        tool_use_id: String,
        /// Result content.
        content: String,
    },
    /// Rate limit information.
    RateLimit {
        /// Timestamp when the rate limit resets.
        resets_at: u64,
    },
    /// Partial tool input JSON chunk (from `input_json_delta`).
    InputJsonDelta(String),
    /// Thinking signature chunk (from `signature_delta`).
    SignatureDelta(String),
    /// Citations chunk (from `citations_delta`).
    CitationsDelta(serde_json::Value),
    /// Start of a message (from `message_start`).
    MessageStart {
        /// Model name.
        model: String,
        /// Message ID.
        id: String,
    },
    /// Start of a content block (from `content_block_start`).
    ContentBlockStart {
        /// Block index.
        index: u64,
        /// Block type (`"text"`, `"thinking"`, `"tool_use"`, etc.).
        block_type: String,
    },
    /// End of a content block (from `content_block_stop`).
    ContentBlockStop {
        /// Block index.
        index: u64,
    },
    /// Message-level delta with stop reason (from `message_delta`).
    MessageDelta {
        /// Why the message stopped.
        stop_reason: Option<String>,
    },
    /// Message complete (from `message_stop`).
    MessageStop,
    /// Keepalive ping (from `ping`).
    Ping,
    /// API error event (from `error`).
    Error {
        /// Error type.
        error_type: String,
        /// Error message.
        message: String,
    },
    /// Final result (same structure as non-streaming response).
    Result(ClaudeResponse),
    /// Unrecognized event (raw JSON preserved so nothing is lost).
    Unknown(serde_json::Value),
}

/// Parses a single NDJSON line into zero or more [`StreamEvent`]s.
///
/// Returns an empty `Vec` if the line cannot be parsed (ANSI-only, empty, unknown type).
pub(crate) fn parse_event(line: &str) -> Vec<StreamEvent> {
    let stripped = strip_ansi(line);
    let json: Value = match serde_json::from_str(stripped) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    match json.get("type").and_then(|t| t.as_str()) {
        Some("system") => parse_system(&json),
        Some("assistant") => parse_assistant(&json),
        Some("user") => parse_user(&json),
        Some("rate_limit_event") => parse_rate_limit(&json),
        Some("result") => parse_result(&json),
        Some("stream_event") => parse_stream_event(&json),
        _ => vec![StreamEvent::Unknown(json)],
    }
}

fn parse_system(json: &Value) -> Vec<StreamEvent> {
    if json.get("subtype").and_then(|s| s.as_str()) != Some("init") {
        return vec![StreamEvent::Unknown(json.clone())];
    }
    let session_id = json
        .get("session_id")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    let model = json
        .get("model")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    vec![StreamEvent::SystemInit { session_id, model }]
}

fn parse_assistant(json: &Value) -> Vec<StreamEvent> {
    let contents = json.pointer("/message/content").and_then(|c| c.as_array());

    let Some(contents) = contents else {
        return vec![];
    };

    contents
        .iter()
        .filter_map(
            |content| match content.get("type").and_then(|t| t.as_str()) {
                Some("thinking") => {
                    let text = content
                        .get("thinking")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(StreamEvent::AssistantThinking(text))
                }
                Some("text") => {
                    let text = content
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(StreamEvent::AssistantText(text))
                }
                Some("tool_use") => {
                    let id = content
                        .get("id")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = content
                        .get("name")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let input = content.get("input").cloned().unwrap_or(Value::Null);
                    Some(StreamEvent::ToolUse { id, name, input })
                }
                _ => None,
            },
        )
        .collect()
}

fn parse_user(json: &Value) -> Vec<StreamEvent> {
    let contents = json.pointer("/message/content").and_then(|c| c.as_array());

    let Some(contents) = contents else {
        return vec![];
    };

    contents
        .iter()
        .filter_map(|content| {
            if content.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                let tool_use_id = content
                    .get("tool_use_id")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                let text = content
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default()
                    .to_string();
                Some(StreamEvent::ToolResult {
                    tool_use_id,
                    content: text,
                })
            } else {
                None
            }
        })
        .collect()
}

fn parse_rate_limit(json: &Value) -> Vec<StreamEvent> {
    let resets_at = json
        .pointer("/rate_limit_info/resetsAt")
        .and_then(|r| r.as_u64())
        .unwrap_or(0);
    vec![StreamEvent::RateLimit { resets_at }]
}

fn parse_stream_event(json: &Value) -> Vec<StreamEvent> {
    let event_type = json.pointer("/event/type").and_then(|t| t.as_str());
    match event_type {
        Some("content_block_delta") => parse_content_block_delta(json),
        Some("message_start") => {
            let model = json
                .pointer("/event/message/model")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string();
            let id = json
                .pointer("/event/message/id")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::MessageStart { model, id }]
        }
        Some("content_block_start") => {
            let index = json
                .pointer("/event/index")
                .and_then(|i| i.as_u64())
                .unwrap_or(0);
            let block_type = json
                .pointer("/event/content_block/type")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::ContentBlockStart { index, block_type }]
        }
        Some("content_block_stop") => {
            let index = json
                .pointer("/event/index")
                .and_then(|i| i.as_u64())
                .unwrap_or(0);
            vec![StreamEvent::ContentBlockStop { index }]
        }
        Some("message_delta") => {
            let stop_reason = json
                .pointer("/event/delta/stop_reason")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            vec![StreamEvent::MessageDelta { stop_reason }]
        }
        Some("message_stop") => vec![StreamEvent::MessageStop],
        Some("ping") => vec![StreamEvent::Ping],
        Some("error") => {
            let error_type = json
                .pointer("/event/error/type")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string();
            let message = json
                .pointer("/event/error/message")
                .and_then(|s| s.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::Error {
                error_type,
                message,
            }]
        }
        _ => vec![StreamEvent::Unknown(json.clone())],
    }
}

fn parse_content_block_delta(json: &Value) -> Vec<StreamEvent> {
    let delta_type = json.pointer("/event/delta/type").and_then(|t| t.as_str());
    match delta_type {
        Some("text_delta") => {
            let text = json
                .pointer("/event/delta/text")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::Text(text)]
        }
        Some("thinking_delta") => {
            let thinking = json
                .pointer("/event/delta/thinking")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::Thinking(thinking)]
        }
        Some("input_json_delta") => {
            let partial = json
                .pointer("/event/delta/partial_json")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::InputJsonDelta(partial)]
        }
        Some("signature_delta") => {
            let sig = json
                .pointer("/event/delta/signature")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();
            vec![StreamEvent::SignatureDelta(sig)]
        }
        Some("citations_delta") => {
            let citation = json
                .pointer("/event/delta/citation")
                .cloned()
                .unwrap_or(Value::Null);
            vec![StreamEvent::CitationsDelta(citation)]
        }
        _ => vec![StreamEvent::Unknown(json.clone())],
    }
}

fn parse_result(json: &Value) -> Vec<StreamEvent> {
    match serde_json::from_value::<ClaudeResponse>(json.clone()) {
        Ok(resp) => vec![StreamEvent::Result(resp)],
        Err(_) => vec![StreamEvent::Unknown(json.clone())],
    }
}

/// Parses an NDJSON byte stream into a [`Stream`] of [`StreamEvent`]s.
///
/// Reads lines from the given `reader`, strips ANSI escapes, parses JSON,
/// and yields `StreamEvent`s. Unparsable lines are silently skipped.
pub(crate) fn parse_stream(
    reader: impl AsyncBufRead + Unpin + Send + 'static,
) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send>> {
    Box::pin(stream! {
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            for event in parse_event(&line) {
                yield event;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio_stream::StreamExt;

    #[test]
    fn parse_system_init() {
        let line = r#"{"type":"system","subtype":"init","session_id":"sess-1","model":"haiku"}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], StreamEvent::SystemInit { session_id, model }
            if session_id == "sess-1" && model == "haiku")
        );
    }

    #[test]
    fn parse_system_non_init_is_unknown() {
        let line = r#"{"type":"system","subtype":"hook_started"}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Unknown(_)));
    }

    #[test]
    fn parse_assistant_thinking() {
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::AssistantThinking(t) if t == "hmm"));
    }

    #[test]
    fn parse_assistant_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::AssistantText(t) if t == "hello"));
    }

    #[test]
    fn parse_tool_use() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"tu_1","name":"Read","input":{"path":"/tmp"}}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::ToolUse { id, name, .. }
            if id == "tu_1" && name == "Read"));
    }

    #[test]
    fn parse_tool_result() {
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"tu_1","content":"file contents"}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], StreamEvent::ToolResult { tool_use_id, content }
            if tool_use_id == "tu_1" && content == "file contents")
        );
    }

    #[test]
    fn parse_rate_limit() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"resetsAt":1700000000}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            StreamEvent::RateLimit {
                resets_at: 1700000000
            }
        ));
    }

    #[test]
    fn parse_result_event() {
        let fixture = include_str!("../tests/fixtures/stream_success.ndjson");
        let last_line = fixture.lines().last().unwrap();
        let events = parse_event(last_line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Result(resp) if resp.result == "Hello!"));
    }

    #[test]
    fn parse_multiple_content_blocks() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"hello"}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], StreamEvent::AssistantThinking(t) if t == "hmm"));
        assert!(matches!(&events[1], StreamEvent::AssistantText(t) if t == "hello"));
    }

    #[test]
    fn parse_stream_event_text_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"hello"}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Text(t) if t == "hello"));
    }

    #[test]
    fn parse_stream_event_thinking_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Thinking(t) if t == "hmm"));
    }

    #[test]
    fn parse_stream_event_message_start() {
        let line = r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_01","model":"haiku","role":"assistant","content":[]}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::MessageStart { model, id }
            if model == "haiku" && id == "msg_01"));
    }

    #[test]
    fn parse_stream_event_content_block_start() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], StreamEvent::ContentBlockStart { index: 0, block_type }
            if block_type == "thinking")
        );
    }

    #[test]
    fn parse_stream_event_content_block_stop() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_stop","index":1}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            StreamEvent::ContentBlockStop { index: 1 }
        ));
    }

    #[test]
    fn parse_stream_event_message_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":50}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], StreamEvent::MessageDelta { stop_reason }
            if stop_reason.as_deref() == Some("end_turn"))
        );
    }

    #[test]
    fn parse_stream_event_message_stop() {
        let line = r#"{"type":"stream_event","event":{"type":"message_stop"}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::MessageStop));
    }

    #[test]
    fn parse_stream_event_ping() {
        let line = r#"{"type":"stream_event","event":{"type":"ping"}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Ping));
    }

    #[test]
    fn parse_stream_event_error() {
        let line = r#"{"type":"stream_event","event":{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(&events[0], StreamEvent::Error { error_type, message }
            if error_type == "overloaded_error" && message == "Overloaded")
        );
    }

    #[test]
    fn parse_stream_event_input_json_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"\"path\":"}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::InputJsonDelta(s) if s == "\"path\":"));
    }

    #[test]
    fn parse_stream_event_signature_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"abc123"}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::SignatureDelta(s) if s == "abc123"));
    }

    #[test]
    fn parse_stream_event_citations_delta() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"citations_delta","citation":{"url":"https://example.com"}}}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::CitationsDelta(_)));
    }

    #[test]
    fn parse_unknown_type_preserved() {
        let line = r#"{"type":"future_event","data":"something"}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Unknown(v) if v["type"] == "future_event"));
    }

    #[test]
    fn parse_ansi_wrapped_line() {
        let line = "\x1b[?1004l{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}";
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::AssistantText(t) if t == "hi"));
    }

    #[test]
    fn parse_empty_line() {
        assert!(parse_event("").is_empty());
    }

    #[test]
    fn parse_invalid_json() {
        assert!(parse_event("not json at all").is_empty());
    }

    #[tokio::test]
    async fn parse_stream_full_sequence() {
        let ndjson = include_str!("../tests/fixtures/stream_success.ndjson");
        let reader = Cursor::new(ndjson.as_bytes().to_vec());
        let mut stream = parse_stream(reader);

        // 1st event: SystemInit
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::SystemInit { .. }));

        // 2nd event: AssistantThinking (from assistant event)
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::AssistantThinking(_)));

        // 3rd event: AssistantText (from assistant event)
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::AssistantText(ref t) if t == "Hello!"));

        // 4th event: Result
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::Result(ref r) if r.result == "Hello!"));

        // Stream ends
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn parse_stream_skips_invalid_lines() {
        let input = "not json\n\n{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}\n";
        let reader = Cursor::new(input.as_bytes().to_vec());
        let mut stream = parse_stream(reader);

        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::AssistantText(ref t) if t == "ok"));

        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn parse_stream_ansi_first_line() {
        let input = "\x1b[?1004l{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\",\"model\":\"haiku\"}\n";
        let reader = Cursor::new(input.as_bytes().to_vec());
        let mut stream = parse_stream(reader);

        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::SystemInit { .. }));
    }
}
