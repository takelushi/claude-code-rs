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
    /// Model's thinking process (extended thinking).
    Thinking(String),
    /// Text response chunk.
    Text(String),
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
    /// Final result (same structure as non-streaming response).
    Result(ClaudeResponse),
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
