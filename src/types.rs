use serde::Deserialize;

/// JSON response from the Claude CLI.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ClaudeResponse {
    /// Model response text.
    pub result: String,
    /// Whether this is an error response.
    pub is_error: bool,
    /// Execution duration in milliseconds.
    pub duration_ms: u64,
    /// Number of turns.
    pub num_turns: u32,
    /// Session ID.
    pub session_id: String,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Stop reason.
    pub stop_reason: String,
    /// Token usage.
    pub usage: Usage,
}

/// Token usage.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Usage {
    /// Input token count.
    pub input_tokens: u64,
    /// Output token count.
    pub output_tokens: u64,
    /// Input tokens read from cache.
    pub cache_read_input_tokens: u64,
    /// Input tokens used for cache creation.
    pub cache_creation_input_tokens: u64,
}

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

/// Strips ANSI escape sequences from stdout and extracts the JSON portion.
pub(crate) fn strip_ansi(input: &str) -> &str {
    // CLI output may be wrapped with escape sequences like `\x1b[?1004l{...}\x1b[?1004l`.
    // Extract from the first '{' to the last '}'.
    let start = input.find('{');
    let end = input.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if s <= e => &input[s..=e],
        _ => input,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_success_fixture() {
        let json = include_str!("../tests/fixtures/success.json");
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.result, "Hello!");
        assert!(!resp.is_error);
        assert_eq!(resp.num_turns, 1);
        assert_eq!(resp.usage.input_tokens, 10);
        assert_eq!(resp.usage.output_tokens, 421);
    }

    #[test]
    fn deserialize_error_fixture() {
        let json = include_str!("../tests/fixtures/error_response.json");
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        assert!(resp.is_error);
        assert_eq!(resp.result, "Error: invalid request");
        assert_eq!(resp.total_cost_usd, 0.0);
    }

    #[test]
    fn strip_ansi_with_escape_sequences() {
        let input = "\x1b[?1004l{\"result\":\"hello\"}\x1b[?1004l";
        assert_eq!(strip_ansi(input), "{\"result\":\"hello\"}");
    }

    #[test]
    fn strip_ansi_without_escape_sequences() {
        let input = "{\"result\":\"hello\"}";
        assert_eq!(strip_ansi(input), "{\"result\":\"hello\"}");
    }

    #[test]
    fn strip_ansi_no_json() {
        let input = "no json here";
        assert_eq!(strip_ansi(input), "no json here");
    }
}
