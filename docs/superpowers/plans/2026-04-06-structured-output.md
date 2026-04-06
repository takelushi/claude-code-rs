# Structured Output ヘルパー実装プラン

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** CLI レスポンスの `result` 文字列を任意の型 `T` に型安全にデシリアライズする API と、`schemars` によるスキーマ自動生成ヘルパーを提供する。

**Architecture:** コア機能（`parse_result`, `ask_structured`）は feature flag なしで常に利用可能。`schemars` 依存の `generate_schema` のみ `structured` feature でゲート。エラー型は既存の `ParseError` と分離した `StructuredOutputError` を追加。

**Tech Stack:** Rust 1.93+, serde, serde_json, schemars 1.x (optional), thiserror, mockall

**設計ドキュメント:** `docs/superpowers/specs/2026-04-06-structured-output-design.md`

---

## ファイル構成

| ファイル | 操作 | 責務 |
|---------|------|------|
| `src/error.rs` | 変更 | `StructuredOutputError` バリアント追加 |
| `tests/fixtures/structured_success.json` | 新規 | structured output 用テストフィクスチャ |
| `src/types.rs` | 変更 | `ClaudeResponse::parse_result::<T>()` 追加 |
| `src/client.rs` | 変更 | `ClaudeClient::ask_structured::<T>()` 追加 |
| `Cargo.toml` | 変更 | `structured` feature + `schemars` optional dep |
| `src/structured.rs` | 新規 | `generate_schema::<T>()` |
| `src/lib.rs` | 変更 | `mod structured` + `pub use` 追加 |
| `CLAUDE.md` | 変更 | Architecture セクション更新 |

---

### Task 1: `StructuredOutputError` エラーバリアント追加

**Files:**
- Modify: `src/error.rs`

- [ ] **Step 1: テスト（Red）を書く**

`src/error.rs` の `#[cfg(test)] mod tests` ブロック末尾に以下を追加:

```rust
    #[test]
    fn structured_output_error_message() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = ClaudeError::StructuredOutputError {
            raw_result: "raw text here".into(),
            source: serde_err,
        };
        assert!(err.to_string().starts_with("failed to deserialize structured output:"));
    }

    #[test]
    fn structured_output_error_preserves_raw_result() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = ClaudeError::StructuredOutputError {
            raw_result: "the raw output".into(),
            source: serde_err,
        };
        match err {
            ClaudeError::StructuredOutputError { raw_result, .. } => {
                assert_eq!(raw_result, "the raw output");
            }
            _ => panic!("expected StructuredOutputError"),
        }
    }
```

- [ ] **Step 2: テスト失敗を確認**

Run: `cargo test --lib error::tests::structured_output_error -- 2>&1`
Expected: コンパイルエラー（`StructuredOutputError` が未定義）

- [ ] **Step 3: 実装（Green）**

`src/error.rs` の `ClaudeError` enum の `Io` バリアントの後に追加:

```rust
    /// CLI succeeded but the `result` field could not be deserialized
    /// into the target type.
    #[error("failed to deserialize structured output: {source}")]
    StructuredOutputError {
        /// Raw result string from CLI.
        raw_result: String,
        /// Deserialization error.
        source: serde_json::Error,
    },
```

- [ ] **Step 4: テスト通過を確認**

Run: `cargo test --lib error::tests::structured_output_error`
Expected: 2 tests PASS

- [ ] **Step 5: clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: warnings なし

- [ ] **Step 6: コミット**

```bash
git add src/error.rs
git commit -m "feat: add StructuredOutputError variant to ClaudeError"
```

---

### Task 2: `ClaudeResponse::parse_result::<T>()` 追加

**Files:**
- Create: `tests/fixtures/structured_success.json`
- Modify: `src/types.rs`

- [ ] **Step 1: テストフィクスチャを作成**

`tests/fixtures/structured_success.json`:

```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "duration_ms": 3000,
  "duration_api_ms": 2950,
  "num_turns": 1,
  "result": "{\"value\":42}",
  "stop_reason": "end_turn",
  "session_id": "test-session-structured",
  "total_cost_usd": 0.01,
  "usage": {
    "input_tokens": 15,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0,
    "output_tokens": 10,
    "server_tool_use": {
      "web_search_requests": 0,
      "web_fetch_requests": 0
    }
  }
}
```

- [ ] **Step 2: テスト（Red）を書く**

`src/types.rs` の `#[cfg(test)] mod tests` ブロック末尾に以下を追加:

```rust
    #[derive(Debug, Deserialize, PartialEq)]
    struct Answer {
        value: i32,
    }

    #[test]
    fn parse_result_success() {
        let json = include_str!("../tests/fixtures/structured_success.json");
        let resp: ClaudeResponse = serde_json::from_str(json).unwrap();
        let answer: Answer = resp.parse_result().unwrap();
        assert_eq!(answer, Answer { value: 42 });
    }

    #[test]
    fn parse_result_invalid_json() {
        let resp = ClaudeResponse {
            result: "not valid json".into(),
            is_error: false,
            duration_ms: 0,
            num_turns: 0,
            session_id: String::new(),
            total_cost_usd: 0.0,
            stop_reason: String::new(),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        };
        let err = resp.parse_result::<Answer>().unwrap_err();
        match err {
            crate::error::ClaudeError::StructuredOutputError { raw_result, .. } => {
                assert_eq!(raw_result, "not valid json");
            }
            _ => panic!("expected StructuredOutputError, got {err:?}"),
        }
    }

    #[test]
    fn parse_result_type_mismatch() {
        let resp = ClaudeResponse {
            result: r#"{"wrong_field": "hello"}"#.into(),
            is_error: false,
            duration_ms: 0,
            num_turns: 0,
            session_id: String::new(),
            total_cost_usd: 0.0,
            stop_reason: String::new(),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        };
        let err = resp.parse_result::<Answer>().unwrap_err();
        assert!(matches!(
            err,
            crate::error::ClaudeError::StructuredOutputError { .. }
        ));
    }
```

- [ ] **Step 3: テスト失敗を確認**

Run: `cargo test --lib types::tests::parse_result 2>&1`
Expected: コンパイルエラー（`parse_result` メソッドが未定義）

- [ ] **Step 4: 実装（Green）**

`src/types.rs` の `impl` ブロックが既存にないので、`strip_ansi` 関数の前に追加:

```rust
use serde::de::DeserializeOwned;

use crate::error::ClaudeError;

impl ClaudeResponse {
    /// Deserializes the `result` field into a strongly-typed value.
    ///
    /// Works with both streaming and non-streaming responses.
    /// The config must have `json_schema` set for the CLI to return
    /// structured JSON in the `result` field.
    pub fn parse_result<T: DeserializeOwned>(&self) -> Result<T, ClaudeError> {
        serde_json::from_str(&self.result).map_err(|e| ClaudeError::StructuredOutputError {
            raw_result: self.result.clone(),
            source: e,
        })
    }
}
```

注意: ファイル先頭の `use serde::Deserialize;` は既存。`DeserializeOwned` は `serde::de::DeserializeOwned` から別途 import が必要。

- [ ] **Step 5: テスト通過を確認**

Run: `cargo test --lib types::tests::parse_result`
Expected: 3 tests PASS

- [ ] **Step 6: clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: warnings なし

- [ ] **Step 7: コミット**

```bash
git add tests/fixtures/structured_success.json src/types.rs
git commit -m "feat: add ClaudeResponse::parse_result for typed deserialization"
```

---

### Task 3: `ClaudeClient::ask_structured::<T>()` 追加

**Files:**
- Modify: `src/client.rs`

- [ ] **Step 1: テスト（Red）を書く**

`src/client.rs` の `#[cfg(test)] mod tests` ブロック末尾に以下を追加:

```rust
    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct TestAnswer {
        value: i32,
    }

    fn structured_success_output() -> Output {
        Output {
            status: ExitStatus::from_raw(0),
            stdout: include_bytes!("../tests/fixtures/structured_success.json").to_vec(),
            stderr: Vec::new(),
        }
    }

    #[tokio::test]
    async fn ask_structured_success() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| Ok(structured_success_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let answer: TestAnswer = client.ask_structured("What is 6*7?").await.unwrap();
        assert_eq!(answer, TestAnswer { value: 42 });
    }

    #[tokio::test]
    async fn ask_structured_deserialization_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| Ok(success_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client
            .ask_structured::<TestAnswer>("hello")
            .await
            .unwrap_err();
        assert!(matches!(err, ClaudeError::StructuredOutputError { .. }));
    }

    #[tokio::test]
    async fn ask_structured_cli_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| Ok(non_zero_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client
            .ask_structured::<TestAnswer>("hello")
            .await
            .unwrap_err();
        assert!(matches!(err, ClaudeError::NonZeroExit { code: 1, .. }));
    }
```

- [ ] **Step 2: テスト失敗を確認**

Run: `cargo test --lib client::tests::ask_structured 2>&1`
Expected: コンパイルエラー（`ask_structured` メソッドが未定義）

- [ ] **Step 3: 実装（Green）**

`src/client.rs` の `impl<R: CommandRunner> ClaudeClient<R>` ブロック（`ask` メソッドがあるブロック）の末尾（`}` の前）に追加:

```rust
    /// Sends a prompt and deserializes the result into `T`.
    ///
    /// Requires `json_schema` to be set on the config beforehand.
    /// Use [`generate_schema`](crate::generate_schema) to auto-generate it
    /// (requires the `structured` feature).
    pub async fn ask_structured<T: serde::de::DeserializeOwned>(
        &self,
        prompt: &str,
    ) -> Result<T, ClaudeError> {
        let response = self.ask(prompt).await?;
        response.parse_result()
    }
```

- [ ] **Step 4: テスト通過を確認**

Run: `cargo test --lib client::tests::ask_structured`
Expected: 3 tests PASS

- [ ] **Step 5: clippy + fmt**

Run: `cargo clippy -- -D warnings && cargo fmt`
Expected: warnings なし

- [ ] **Step 6: コミット**

```bash
git add src/client.rs
git commit -m "feat: add ClaudeClient::ask_structured for typed responses"
```

---

### Task 4: `structured` feature flag + `generate_schema::<T>()`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/structured.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Cargo.toml に feature と依存を追加**

`Cargo.toml` の `[dependencies]` セクション末尾に追加:

```toml
schemars = { version = "1", optional = true }
```

`[dev-dependencies]` セクションの後に追加:

```toml
[features]
structured = ["dep:schemars"]
```

- [ ] **Step 2: テスト（Red）を書く**

`src/structured.rs` を新規作成:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(schemars::JsonSchema)]
    struct TestStruct {
        name: String,
        count: i32,
    }

    #[test]
    fn generate_schema_returns_valid_json() {
        let schema_str = generate_schema::<TestStruct>().unwrap();
        let value: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
        assert!(value.is_object());
    }

    #[test]
    fn generate_schema_contains_properties() {
        let schema_str = generate_schema::<TestStruct>().unwrap();
        let value: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
        let props = value.get("properties").expect("should have properties");
        assert!(props.get("name").is_some());
        assert!(props.get("count").is_some());
    }
}
```

- [ ] **Step 3: テスト失敗を確認**

Run: `cargo test --lib --features structured structured::tests 2>&1`
Expected: コンパイルエラー（`generate_schema` が未定義）

- [ ] **Step 4: 実装（Green）**

`src/structured.rs` のテストブロックの前に追加:

```rust
use schemars::{JsonSchema, schema_for};

use crate::error::ClaudeError;

/// Generates a JSON Schema string from a type implementing [`JsonSchema`].
///
/// Use the result with [`ClaudeConfigBuilder::json_schema`](crate::ClaudeConfigBuilder::json_schema)
/// to enable structured output from the CLI.
///
/// # Errors
///
/// Returns [`ClaudeError::ParseError`] if schema serialization fails.
pub fn generate_schema<T: JsonSchema>() -> Result<String, ClaudeError> {
    let schema = schema_for!(T);
    serde_json::to_string(&schema).map_err(ClaudeError::from)
}
```

- [ ] **Step 5: `src/lib.rs` にモジュールとエクスポートを追加**

`src/lib.rs` の `mod types;` の後に追加:

```rust
#[cfg(feature = "structured")]
mod structured;
```

`pub use types::{...};` の後に追加:

```rust
#[cfg(feature = "structured")]
pub use structured::generate_schema;
```

- [ ] **Step 6: テスト通過を確認**

Run: `cargo test --lib --features structured structured::tests`
Expected: 2 tests PASS

- [ ] **Step 7: feature なしでもビルドできることを確認**

Run: `cargo test --lib`
Expected: 全テスト PASS（`structured` テストはスキップされる）

- [ ] **Step 8: clippy + fmt**

Run: `cargo clippy --features structured -- -D warnings && cargo fmt`
Expected: warnings なし

- [ ] **Step 9: コミット**

```bash
git add Cargo.toml src/structured.rs src/lib.rs
git commit -m "feat: add generate_schema helper behind structured feature"
```

---

### Task 5: ドキュメント更新

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: CLAUDE.md の Architecture セクションを更新**

Architecture の `src/` ツリーに `structured.rs` を追加:

```plain
src/
  lib.rs           # pub API re-export
  client.rs        # ClaudeClient (CLI実行の中核)
  config.rs        # ClaudeConfig (--model, --system-prompt 等のオプション)
  conversation.rs  # Conversation (session_id 自動管理の複数ターン会話)
  types.rs         # JSON/stream-json 両方の型定義のみ
  error.rs         # エラー型
  stream.rs        # stream-json のパース・イテレーション・バッファリング
  structured.rs    # generate_schema (schemars feature gated)
```

- [ ] **Step 2: Error Variants セクションに `StructuredOutputError` を追加**

既存の `Io` の後に追加:

```
- `StructuredOutputError { raw_result, source }` — CLI は成功したが result の JSON デシリアライズに失敗した
```

- [ ] **Step 3: コミット**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with structured output architecture"
```

---

## 実行順序のまとめ

| Task | 依存 | 概要 |
|------|------|------|
| 1 | なし | `StructuredOutputError` エラーバリアント |
| 2 | Task 1 | `ClaudeResponse::parse_result::<T>()` |
| 3 | Task 2 | `ClaudeClient::ask_structured::<T>()` |
| 4 | Task 1 | `structured` feature + `generate_schema::<T>()` |
| 5 | Task 1-4 | ドキュメント更新 |

Task 2 と Task 4 は Task 1 完了後に並列実行可能。Task 3 は Task 2 に依存。
