# Minimal Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Claude Code CLI を `--print --output-format json` で実行し、結果を型安全にパースする最小プロトタイプを TDD で実装する。

**Architecture:** ボトムアップで `error` → `types` → `config` → `client` の順に実装。各モジュールを独立にテストし、`CommandRunner` trait で CLI 実行を抽象化してモックテストを可能にする。

**Tech Stack:** Rust 1.93 (edition 2024), tokio, serde/serde_json, thiserror, mockall 0.14

**Status:** All tasks completed (2026-04-05). 22 unit tests passing, 1 E2E test (ignored), clippy clean, fmt clean.

**Implementation Notes:**
- mockall 0.14 required for native async fn in trait support (0.13 does not work)
- `CommandRunner` trait needs `#[allow(async_fn_in_trait)]` (internal-only trait, `Send` bound warning suppressed)
- mockall's `returning()` does not support async closures; timeout test uses a hand-written `SlowRunner` struct + `tokio::test(start_paused = true)` instead
- Code comments and doc comments are in English (convention established post-implementation)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/error.rs` | `ClaudeError` enum (5 variants) |
| `src/types.rs` | `ClaudeResponse`, `Usage` (デシリアライズ型) |
| `src/config.rs` | `ClaudeConfig`, `ClaudeConfigBuilder` (Builder パターン) |
| `src/client.rs` | `CommandRunner` trait, `DefaultRunner`, `ClaudeClient` |
| `src/lib.rs` | pub re-export |
| `src/stream.rs` | 空 (次イテレーション) |
| `tests/fixtures/success.json` | 正常系 fixture |
| `tests/fixtures/error_response.json` | CLI がエラーレスポンスを返した fixture |
| `tests/e2e.rs` | E2E テスト (`#[ignore]`) |

---

### Task 1: `error.rs` — エラー型定義

**Files:**
- Modify: `src/error.rs`
- Modify: `src/lib.rs`

- [x] **Step 1: エラー型を定義する**

```rust
// src/error.rs
use std::io;

/// claude-code-rs で発生するエラー型。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClaudeError {
    /// `claude` コマンドが PATH に見つからない。
    #[error("claude CLI not found in PATH")]
    CliNotFound,

    /// CLI が非ゼロ終了コードを返した。
    #[error("claude exited with code {code}: {stderr}")]
    NonZeroExit {
        /// 終了コード。
        code: i32,
        /// 標準エラー出力の内容。
        stderr: String,
    },

    /// JSON / stream-json レスポンスのデシリアライズに失敗した。
    #[error("failed to parse response")]
    ParseError(#[from] serde_json::Error),

    /// 指定時間内に応答が返らなかった。
    #[error("request timed out")]
    Timeout,

    /// プロセス起動・stdout/stderr 読み取り等の I/O エラー。
    #[error(transparent)]
    Io(#[from] io::Error),
}
```

- [x] **Step 2: `lib.rs` を更新する**

```rust
// src/lib.rs
mod client;
mod config;
mod error;
mod stream;
mod types;

pub use error::*;
```

他のモジュールは空なので、re-export はまだ `error` のみ。

- [x] **Step 3: ビルドを確認する**

Run: `cargo build 2>&1`
Expected: warning が出るが正常にコンパイル成功

- [x] **Step 4: テストを書く**

```rust
// src/error.rs の末尾に追加
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_not_found_message() {
        let err = ClaudeError::CliNotFound;
        assert_eq!(err.to_string(), "claude CLI not found in PATH");
    }

    #[test]
    fn non_zero_exit_message() {
        let err = ClaudeError::NonZeroExit {
            code: 1,
            stderr: "something went wrong".into(),
        };
        assert_eq!(
            err.to_string(),
            "claude exited with code 1: something went wrong"
        );
    }

    #[test]
    fn timeout_message() {
        let err = ClaudeError::Timeout;
        assert_eq!(err.to_string(), "request timed out");
    }

    #[test]
    fn from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::Other, "disk full");
        let err = ClaudeError::from(io_err);
        assert!(matches!(err, ClaudeError::Io(_)));
        assert_eq!(err.to_string(), "disk full");
    }

    #[test]
    fn from_serde_error() {
        let serde_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = ClaudeError::from(serde_err);
        assert!(matches!(err, ClaudeError::ParseError(_)));
    }
}
```

- [x] **Step 5: テストを実行する**

Run: `cargo test --lib error 2>&1`
Expected: 5 tests passed

- [x] **Step 6: clippy + fmt を確認する**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし、フォーマット差分なし

- [x] **Step 7: コミットする**

```bash
git add src/error.rs src/lib.rs
git commit -m "feat: add ClaudeError type with 5 variants"
```

---

### Task 2: `types.rs` — レスポンス型定義

**Files:**
- Create: `tests/fixtures/success.json`
- Create: `tests/fixtures/error_response.json`
- Modify: `src/types.rs`
- Modify: `src/lib.rs`

- [x] **Step 1: fixture ファイルを作成する**

`tests/fixtures/success.json`:
```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "duration_ms": 5527,
  "duration_api_ms": 5510,
  "num_turns": 1,
  "result": "Hello!",
  "stop_reason": "end_turn",
  "session_id": "b878fe1e-c175-40c8-b9cb-a0999daa229f",
  "total_cost_usd": 0.013,
  "usage": {
    "input_tokens": 10,
    "cache_creation_input_tokens": 5229,
    "cache_read_input_tokens": 43936,
    "output_tokens": 421,
    "server_tool_use": {
      "web_search_requests": 0,
      "web_fetch_requests": 0
    }
  }
}
```

`tests/fixtures/error_response.json`:
```json
{
  "type": "result",
  "subtype": "error",
  "is_error": true,
  "duration_ms": 1200,
  "duration_api_ms": 1100,
  "num_turns": 0,
  "result": "Error: invalid request",
  "stop_reason": "error",
  "session_id": "c999fe2e-d286-41d9-b0dc-b1000eab330f",
  "total_cost_usd": 0.0,
  "usage": {
    "input_tokens": 0,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0,
    "output_tokens": 0,
    "server_tool_use": {
      "web_search_requests": 0,
      "web_fetch_requests": 0
    }
  }
}
```

- [x] **Step 2: テストを書く（Red）**

```rust
// src/types.rs
use serde::Deserialize;

/// Claude CLI の JSON レスポンス。
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct ClaudeResponse {
    /// モデルの応答テキスト。
    pub result: String,
    /// エラーレスポンスかどうか。
    pub is_error: bool,
    /// 実行時間 (ミリ秒)。
    pub duration_ms: u64,
    /// ターン数。
    pub num_turns: u32,
    /// セッション ID。
    pub session_id: String,
    /// 合計コスト (USD)。
    pub total_cost_usd: f64,
    /// 停止理由。
    pub stop_reason: String,
    /// トークン使用量。
    pub usage: Usage,
}

/// トークン使用量。
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct Usage {
    /// 入力トークン数。
    pub input_tokens: u64,
    /// 出力トークン数。
    pub output_tokens: u64,
    /// キャッシュから読んだ入力トークン数。
    pub cache_read_input_tokens: u64,
    /// キャッシュ作成に使った入力トークン数。
    pub cache_creation_input_tokens: u64,
}

/// stdout から ANSI エスケープシーケンスを除去して JSON 部分を抽出する。
pub(crate) fn strip_ansi(input: &str) -> &str {
    // CLI 出力は `\x1b[?1004l{...}\x1b[?1004l` のようにエスケープシーケンスで囲まれる場合がある
    // 最初の '{' から最後の '}' までを抽出する
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
```

- [x] **Step 3: `lib.rs` に re-export を追加する**

```rust
// src/lib.rs
mod client;
mod config;
mod error;
mod stream;
mod types;

pub use error::*;
pub use types::{ClaudeResponse, Usage};
```

- [x] **Step 4: テストを実行する**

Run: `cargo test --lib types 2>&1`
Expected: 5 tests passed

- [x] **Step 5: clippy + fmt を確認する**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [x] **Step 6: コミットする**

```bash
git add src/types.rs src/lib.rs tests/fixtures/
git commit -m "feat: add ClaudeResponse and Usage types with fixture tests"
```

---

### Task 3: `config.rs` — Builder パターン

**Files:**
- Modify: `src/config.rs`
- Modify: `src/lib.rs`

- [x] **Step 1: テストを書く（Red）**

```rust
// src/config.rs
use std::time::Duration;

/// Claude CLI 実行時のオプション設定。
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfig {
    /// 使用するモデル (`--model`)。
    pub model: Option<String>,
    /// システムプロンプト (`--system-prompt`)。`None` なら空文字。
    pub system_prompt: Option<String>,
    /// 最大ターン数 (`--max-turns`)。
    pub max_turns: Option<u32>,
    /// タイムアウト時間。`None` ならタイムアウトなし。
    pub timeout: Option<Duration>,
}

impl ClaudeConfig {
    /// Builder を返す。
    #[must_use]
    pub fn builder() -> ClaudeConfigBuilder {
        ClaudeConfigBuilder::default()
    }

    /// 設定からコマンド引数を組み立てる。
    ///
    /// `--print --output-format json` 等の固定オプションを含む。
    #[must_use]
    pub fn to_args(&self, prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--print".into(),
            "--output-format".into(),
            "json".into(),
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

        args.push(prompt.into());
        args
    }
}

/// `ClaudeConfig` の Builder。
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfigBuilder {
    model: Option<String>,
    system_prompt: Option<String>,
    max_turns: Option<u32>,
    timeout: Option<Duration>,
}

impl ClaudeConfigBuilder {
    /// モデルを設定する。
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// システムプロンプトを設定する。
    #[must_use]
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 最大ターン数を設定する。
    #[must_use]
    pub fn max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = Some(max_turns);
        self
    }

    /// タイムアウトを設定する。
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// `ClaudeConfig` をビルドする。
    #[must_use]
    pub fn build(self) -> ClaudeConfig {
        ClaudeConfig {
            model: self.model,
            system_prompt: self.system_prompt,
            max_turns: self.max_turns,
            timeout: self.timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ClaudeConfig::default();
        assert!(config.model.is_none());
        assert!(config.system_prompt.is_none());
        assert!(config.max_turns.is_none());
        assert!(config.timeout.is_none());
    }

    #[test]
    fn builder_sets_all_fields() {
        let config = ClaudeConfig::builder()
            .model("haiku")
            .system_prompt("You are helpful")
            .max_turns(3)
            .timeout(Duration::from_secs(30))
            .build();

        assert_eq!(config.model.as_deref(), Some("haiku"));
        assert_eq!(config.system_prompt.as_deref(), Some("You are helpful"));
        assert_eq!(config.max_turns, Some(3));
        assert_eq!(config.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn to_args_minimal() {
        let config = ClaudeConfig::default();
        let args = config.to_args("hello");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"json".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--disable-slash-commands".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        // system-prompt は空文字
        let sp_idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[sp_idx + 1], "");
        // model, max-turns は含まれない
        assert!(!args.contains(&"--model".to_string()));
        assert!(!args.contains(&"--max-turns".to_string()));
        // prompt は末尾
        assert_eq!(args.last().unwrap(), "hello");
    }

    #[test]
    fn to_args_with_options() {
        let config = ClaudeConfig::builder()
            .model("haiku")
            .system_prompt("Be concise")
            .max_turns(5)
            .build();
        let args = config.to_args("test prompt");

        let model_idx = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[model_idx + 1], "haiku");

        let sp_idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[sp_idx + 1], "Be concise");

        let mt_idx = args.iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[mt_idx + 1], "5");

        assert_eq!(args.last().unwrap(), "test prompt");
    }
}
```

- [x] **Step 2: `lib.rs` に re-export を追加する**

```rust
// src/lib.rs
mod client;
mod config;
mod error;
mod stream;
mod types;

pub use config::{ClaudeConfig, ClaudeConfigBuilder};
pub use error::*;
pub use types::{ClaudeResponse, Usage};
```

- [x] **Step 3: テストを実行する**

Run: `cargo test --lib config 2>&1`
Expected: 4 tests passed

- [x] **Step 4: clippy + fmt を確認する**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [x] **Step 5: コミットする**

```bash
git add src/config.rs src/lib.rs
git commit -m "feat: add ClaudeConfig with builder pattern and arg generation"
```

---

### Task 4: `client.rs` — CommandRunner trait + ClaudeClient

**Files:**
- Modify: `src/client.rs`
- Modify: `src/lib.rs`
- Possibly modify: `Cargo.toml` (mockall / async-trait の互換性次第)

- [x] **Step 1: mockall + async trait の互換性を検証する**

以下のコードで `mockall` がネイティブ async fn in trait をモックできるか確認する:

```rust
// src/client.rs に一時的に書いて cargo build で確認
use mockall::automock;
use std::process::Output;

#[automock]
pub trait CommandRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;
}
```

Run: `cargo build 2>&1`

成功すれば `mockall` のまま続行。失敗した場合は以下の順で対応:
1. `mockall` を `0.14` に更新して再試行
2. `Cargo.toml` に `async-trait = "0.1"` を追加して `#[async_trait]` を使用
3. どちらもダメならモック用の手動実装に切り替え

- [x] **Step 2: `CommandRunner` trait と `DefaultRunner` を実装する**

```rust
// src/client.rs
use std::process::Output;

use tokio::process::Command;

#[cfg(test)]
use mockall::automock;

/// Trait abstracting CLI execution. Mockable in tests.
#[allow(async_fn_in_trait)]
#[cfg_attr(test, automock)]
pub trait CommandRunner: Send + Sync {
    /// Runs the `claude` command with the given arguments.
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;
}

/// 実際に `tokio::process::Command` で `claude` を実行する。
#[derive(Debug, Clone)]
pub struct DefaultRunner;

impl CommandRunner for DefaultRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        Command::new("claude").args(args).output().await
    }
}
```

- [x] **Step 3: `ClaudeClient` を実装する**

```rust
// src/client.rs に追加

use crate::config::ClaudeConfig;
use crate::error::ClaudeError;
use crate::types::{strip_ansi, ClaudeResponse};

/// Claude Code CLI クライアント。
#[derive(Debug)]
pub struct ClaudeClient<R: CommandRunner = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
}

impl ClaudeClient {
    /// デフォルトの `DefaultRunner` で構築する。
    #[must_use]
    pub fn new(config: ClaudeConfig) -> Self {
        Self {
            config,
            runner: DefaultRunner,
        }
    }
}

impl<R: CommandRunner> ClaudeClient<R> {
    /// テスト用にカスタム `CommandRunner` を注入して構築する。
    #[must_use]
    pub fn with_runner(config: ClaudeConfig, runner: R) -> Self {
        Self { config, runner }
    }

    /// プロンプトを送信し、レスポンスを返す。
    pub async fn ask(&self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> {
        let args = self.config.to_args(prompt);

        let output = if let Some(timeout) = self.config.timeout {
            tokio::time::timeout(timeout, self.runner.run(&args))
                .await
                .map_err(|_| ClaudeError::Timeout)?
        } else {
            self.runner.run(&args).await
        };

        let output = output.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ClaudeError::CliNotFound
            } else {
                ClaudeError::Io(e)
            }
        })?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(ClaudeError::NonZeroExit { code, stderr });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_str = strip_ansi(&stdout);
        let response: ClaudeResponse = serde_json::from_str(json_str)?;
        Ok(response)
    }
}
```

- [x] **Step 4: `lib.rs` を最終形に更新する**

```rust
// src/lib.rs
mod client;
mod config;
mod error;
mod stream;
mod types;

pub use client::{ClaudeClient, CommandRunner, DefaultRunner};
pub use config::{ClaudeConfig, ClaudeConfigBuilder};
pub use error::ClaudeError;
pub use types::{ClaudeResponse, Usage};
```

- [x] **Step 5: ビルドを確認する**

Run: `cargo build 2>&1`
Expected: コンパイル成功（warning は許容）

- [x] **Step 6: テストを書く**

```rust
// src/client.rs の末尾に追加
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn success_output() -> Output {
        Output {
            status: ExitStatus::from_raw(0),
            stdout: include_bytes!("../tests/fixtures/success.json").to_vec(),
            stderr: Vec::new(),
        }
    }

    fn non_zero_output() -> Output {
        Output {
            status: ExitStatus::from_raw(256), // exit code 1
            stdout: Vec::new(),
            stderr: b"something went wrong".to_vec(),
        }
    }

    #[tokio::test]
    async fn ask_success() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| Ok(success_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let resp = client.ask("hello").await.unwrap();
        assert_eq!(resp.result, "Hello!");
        assert!(!resp.is_error);
    }

    #[tokio::test]
    async fn ask_cli_not_found() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found")));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::CliNotFound));
    }

    #[tokio::test]
    async fn ask_non_zero_exit() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| Ok(non_zero_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::NonZeroExit { code: 1, .. }));
    }

    #[tokio::test]
    async fn ask_parse_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| {
                Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: b"not json".to_vec(),
                    stderr: Vec::new(),
                })
            });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::ParseError(_)));
    }

    // NOTE: mockall's returning() does not support async closures.
    // Use a hand-written struct + tokio::test(start_paused = true) instead.

    /// Custom CommandRunner that always sleeps (for timeout tests).
    struct SlowRunner;

    impl CommandRunner for SlowRunner {
        async fn run(&self, _args: &[String]) -> std::io::Result<Output> {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            Ok(Output {
                status: std::os::unix::process::ExitStatusExt::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ask_timeout() {
        let config = ClaudeConfig::builder()
            .timeout(std::time::Duration::from_millis(10))
            .build();
        let client = ClaudeClient::with_runner(config, SlowRunner);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::Timeout));
    }

    #[tokio::test]
    async fn ask_io_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| {
                Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"))
            });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::Io(_)));
    }

    #[tokio::test]
    async fn ask_with_ansi_escape() {
        let json = include_str!("../tests/fixtures/success.json");
        let stdout = format!("\x1b[?1004l{json}\x1b[?1004l");

        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(move |_| {
                Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: stdout.clone().into_bytes(),
                    stderr: Vec::new(),
                })
            });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let resp = client.ask("hello").await.unwrap();
        assert_eq!(resp.result, "Hello!");
    }

    #[tokio::test]
    async fn ask_passes_correct_args() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .withf(|args| {
                args.contains(&"--print".to_string())
                    && args.contains(&"--model".to_string())
                    && args.contains(&"haiku".to_string())
                    && args.last() == Some(&"test prompt".to_string())
            })
            .returning(|_| Ok(success_output()));

        let config = ClaudeConfig::builder().model("haiku").build();
        let client = ClaudeClient::with_runner(config, mock);
        client.ask("test prompt").await.unwrap();
    }
}
```

- [x] **Step 7: テストを実行する**

Run: `cargo test --lib client 2>&1`
Expected: 全テスト passed

注: `ask_timeout` は mockall の制約により手動 `SlowRunner` + `start_paused = true` で実装。詳細は Implementation Notes 参照。

- [x] **Step 8: clippy + fmt を確認する**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [x] **Step 9: コミットする**

```bash
git add src/client.rs src/lib.rs Cargo.toml
git commit -m "feat: add ClaudeClient with CommandRunner trait and mock tests"
```

---

### Task 5: E2E テスト

**Files:**
- Create: `tests/e2e.rs`

- [x] **Step 1: E2E テストを書く**

```rust
// tests/e2e.rs
use claude_code_rs::{ClaudeClient, ClaudeConfig};
use std::time::Duration;

#[tokio::test]
#[ignore] // cargo test -- --ignored で明示的に実行
async fn e2e_ask_with_haiku() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .max_turns(1)
        .timeout(Duration::from_secs(30))
        .build();

    let client = ClaudeClient::new(config);
    let resp = client.ask("Say 'hello' and nothing else").await.unwrap();

    assert!(!resp.is_error);
    assert!(!resp.result.is_empty());
    assert!(resp.num_turns >= 1);
    assert!(resp.total_cost_usd >= 0.0);
    assert!(resp.usage.output_tokens > 0);
}
```

- [x] **Step 2: ビルド確認する（実行はしない）**

Run: `cargo test --test e2e --no-run 2>&1`
Expected: コンパイル成功

- [x] **Step 3: E2E テストを実行する**

Run: `cargo test --test e2e -- --ignored 2>&1`
Expected: 1 test passed（実際に Claude CLI が haiku で応答する）

- [x] **Step 4: コミットする**

```bash
git add tests/e2e.rs
git commit -m "test: add E2E test with haiku model"
```

---

### Task 6: 最終確認

- [x] **Step 1: 全ユニットテストを実行する**

Run: `cargo test 2>&1`
Expected: 全テスト passed（E2E は `#[ignore]` で除外）

- [x] **Step 2: clippy + fmt を確認する**

Run: `cargo clippy -- -D warnings 2>&1 && cargo fmt --check 2>&1`
Expected: warning なし

- [x] **Step 3: ドキュメントをビルドする**

Run: `cargo doc --no-deps 2>&1`
Expected: doc comment のあるすべての pub アイテムのドキュメント生成成功
