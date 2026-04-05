# stream-json 対応 実装プラン

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Claude Code CLI の `--output-format stream-json` をサポートし、リアルタイムイベントを `Stream` で公開する。

**Architecture:** ボトムアップで依存追加 → types(StreamEvent) → config(base_args リファクタ + to_stream_args) → stream(パースロジック) → client(ask_stream) → E2E → example → ドキュメントの順に実装。パースロジックは `impl AsyncBufRead` を受け取る設計にし、`Cursor` でユニットテスト可能にする。

**Tech Stack:** Rust 1.93 (edition 2024), tokio, tokio-stream, async-stream, serde/serde_json, thiserror, mockall 0.14

---

## ファイル構成

| ファイル | 責務 |
| --- | --- |
| `Cargo.toml` | `tokio-stream`, `async-stream` 依存追加 |
| `src/types.rs` | `StreamEvent` enum 追加 |
| `src/config.rs` | `include_partial_messages` フィールド追加、`base_args()` 抽出、`to_stream_args()` 追加 |
| `src/stream.rs` | NDJSON 行パース → `StreamEvent` 変換 (`parse_event()`, `parse_stream()`) |
| `src/client.rs` | `ask_stream()` メソッド追加 |
| `src/lib.rs` | `StreamEvent` エクスポート追加 |
| `tests/fixtures/stream_success.ndjson` | ストリーム正常系 fixture |
| `tests/e2e.rs` | ストリーム E2E テスト追加 |
| `examples/stream.rs` | ストリーミング使用例 |
| `CLAUDE.md` | Architecture, Commands 更新 |
| `docs/claude-cli.md` | stream-json 詳細を記録 |

---

### Task 1: 依存追加

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: `tokio-stream` と `async-stream` を追加する**

```toml
[dependencies]
tokio = { version = "1", features = ["process", "io-util", "rt-multi-thread", "macros", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio-stream = "0.1"
async-stream = "0.3"
```

- [ ] **Step 2: ビルド確認**

Run: `cargo build 2>&1`
Expected: コンパイル成功

- [ ] **Step 3: コミット**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: tokio-stream, async-stream を依存に追加"
```

---

### Task 2: `StreamEvent` 型定義

**Files:**
- Modify: `src/types.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: `StreamEvent` enum を定義する**

`src/types.rs` の末尾（`strip_ansi` 関数の前）に追加:

```rust
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
```

- [ ] **Step 2: `lib.rs` に `StreamEvent` エクスポートを追加する**

`src/lib.rs` を以下に更新:

```rust
mod client;
mod config;
mod error;
mod stream;
mod types;

pub use client::{ClaudeClient, CommandRunner, DefaultRunner};
pub use config::{ClaudeConfig, ClaudeConfigBuilder};
pub use error::ClaudeError;
pub use types::{ClaudeResponse, StreamEvent, Usage};
```

- [ ] **Step 3: ビルド確認**

Run: `cargo build 2>&1`
Expected: コンパイル成功（warning は許容）

- [ ] **Step 4: コミット**

```bash
git add src/types.rs src/lib.rs
git commit -m "feat: StreamEvent enum を追加"
```

---

### Task 3: `config.rs` — `base_args` リファクタ + `to_stream_args`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: `include_partial_messages` フィールドを追加する**

`ClaudeConfig` に追加:

```rust
/// Configuration options for Claude CLI execution.
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfig {
    /// Model to use (`--model`).
    pub model: Option<String>,
    /// System prompt (`--system-prompt`). Defaults to empty string when `None`.
    pub system_prompt: Option<String>,
    /// Maximum number of turns (`--max-turns`).
    pub max_turns: Option<u32>,
    /// Timeout duration. No timeout when `None`.
    pub timeout: Option<Duration>,
    /// Include partial message chunks in stream output (`--include-partial-messages`).
    pub include_partial_messages: Option<bool>,
}
```

`ClaudeConfigBuilder` にも追加:

```rust
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfigBuilder {
    model: Option<String>,
    system_prompt: Option<String>,
    max_turns: Option<u32>,
    timeout: Option<Duration>,
    include_partial_messages: Option<bool>,
}
```

Builder メソッド追加:

```rust
    /// Enables or disables partial message chunks in stream output.
    #[must_use]
    pub fn include_partial_messages(mut self, enabled: bool) -> Self {
        self.include_partial_messages = Some(enabled);
        self
    }
```

`build()` を更新:

```rust
    #[must_use]
    pub fn build(self) -> ClaudeConfig {
        ClaudeConfig {
            model: self.model,
            system_prompt: self.system_prompt,
            max_turns: self.max_turns,
            timeout: self.timeout,
            include_partial_messages: self.include_partial_messages,
        }
    }
```

- [ ] **Step 2: `base_args()` を抽出し `to_args()` をリファクタする**

```rust
impl ClaudeConfig {
    /// Returns a new builder.
    #[must_use]
    pub fn builder() -> ClaudeConfigBuilder {
        ClaudeConfigBuilder::default()
    }

    /// Builds common CLI arguments shared by JSON and stream-json modes.
    fn base_args(&self) -> Vec<String> {
        let mut args = vec![
            "--print".into(),
            "--no-session-persistence".into(),
            "--setting-sources".into(),
            String::new(),
            "--strict-mcp-config".into(),
            "--mcp-config".into(),
            r#"{"mcpServers":{}}"#.into(),
            "--tools".into(),
            String::new(),
            "--disable-slash-commands".into(),
            "--system-prompt".into(),
        ];

        match &self.system_prompt {
            Some(sp) => args.push(sp.clone()),
            None => args.push(String::new()),
        }

        if let Some(model) = &self.model {
            args.push("--model".into());
            args.push(model.clone());
        }

        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".into());
            args.push(max_turns.to_string());
        }

        args
    }

    /// Builds command-line arguments for JSON output mode.
    ///
    /// Includes fixed options such as `--print --output-format json`.
    #[must_use]
    pub fn to_args(&self, prompt: &str) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--output-format".into());
        args.push("json".into());
        args.push(prompt.into());
        args
    }

    /// Builds command-line arguments for stream-json output mode.
    ///
    /// Includes `--verbose` (required for stream-json) and optionally
    /// `--include-partial-messages`.
    #[must_use]
    pub fn to_stream_args(&self, prompt: &str) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--output-format".into());
        args.push("stream-json".into());
        args.push("--verbose".into());

        if self.include_partial_messages == Some(true) {
            args.push("--include-partial-messages".into());
        }

        args.push(prompt.into());
        args
    }
}
```

- [ ] **Step 3: 既存テストを実行して壊れていないことを確認する**

Run: `cargo test --lib config 2>&1`
Expected: 4 tests passed（既存テストがすべてパス）

- [ ] **Step 4: `to_stream_args` のテストを書く**

`src/config.rs` のテストモジュールに追加:

```rust
    #[test]
    fn to_stream_args_minimal() {
        let config = ClaudeConfig::default();
        let args = config.to_stream_args("hello");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(!args.contains(&"json".to_string()));
        assert!(!args.contains(&"--include-partial-messages".to_string()));
        assert_eq!(args.last().unwrap(), "hello");
    }

    #[test]
    fn to_stream_args_with_partial_messages() {
        let config = ClaudeConfig::builder()
            .include_partial_messages(true)
            .build();
        let args = config.to_stream_args("hello");

        assert!(args.contains(&"--include-partial-messages".to_string()));
    }

    #[test]
    fn builder_sets_include_partial_messages() {
        let config = ClaudeConfig::builder()
            .include_partial_messages(true)
            .build();
        assert_eq!(config.include_partial_messages, Some(true));
    }
```

- [ ] **Step 5: テスト実行**

Run: `cargo test --lib config 2>&1`
Expected: 7 tests passed

- [ ] **Step 6: clippy + fmt**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [ ] **Step 7: コミット**

```bash
git add src/config.rs
git commit -m "feat: base_args リファクタ + to_stream_args 追加"
```

---

### Task 4: `stream.rs` — NDJSON パースロジック

**Files:**
- Create: `tests/fixtures/stream_success.ndjson`
- Modify: `src/stream.rs`

- [ ] **Step 1: fixture ファイルを作成する**

`tests/fixtures/stream_success.ndjson`:

```
{"type":"system","subtype":"init","session_id":"test-session-001","model":"claude-haiku-4-5-20251001","cwd":"/tmp","tools":[],"mcp_servers":[]}
{"type":"assistant","message":{"id":"msg_001","type":"message","role":"assistant","content":[{"type":"thinking","thinking":"Let me think...","signature":"sig123"}],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"session_id":"test-session-001"}
{"type":"assistant","message":{"id":"msg_001","type":"message","role":"assistant","content":[{"type":"text","text":"Hello!"}],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":5,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}},"session_id":"test-session-001"}
{"type":"result","subtype":"success","is_error":false,"duration_ms":1000,"duration_api_ms":990,"num_turns":1,"result":"Hello!","stop_reason":"end_turn","session_id":"test-session-001","total_cost_usd":0.001,"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5,"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0}}}
```

- [ ] **Step 2: `parse_event` 関数を実装する**

`src/stream.rs`:

```rust
use serde_json::Value;

use crate::error::ClaudeError;
use crate::types::{ClaudeResponse, StreamEvent, strip_ansi};

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
    let session_id = json.get("session_id")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    let model = json.get("model")
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string();
    vec![StreamEvent::SystemInit { session_id, model }]
}

fn parse_assistant(json: &Value) -> Vec<StreamEvent> {
    let contents = json
        .pointer("/message/content")
        .and_then(|c| c.as_array());

    let Some(contents) = contents else {
        return vec![];
    };

    contents
        .iter()
        .filter_map(|content| {
            match content.get("type").and_then(|t| t.as_str()) {
                Some("thinking") => {
                    let text = content.get("thinking")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(StreamEvent::Thinking(text))
                }
                Some("text") => {
                    let text = content.get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(StreamEvent::Text(text))
                }
                Some("tool_use") => {
                    let id = content.get("id")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let name = content.get("name")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let input = content.get("input")
                        .cloned()
                        .unwrap_or(Value::Null);
                    Some(StreamEvent::ToolUse { id, name, input })
                }
                _ => None,
            }
        })
        .collect()
}

fn parse_user(json: &Value) -> Vec<StreamEvent> {
    let contents = json
        .pointer("/message/content")
        .and_then(|c| c.as_array());

    let Some(contents) = contents else {
        return vec![];
    };

    contents
        .iter()
        .filter_map(|content| {
            if content.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                let tool_use_id = content.get("tool_use_id")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                let text = content.get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or_default()
                    .to_string();
                Some(StreamEvent::ToolResult { tool_use_id, content: text })
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
```

- [ ] **Step 3: テストを書く**

`src/stream.rs` の末尾に追加:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_system_init() {
        let line = r#"{"type":"system","subtype":"init","session_id":"sess-1","model":"haiku"}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::SystemInit { session_id, model }
            if session_id == "sess-1" && model == "haiku"));
    }

    #[test]
    fn parse_system_non_init_skipped() {
        let line = r#"{"type":"system","subtype":"hook_started"}"#;
        let events = parse_event(line);
        assert!(events.is_empty());
    }

    #[test]
    fn parse_thinking() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"}]}}"#;
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
        assert!(matches!(&events[0], StreamEvent::ToolResult { tool_use_id, content }
            if tool_use_id == "tu_1" && content == "file contents"));
    }

    #[test]
    fn parse_rate_limit() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"resetsAt":1700000000}}"#;
        let events = parse_event(line);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], StreamEvent::RateLimit { resets_at: 1700000000 }));
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

    #[test]
    fn parse_unknown_type() {
        let line = r#"{"type":"unknown_event","data":"foo"}"#;
        assert!(parse_event(line).is_empty());
    }
}
```

- [ ] **Step 4: テスト実行**

Run: `cargo test --lib stream 2>&1`
Expected: 12 tests passed

- [ ] **Step 5: clippy + fmt**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [ ] **Step 6: コミット**

```bash
git add src/stream.rs tests/fixtures/stream_success.ndjson
git commit -m "feat: stream-json イベントパーサーを実装"
```

---

### Task 5: `stream.rs` — `parse_stream` 関数

**Files:**
- Modify: `src/stream.rs`

- [ ] **Step 1: `parse_stream` 関数を実装する**

`src/stream.rs` の `parse_event` の後に追加:

```rust
use std::pin::Pin;

use async_stream::stream;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio_stream::Stream;

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
```

- [ ] **Step 2: テストを書く**

`src/stream.rs` のテストモジュールに追加:

```rust
    use std::io::Cursor;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn parse_stream_full_sequence() {
        let ndjson = include_str!("../tests/fixtures/stream_success.ndjson");
        let reader = Cursor::new(ndjson.as_bytes().to_vec());
        let mut stream = parse_stream(reader);

        // 1st event: SystemInit
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::SystemInit { .. }));

        // 2nd event: Thinking
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::Thinking(_)));

        // 3rd event: Text
        let event = stream.next().await.unwrap();
        assert!(matches!(event, StreamEvent::Text(ref t) if t == "Hello!"));

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
        assert!(matches!(event, StreamEvent::Text(ref t) if t == "ok"));

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
```

- [ ] **Step 3: テスト実行**

Run: `cargo test --lib stream 2>&1`
Expected: 15 tests passed

- [ ] **Step 4: clippy + fmt**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [ ] **Step 5: コミット**

```bash
git add src/stream.rs
git commit -m "feat: parse_stream で AsyncBufRead から StreamEvent ストリームを生成"
```

---

### Task 6: `client.rs` — `ask_stream()` メソッド

**Files:**
- Modify: `src/client.rs`

- [ ] **Step 1: `ask_stream()` を実装する**

`src/client.rs` に import を追加:

```rust
use std::pin::Pin;

use tokio::io::BufReader;
use tokio::process::Command as TokioCommand;
use tokio_stream::Stream;

use crate::stream::parse_stream;
use crate::types::StreamEvent;
```

`impl ClaudeClient` ブロック（`new` がある方）に `ask_stream` を追加:

```rust
impl ClaudeClient {
    /// Creates a new client with the default [`DefaultRunner`].
    #[must_use]
    pub fn new(config: ClaudeConfig) -> Self {
        Self {
            config,
            runner: DefaultRunner,
        }
    }

    /// Sends a prompt and returns a stream of events.
    ///
    /// Spawns the CLI with `--output-format stream-json` and streams events
    /// in real-time. The stream ends with a [`StreamEvent::Result`] on success.
    ///
    /// Timeout is not applied to streams. Use `tokio_stream::StreamExt::timeout()`
    /// if needed.
    pub async fn ask_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        let args = self.config.to_stream_args(prompt);

        let mut child = TokioCommand::new("claude")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ClaudeError::CliNotFound
                } else {
                    ClaudeError::Io(e)
                }
            })?;

        let stdout = child.stdout.take().expect("stdout must be piped");
        let reader = BufReader::new(stdout);
        let event_stream = parse_stream(reader);

        Ok(Box::pin(async_stream::stream! {
            tokio::pin!(event_stream);
            while let Some(event) = tokio_stream::StreamExt::next(&mut event_stream).await {
                yield Ok(event);
            }

            let status = child.wait().await;
            match status {
                Ok(s) if !s.success() => {
                    let code = s.code().unwrap_or(-1);
                    let mut stderr_buf = Vec::new();
                    if let Some(mut stderr) = child.stderr.take() {
                        let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_buf).await;
                    }
                    let stderr = String::from_utf8_lossy(&stderr_buf).into_owned();
                    yield Err(ClaudeError::NonZeroExit { code, stderr });
                }
                Err(e) => {
                    yield Err(ClaudeError::Io(e));
                }
                Ok(_) => {}
            }
        }))
    }
}
```

- [ ] **Step 2: ビルド確認**

Run: `cargo build 2>&1`
Expected: コンパイル成功

- [ ] **Step 3: 全テスト実行（既存テストが壊れていないこと）**

Run: `cargo test 2>&1`
Expected: 既存 22 テスト + stream 15 テスト = 37 テスト passed、1 ignored

- [ ] **Step 4: clippy + fmt**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [ ] **Step 5: コミット**

```bash
git add src/client.rs
git commit -m "feat: ask_stream() でリアルタイムイベントストリームを返す"
```

---

### Task 7: E2E テスト

**Files:**
- Modify: `tests/e2e.rs`

- [ ] **Step 1: ストリーム E2E テストを書く**

`tests/e2e.rs` に追加:

```rust
use claude_code_rs::StreamEvent;
use tokio_stream::StreamExt;

#[tokio::test]
#[ignore] // Run explicitly with: cargo test -- --ignored
async fn e2e_ask_stream_with_haiku() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .max_turns(1)
        .build();

    let client = ClaudeClient::new(config);
    let mut stream = client.ask_stream("Say 'hello' and nothing else").await.unwrap();

    let mut got_text = false;
    let mut got_result = false;

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::Text(_) => got_text = true,
            StreamEvent::Result(resp) => {
                assert!(!resp.is_error);
                assert!(!resp.result.is_empty());
                got_result = true;
            }
            _ => {}
        }
    }

    assert!(got_text, "should have received at least one Text event");
    assert!(got_result, "should have received a Result event");
}
```

- [ ] **Step 2: ビルド確認（実行はしない）**

Run: `cargo test --test e2e --no-run 2>&1`
Expected: コンパイル成功

- [ ] **Step 3: E2E テストを実行する**

Run: `cargo test --test e2e -- --ignored 2>&1`
Expected: 2 tests passed

- [ ] **Step 4: コミット**

```bash
git add tests/e2e.rs
git commit -m "test: stream-json E2E テストを追加"
```

---

### Task 8: `examples/stream.rs`

**Files:**
- Create: `examples/stream.rs`

- [ ] **Step 1: ストリーミング example を作成する**

```rust
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let prompt = std::env::args().nth(1).unwrap_or_else(|| "Say hello".into());
    let config = claude_code_rs::ClaudeConfig::builder()
        .max_turns(1)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);

    let mut stream = match client.ask_stream(&prompt).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    while let Some(event) = stream.next().await {
        match event {
            Ok(claude_code_rs::StreamEvent::Text(text)) => print!("{text}"),
            Ok(claude_code_rs::StreamEvent::Result(resp)) => {
                println!("\n---");
                println!("Cost: ${:.6}", resp.total_cost_usd);
                println!("Tokens: {} in / {} out", resp.usage.input_tokens, resp.usage.output_tokens);
            }
            Ok(_) => {}
            Err(e) => eprintln!("\nStream error: {e}"),
        }
    }
    println!();
}
```

- [ ] **Step 2: ビルド確認**

Run: `cargo build --example stream 2>&1`
Expected: コンパイル成功

- [ ] **Step 3: コミット**

```bash
git add examples/stream.rs
git commit -m "feat: ストリーミング example を追加"
```

---

### Task 9: ドキュメント更新

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/claude-cli.md`

- [ ] **Step 1: `CLAUDE.md` を更新する**

Architecture セクションに `examples/stream.rs` を追加:

```plain
examples/
  simple.rs     # 最小限の動作確認用サンプル
  stream.rs     # ストリーミング動作確認用サンプル
```

Commands セクションに追加:

```sh
cargo run --example stream     # ストリーミング動作確認
```

Tech Stack に追加:

```
- ストリーミング: tokio-stream / async-stream
```

- [ ] **Step 2: `docs/claude-cli.md` に stream-json の詳細を追記する**

以下のセクションを追加:

```markdown
## stream-json のイベント型

`--output-format stream-json --verbose` で出力される NDJSON の各イベント:

| type | subtype / content type | 内容 |
| --- | --- | --- |
| `system` | `init` | セッション初期化情報（session_id, model 等） |
| `system` | `hook_started` / `hook_response` | hook の実行（ライブラリではスキップ） |
| `assistant` | content[].type = `thinking` | モデルの思考過程 |
| `assistant` | content[].type = `text` | テキスト応答チャンク |
| `assistant` | content[].type = `tool_use` | ツール呼び出し |
| `user` | content[].type = `tool_result` | ツール実行結果 |
| `rate_limit_event` | — | レートリミット情報 |
| `result` | `success` | 最終結果（`--output-format json` と同じ構造） |

### content 配列の複数要素

1つの `assistant` / `user` イベントの `.message.content[]` に複数ブロックが含まれる場合がある。ライブラリでは各要素を個別の `StreamEvent` として yield する。

### --include-partial-messages

このオプションを付けると、テキストがより細かいチャンク（単語単位レベル）で送信される。デフォルトでは文単位程度のまとまりで送信される。
```

- [ ] **Step 3: コミット**

```bash
git add CLAUDE.md docs/claude-cli.md
git commit -m "docs: stream-json 対応のドキュメントを更新"
```

---

### Task 10: 最終確認

- [ ] **Step 1: 全ユニットテストを実行する**

Run: `cargo test 2>&1`
Expected: 全テスト passed（E2E は `#[ignore]` で除外）

- [ ] **Step 2: clippy + fmt を確認する**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [ ] **Step 3: ドキュメントをビルドする**

Run: `cargo doc --no-deps 2>&1`
Expected: `StreamEvent` を含む全 pub アイテムのドキュメント生成成功
