# Conversation API 実装プラン

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `session_id` を自動管理する `Conversation` ラッパーを追加し、複数ターン会話を簡潔に書けるようにする

**Architecture:** `Conversation<R>` が `ClaudeConfig` + `R: CommandRunner` + `Arc<Mutex<Option<String>>>` を所有。各ターンで config を clone し `--resume` を注入して一時 `ClaudeClient` で実行。ストリームは `wrap_stream` ヘルパーで session_id をキャプチャ

**Tech Stack:** Rust 1.93+, tokio, tokio-stream, async-stream, serde_json

**設計ドキュメント:** `docs/superpowers/specs/2026-04-06-conversation-api-design.md`

---

## ファイル構成

| 操作 | パス | 責務 |
|------|------|------|
| 新規 | `src/conversation.rs` | `Conversation` 構造体・全メソッド・テスト |
| 変更 | `src/config.rs:68-73` | `to_builder()` メソッド追加 |
| 変更 | `src/client.rs:116` | `conversation()` / `conversation_resume()` メソッド追加 |
| 変更 | `src/lib.rs` | `mod conversation` + re-export |
| 変更 | `examples/multi_turn.rs` | Conversation API で書き直し |
| 変更 | `CLAUDE.md` | Architecture にファイル追記 |

---

### Task 1: `ClaudeConfig::to_builder()`

`Conversation::ask_with()` のクロージャが `ClaudeConfigBuilder` を受け取るため、`ClaudeConfig` → `ClaudeConfigBuilder` 変換が必要。

**Files:**
- Modify: `src/config.rs:68-73` (impl ClaudeConfig ブロック内)
- Test: `src/config.rs` (既存テストモジュール末尾)

- [ ] **Step 1: テスト追加（Red）**

`src/config.rs` のテストモジュール末尾に追加:

```rust
#[test]
fn to_builder_round_trip_fields() {
    let original = ClaudeConfig::builder()
        .model("haiku")
        .system_prompt("test")
        .max_turns(5)
        .timeout(Duration::from_secs(30))
        .no_session_persistence(false)
        .resume("session-123")
        .build();

    let rebuilt = original.to_builder().build();

    assert_eq!(rebuilt.model, original.model);
    assert_eq!(rebuilt.system_prompt, original.system_prompt);
    assert_eq!(rebuilt.max_turns, original.max_turns);
    assert_eq!(rebuilt.timeout, original.timeout);
    assert_eq!(rebuilt.no_session_persistence, original.no_session_persistence);
    assert_eq!(rebuilt.resume, original.resume);
}

#[test]
fn to_builder_round_trip_args() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .max_turns(3)
        .effort("high")
        .allowed_tools(["Bash", "Read"])
        .no_session_persistence(false)
        .build();

    let rebuilt = config.to_builder().build();
    assert_eq!(config.to_args("hi"), rebuilt.to_args("hi"));
}
```

- [ ] **Step 2: テスト失敗を確認**

Run: `cargo test --lib config::tests::to_builder`
Expected: コンパイルエラー — `to_builder` メソッドが存在しない

- [ ] **Step 3: `to_builder()` を実装**

`src/config.rs` の `impl ClaudeConfig` ブロック内（`builder()` の直後）に追加:

```rust
/// Creates a builder pre-filled with this configuration's values.
#[must_use]
pub fn to_builder(&self) -> ClaudeConfigBuilder {
    ClaudeConfigBuilder {
        model: self.model.clone(),
        system_prompt: self.system_prompt.clone(),
        append_system_prompt: self.append_system_prompt.clone(),
        max_turns: self.max_turns,
        timeout: self.timeout,
        fallback_model: self.fallback_model.clone(),
        effort: self.effort.clone(),
        max_budget_usd: self.max_budget_usd,
        allowed_tools: self.allowed_tools.clone(),
        disallowed_tools: self.disallowed_tools.clone(),
        tools: self.tools.clone(),
        mcp_config: self.mcp_config.clone(),
        setting_sources: self.setting_sources.clone(),
        settings: self.settings.clone(),
        json_schema: self.json_schema.clone(),
        include_partial_messages: self.include_partial_messages,
        include_hook_events: self.include_hook_events,
        permission_mode: self.permission_mode.clone(),
        dangerously_skip_permissions: self.dangerously_skip_permissions,
        add_dir: self.add_dir.clone(),
        file: self.file.clone(),
        resume: self.resume.clone(),
        session_id: self.session_id.clone(),
        bare: self.bare,
        no_session_persistence: self.no_session_persistence,
        disable_slash_commands: self.disable_slash_commands,
        strict_mcp_config: self.strict_mcp_config,
        extra_args: self.extra_args.clone(),
    }
}
```

- [ ] **Step 4: テスト成功を確認**

Run: `cargo test --lib config::tests::to_builder`
Expected: 2 件 PASS

- [ ] **Step 5: コミット**

```bash
git add src/config.rs
git commit -m "feat: ClaudeConfig::to_builder() を追加"
```

---

### Task 2: `Conversation` 構造体 + `ask_with()` + ユニットテスト

`Conversation` の中核部分。`RecordingRunner`（Clone 可能なテスト用ランナー）を定義し、ask パスのテストを全て書く。

**Files:**
- Create: `src/conversation.rs`

- [ ] **Step 1: テスト用ヘルパー + 全テスト追加（Red）**

`src/conversation.rs` を作成。まずテストモジュールとヘルパーから:

```rust
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio_stream::Stream;

use crate::client::{ClaudeClient, CommandRunner, DefaultRunner};
use crate::config::{ClaudeConfig, ClaudeConfigBuilder};
use crate::error::ClaudeError;
use crate::types::{ClaudeResponse, StreamEvent};

/// Stateful multi-turn conversation wrapper around [`ClaudeClient`].
///
/// Manages `session_id` automatically across turns using `--resume`.
/// The base config is cloned per turn; each turn builds a temporary
/// config with `--resume <session_id>` injected.
///
/// # Design decisions
///
/// **Ownership model:** Owns cloned copies of [`ClaudeConfig`] and the runner
/// instead of borrowing `&ClaudeClient`. `ClaudeClient` is stateless (config +
/// runner only, no connection pool), so cloning is cheap and avoids lifetime
/// parameters that complicate async usage (spawn, struct storage).
///
/// **session_id storage:** Uses `Arc<Mutex<Option<String>>>` so that the
/// streaming path can update the session ID while the caller consumes the
/// returned `Stream` (which outlives the `&mut self` borrow).
///
/// # Note
///
/// Callers must set [`ClaudeConfigBuilder::no_session_persistence`]`(false)` in
/// the config for multi-turn to work. The library does not override this; option
/// validation is the CLI's responsibility.
#[derive(Debug)]
pub struct Conversation<R: CommandRunner = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
    session_id: Arc<Mutex<Option<String>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    /// A [`CommandRunner`] that records arguments and returns pre-configured
    /// responses. Clone-compatible (unlike mockall mocks), which is required
    /// for `Conversation` since it clones the runner for each turn.
    #[derive(Clone)]
    struct RecordingRunner {
        responses: Arc<Mutex<VecDeque<io::Result<Output>>>>,
        captured_args: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl RecordingRunner {
        fn new(responses: Vec<io::Result<Output>>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(VecDeque::from(responses))),
                captured_args: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn captured_args(&self) -> Vec<Vec<String>> {
            self.captured_args.lock().unwrap().clone()
        }
    }

    impl CommandRunner for RecordingRunner {
        async fn run(&self, args: &[String]) -> io::Result<Output> {
            self.captured_args.lock().unwrap().push(args.to_vec());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("RecordingRunner: no more responses")
        }
    }

    fn make_success_output(session_id: &str) -> io::Result<Output> {
        let json = format!(
            r#"{{"type":"result","subtype":"success","is_error":false,"duration_ms":100,"duration_api_ms":90,"num_turns":1,"result":"Hello!","stop_reason":"end_turn","session_id":"{session_id}","total_cost_usd":0.001,"usage":{{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5,"server_tool_use":{{"web_search_requests":0,"web_fetch_requests":0}}}}}}"#
        );
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: json.into_bytes(),
            stderr: Vec::new(),
        })
    }

    #[tokio::test]
    async fn session_id_initially_none() {
        let runner = RecordingRunner::new(vec![]);
        let conv = Conversation::with_runner(ClaudeConfig::default(), runner);
        assert!(conv.session_id().is_none());
    }

    #[tokio::test]
    async fn ask_captures_session_id() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner);

        let resp = conv.ask("hello").await.unwrap();
        assert_eq!(resp.session_id, "sid-001");
        assert_eq!(conv.session_id(), Some("sid-001".to_string()));
    }

    #[tokio::test]
    async fn second_turn_sends_resume() {
        let runner = RecordingRunner::new(vec![
            make_success_output("sid-001"),
            make_success_output("sid-001"),
        ]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner.clone());

        conv.ask("turn 1").await.unwrap();
        conv.ask("turn 2").await.unwrap();

        let args = runner.captured_args();
        // Turn 1: no --resume
        assert!(!args[0].contains(&"--resume".to_string()));
        // Turn 2: --resume sid-001
        let idx = args[1].iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[1][idx + 1], "sid-001");
    }

    #[tokio::test]
    async fn ask_with_overrides_config() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner.clone());

        conv.ask_with("hello", |b| b.max_turns(5)).await.unwrap();

        let args = &runner.captured_args()[0];
        let idx = args.iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[idx + 1], "5");
    }

    #[tokio::test]
    async fn ask_with_does_not_affect_base_config() {
        let runner = RecordingRunner::new(vec![
            make_success_output("sid-001"),
            make_success_output("sid-001"),
        ]);
        let config = ClaudeConfig::builder().max_turns(1).build();
        let mut conv = Conversation::with_runner(config, runner.clone());

        conv.ask_with("turn 1", |b| b.max_turns(5)).await.unwrap();
        conv.ask("turn 2").await.unwrap();

        let args = runner.captured_args();
        let idx1 = args[0].iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[0][idx1 + 1], "5");
        let idx2 = args[1].iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[1][idx2 + 1], "1");
    }

    #[tokio::test]
    async fn error_preserves_session_id() {
        let error_output: io::Result<Output> = Ok(Output {
            status: ExitStatus::from_raw(256), // exit code 1
            stdout: Vec::new(),
            stderr: b"error".to_vec(),
        });
        let runner = RecordingRunner::new(vec![
            make_success_output("sid-001"),
            error_output,
        ]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner);

        conv.ask("turn 1").await.unwrap();
        assert_eq!(conv.session_id(), Some("sid-001".to_string()));

        let _ = conv.ask("turn 2").await;
        assert_eq!(conv.session_id(), Some("sid-001".to_string()));
    }

    #[tokio::test]
    async fn conversation_resume_sends_resume_on_first_turn() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let mut conv = Conversation::with_runner_resume(
            ClaudeConfig::default(),
            runner.clone(),
            "existing-sid",
        );

        conv.ask("hello").await.unwrap();

        let args = &runner.captured_args()[0];
        let idx = args.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[idx + 1], "existing-sid");
    }
}
```

- [ ] **Step 2: テ��ト失敗を確認**

`src/lib.rs` に `mod conversation;` を追加してからテスト実行:

```rust
mod conversation;
```

Run: `cargo test --lib conversation::tests`
Expected: コンパイルエラー — `Conversation` のメソッドが未実装

- [ ] **Step 3: `Conversation` 実装（Green）**

`src/conversation.rs` の `struct Conversation` 定義の後（テストモジュールの前）に追加:

```rust
impl<R: CommandRunner> Conversation<R> {
    /// Returns the current session ID, or `None` if no turn has completed.
    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.session_id.lock().unwrap().clone()
    }
}

impl<R: CommandRunner + Clone> Conversation<R> {
    /// Creates a new conversation (internal; use [`ClaudeClient::conversation`]).
    pub(crate) fn with_runner(config: ClaudeConfig, runner: R) -> Self {
        Self {
            config,
            runner,
            session_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates a conversation resuming an existing session (internal;
    /// use [`ClaudeClient::conversation_resume`]).
    pub(crate) fn with_runner_resume(
        config: ClaudeConfig,
        runner: R,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            config,
            runner,
            session_id: Arc::new(Mutex::new(Some(session_id.into()))),
        }
    }

    /// Sends a prompt and returns the response.
    ///
    /// Shorthand for `ask_with(prompt, |b| b)`.
    pub async fn ask(&mut self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> {
        self.ask_with(prompt, |b| b).await
    }

    /// Sends a prompt with per-turn config overrides and returns the response.
    ///
    /// The closure receives a [`ClaudeConfigBuilder`] pre-filled with the base
    /// config. Overrides apply to this turn only; the base config is unchanged.
    pub async fn ask_with<F>(
        &mut self,
        prompt: &str,
        config_fn: F,
    ) -> Result<ClaudeResponse, ClaudeError>
    where
        F: FnOnce(ClaudeConfigBuilder) -> ClaudeConfigBuilder,
    {
        let builder = config_fn(self.config.to_builder());
        let mut config = builder.build();

        if let Some(ref id) = *self.session_id.lock().unwrap() {
            config.resume = Some(id.clone());
        }

        let client = ClaudeClient::with_runner(config, self.runner.clone());
        let response = client.ask(prompt).await?;

        *self.session_id.lock().unwrap() = Some(response.session_id.clone());

        Ok(response)
    }
}
```

- [ ] **Step 4: テスト成功を確認**

Run: `cargo test --lib conversation::tests`
Expected: 7 件す���て PASS

- [ ] **Step 5: clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`

- [ ] **Step 6: コミット**

```bash
git add src/conversation.rs src/lib.rs
git commit -m "feat: Conversation 構造体と ask/ask_with を実装"
```

---

### Task 3: ストリーム `wrap_stream` + `ask_stream_with()` + テスト

ストリーミングパスの実装。`wrap_stream` ヘルパーを合成ストリームでテストする。

**Files:**
- Modify: `src/conversation.rs`

- [ ] **Step 1: ストリームテスト追���（Red）**

`src/conversation.rs` のテストモジュール末��に追加:

```rust
use crate::types::Usage;

#[tokio::test]
async fn wrap_stream_captures_session_id_from_system_init() {
    let session_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let events: Vec<Result<StreamEvent, ClaudeError>> = vec![
        Ok(StreamEvent::SystemInit {
            session_id: "sid-stream-001".into(),
            model: "haiku".into(),
        }),
        Ok(StreamEvent::AssistantText("Hello!".into())),
    ];
    let inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> =
        Box::pin(tokio_stream::iter(events));

    let wrapped = wrap_stream(inner, Arc::clone(&session_id));
    tokio::pin!(wrapped);

    let mut count = 0;
    while let Some(_) = tokio_stream::StreamExt::next(&mut wrapped).await {
        count += 1;
    }

    assert_eq!(*session_id.lock().unwrap(), Some("sid-stream-001".to_string()));
    assert_eq!(count, 2);
}

#[tokio::test]
async fn wrap_stream_updates_session_id_from_result() {
    let session_id: Arc<Mutex<Option<String>>> =
        Arc::new(Mutex::new(Some("old-sid".to_string())));
    let response = ClaudeResponse {
        result: "Hello!".into(),
        is_error: false,
        duration_ms: 100,
        num_turns: 1,
        session_id: "new-sid".into(),
        total_cost_usd: 0.001,
        stop_reason: "end_turn".into(),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        },
    };
    let events: Vec<Result<StreamEvent, ClaudeError>> = vec![
        Ok(StreamEvent::SystemInit {
            session_id: "old-sid".into(),
            model: "haiku".into(),
        }),
        Ok(StreamEvent::Result(response)),
    ];
    let inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> =
        Box::pin(tokio_stream::iter(events));

    let wrapped = wrap_stream(inner, Arc::clone(&session_id));
    tokio::pin!(wrapped);
    while let Some(_) = tokio_stream::StreamExt::next(&mut wrapped).await {}

    assert_eq!(*session_id.lock().unwrap(), Some("new-sid".to_string()));
}
```

- [ ] **Step 2: テスト失敗を確認**

Run: `cargo test --lib conversation::tests::wrap_stream`
Expected: コンパイル���ラー — `wrap_stream` が未定��

- [ ] **Step 3: `wrap_stream` + `ask_stream_with` + `ask_stream` 実装（Green）**

`src/conversation.rs` の `impl<R: CommandRunner + Clone>` ブロックの後に追加:

```rust
/// Wraps a stream to transparently capture `session_id` from
/// [`StreamEvent::SystemInit`] and [`StreamEvent::Result`].
fn wrap_stream(
    inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>,
    session_id: Arc<Mutex<Option<String>>>,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> {
    Box::pin(async_stream::stream! {
        tokio::pin!(inner);
        while let Some(item) = tokio_stream::StreamExt::next(&mut inner).await {
            if let Ok(ref event) = item {
                match event {
                    StreamEvent::SystemInit { session_id: sid, .. } => {
                        *session_id.lock().unwrap() = Some(sid.clone());
                    }
                    StreamEvent::Result(response) => {
                        *session_id.lock().unwrap() = Some(response.session_id.clone());
                    }
                    _ => {}
                }
            }
            yield item;
        }
    })
}

impl Conversation {
    /// Sends a prompt and returns a stream of events.
    ///
    /// Shorthand for `ask_stream_with(prompt, |b| b)`.
    pub async fn ask_stream(
        &mut self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        self.ask_stream_with(prompt, |b| b).await
    }

    /// Sends a prompt with per-turn config overrides and returns a stream.
    ///
    /// The closure receives a [`ClaudeConfigBuilder`] pre-filled with the base
    /// config. Overrides apply to this turn only; the base config is unchanged.
    ///
    /// All events are passed through transparently. Internally, `session_id`
    /// is captured from [`StreamEvent::SystemInit`] and updated from
    /// [`StreamEvent::Result`].
    pub async fn ask_stream_with<F>(
        &mut self,
        prompt: &str,
        config_fn: F,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    where
        F: FnOnce(ClaudeConfigBuilder) -> ClaudeConfigBuilder,
    {
        let builder = config_fn(self.config.to_builder());
        let mut config = builder.build();

        if let Some(ref id) = *self.session_id.lock().unwrap() {
            config.resume = Some(id.clone());
        }

        let client = ClaudeClient::new(config);
        let inner = client.ask_stream(prompt).await?;

        Ok(wrap_stream(inner, Arc::clone(&self.session_id)))
    }
}
```

- [ ] **Step 4: テスト成功を確認**

Run: `cargo test --lib conversation::tests::wrap_stream`
Expected: 2 件 PASS

- [ ] **Step 5: clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`

- [ ] **Step 6: コミット**

```bash
git add src/conversation.rs
git commit -m "feat: Conversation にストリーミング対応 (ask_stream/ask_stream_with) を追加"
```

---

### Task 4: `ClaudeClient` convenience メソッド + テスト

`client.conversation()` / `client.conversation_resume()` を追加。テストは `RecordingRunner` のある `conversation::tests` に配置。

**Files:**
- Modify: `src/client.rs:116` (`impl<R: CommandRunner>` ブロックの後)
- Modify: `src/conversation.rs` (テスト追加)

- [ ] **Step 1: テスト追加（Red）**

`src/conversation.rs` のテストモジュール末尾に追加:

```rust
#[tokio::test]
async fn client_conversation_creates_working_conversation() {
    let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
    let config = ClaudeConfig::builder().model("haiku").build();
    let client = ClaudeClient::with_runner(config, runner);

    let mut conv = client.conversation();
    let resp = conv.ask("hello").await.unwrap();
    assert_eq!(resp.session_id, "sid-001");
}

#[tokio::test]
async fn client_conversation_resume_sends_resume() {
    let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
    let client = ClaudeClient::with_runner(ClaudeConfig::default(), runner.clone());

    let mut conv = client.conversation_resume("existing-sid");
    conv.ask("hello").await.unwrap();

    let args = &runner.captured_args()[0];
    let idx = args.iter().position(|a| a == "--resume").unwrap();
    assert_eq!(args[idx + 1], "existing-sid");
}
```

- [ ] **Step 2: テスト失敗を確認**

Run: `cargo test --lib conversation::tests::client_conversation`
Expected: コンパイルエラー — `conversation()` メソッドが存在しない

- [ ] **Step 3: `ClaudeClient` にメソッド追加（Green）**

`src/client.rs` の `impl<R: CommandRunner> ClaudeClient<R>` ブロックの後に新しい impl ブロック追加:

```rust
use crate::conversation::Conversation;

impl<R: CommandRunner + Clone> ClaudeClient<R> {
    /// Creates a new [`Conversation`] for multi-turn interaction.
    ///
    /// The conversation manages `session_id` automatically, injecting
    /// `--resume` from the second turn onwards.
    ///
    /// Callers must set [`crate::ClaudeConfigBuilder::no_session_persistence`]`(false)`
    /// for multi-turn to work.
    #[must_use]
    pub fn conversation(&self) -> Conversation<R> {
        Conversation::with_runner(self.config.clone(), self.runner.clone())
    }

    /// Creates a [`Conversation`] that resumes an existing session.
    ///
    /// The first `ask()` / `ask_stream()` call will include `--resume`
    /// with the given session ID.
    #[must_use]
    pub fn conversation_resume(&self, session_id: impl Into<String>) -> Conversation<R> {
        Conversation::with_runner_resume(self.config.clone(), self.runner.clone(), session_id)
    }
}
```

注意: `use crate::conversation::Conversation;` はファイル先頭の import ブロックに追加する。

- [ ] **Step 4: テスト成功を確認**

Run: `cargo test --lib conversation::tests::client_conversation`
Expected: 2 件 PASS

- [ ] **Step 5: 全テスト実行 + clippy + fmt**

Run: `cargo test --lib && cargo clippy -- -D warnings && cargo fmt`
Expected: 全 PASS

- [ ] **Step 6: コミット**

```bash
git add src/client.rs src/conversation.rs
git commit -m "feat: ClaudeClient に conversation()/conversation_resume() を追加"
```

---

### Task 5: `lib.rs` re-export + example 更新 + CLAUDE.md 更新

公開 API を整え、example を Conversation API で書き直す。

**Files:**
- Modify: `src/lib.rs`
- Modify: `examples/multi_turn.rs`
- Modify: `CLAUDE.md`

- [ ] **Step 1: `lib.rs` に re-export 追加**

`src/lib.rs` を以下に変更:

```rust
mod client;
mod config;
mod conversation;
mod error;
mod stream;
mod types;

pub use client::{ClaudeClient, CommandRunner, DefaultRunner};
pub use config::{ClaudeConfig, ClaudeConfigBuilder, effort, permission_mode};
pub use conversation::Conversation;
pub use error::ClaudeError;
pub use tokio_stream::StreamExt;
pub use types::{ClaudeResponse, StreamEvent, Usage};
```

- [ ] **Step 2: `examples/multi_turn.rs` を Conversation API で書き直し**

```rust
// Multi-turn conversation example using the Conversation API.
// Automatically manages session_id across turns via --resume.
// Requires no_session_persistence(false) so the CLI saves the session to disk.
//
// Usage: cargo run --example multi_turn

#[tokio::main]
async fn main() {
    let config = claude_code_rs::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);
    let mut conv = client.conversation();

    // Turn 1
    println!("[Turn 1] Asking: What is 2+2?");
    let resp1 = match conv.ask("What is 2+2? Answer in one word.").await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };
    println!("[Turn 1] Response: {}", resp1.result);
    println!("[Turn 1] Session: {}", resp1.session_id);

    // Turn 2: session_id is automatically managed
    println!("\n[Turn 2] Asking: What was my previous question?");
    let resp2 = match conv
        .ask("What was my previous question? Repeat it exactly.")
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };
    println!("[Turn 2] Response: {}", resp2.result);
    println!(
        "\nTotal cost: ${:.6}",
        resp1.total_cost_usd + resp2.total_cost_usd
    );
}
```

- [ ] **Step 3: `CLAUDE.md` Architecture 更新**

Architecture セクションのファイル構成に `conversation.rs` を追加:

```plain
src/
  lib.rs           # pub API re-export
  client.rs        # ClaudeClient (CLI実行の中核)
  config.rs        # ClaudeConfig (--model, --system-prompt 等のオプション)
  conversation.rs  # Conversation (session_id 自動管理の複数ターン会話)
  types.rs         # JSON/stream-json 両方の型定義のみ
  error.rs         # エラー型
  stream.rs        # stream-json のパース・イテレーション・バッファリング
```

- [ ] **Step 4: 全テスト + clippy + fmt + doc**

Run: `cargo test --lib && cargo clippy -- -D warnings && cargo fmt --check && cargo doc --no-deps`
Expected: 全 PASS、warning なし

- [ ] **Step 5: コミット**

```bash
git add src/lib.rs examples/multi_turn.rs CLAUDE.md
git commit -m "feat: Conversation を公開 API に追加、example を更新"
```

---

### Task 6: E2E テスト

実際の `claude` CLI を呼び出して複数ターン会話を検証。`#[ignore]` 付き。

**Files:**
- Create: `tests/conversation_e2e.rs`

- [ ] **Step 1: E2E テストファイルを作成**

```rust
//! E2E tests for Conversation API.
//! Requires `claude` CLI in PATH. Run with `cargo test -- --ignored`.

use claude_code_rs::{ClaudeClient, ClaudeConfig, StreamExt};

#[tokio::test]
#[ignore]
async fn conversation_two_turn_ask() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = ClaudeClient::new(config);
    let mut conv = client.conversation();

    let resp1 = conv.ask("What is 2+2? Answer with just the number.").await.unwrap();
    assert!(!resp1.is_error);
    assert!(resp1.result.contains('4'));
    assert!(conv.session_id().is_some());

    let resp2 = conv
        .ask("What number did you just tell me? Reply with just the number.")
        .await
        .unwrap();
    assert!(!resp2.is_error);
    assert!(resp2.result.contains('4'));
}

#[tokio::test]
#[ignore]
async fn conversation_stream_then_ask() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = ClaudeClient::new(config);
    let mut conv = client.conversation();

    // Turn 1: stream
    let mut stream = conv
        .ask_stream("What is 3+3? Answer with just the number.")
        .await
        .unwrap();
    while let Some(event) = stream.next().await {
        let _ = event.unwrap();
    }
    assert!(conv.session_id().is_some());

    // Turn 2: ask (same session)
    let resp2 = conv
        .ask("What number did you just tell me? Reply with just the number.")
        .await
        .unwrap();
    assert!(!resp2.is_error);
    assert!(resp2.result.contains('6'));
}
```

- [ ] **Step 2: ビルド確認**

Run: `cargo test --test conversation_e2e -- --ignored --list`
Expected: 2 件リスト表示

- [ ] **Step 3: コミット**

```bash
git add tests/conversation_e2e.rs
git commit -m "test: Conversation API の E2E ��ストを追加"
```

- [ ] **Step 4: E2E 実行（オプション）**

Run: `cargo test --test conversation_e2e -- --ignored`
Expected: 2 件 PASS（`claude` CLI が PATH にある場合）
