#![allow(dead_code)]
#[allow(unused_imports)]
use crate::error::ClaudeError;
use crate::types::{ClaudeResponse, StreamEvent, strip_ansi};
use serde_json::Value;

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
        _ => vec![],
    }
}

fn parse_system(json: &Value) -> Vec<StreamEvent> {
    if json.get("subtype").and_then(|s| s.as_str()) != Some("init") {
        return vec![];
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
                    Some(StreamEvent::Thinking(text))
                }
                Some("text") => {
                    let text = content
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(StreamEvent::Text(text))
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

fn parse_result(json: &Value) -> Vec<StreamEvent> {
    match serde_json::from_value::<ClaudeResponse>(json.clone()) {
        Ok(resp) => vec![StreamEvent::Result(resp)],
        Err(_) => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_system_non_init_skipped() {
        let line = r#"{"type":"system","subtype":"hook_started"}"#;
        let events = parse_event(line);
        assert!(events.is_empty());
    }

    #[test]
    fn parse_thinking() {
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Thinking(t) if t == "hmm"));
    }

    #[test]
    fn parse_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Text(t) if t == "hello"));
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
        assert!(matches!(&events[0], StreamEvent::Thinking(t) if t == "hmm"));
        assert!(matches!(&events[1], StreamEvent::Text(t) if t == "hello"));
    }

    #[test]
    fn parse_ansi_wrapped_line() {
        let line = "\x1b[?1004l{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}";
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::Text(t) if t == "hi"));
    }

    #[test]
    fn parse_empty_line() {
        assert!(parse_event("").is_empty());
    }

    #[test]
    fn parse_invalid_json() {
        assert!(parse_event("not json at all").is_empty());
    }
}
