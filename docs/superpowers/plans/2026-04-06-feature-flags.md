# Feature Flags 実装プラン

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ストリーミング、Structured Output スキーマ生成、tracing を Cargo feature flags でオプショナルにする

**Architecture:** モジュール境界ゲート方式。`#[cfg]` はモジュール宣言と `impl` ブロック単位で適用し、型定義レベルの細粒度 cfg は避ける。`default = ["stream", "structured", "tracing"]` で全部入りデフォルト。

**Tech Stack:** Rust 1.93+, Cargo features, `dep:` 構文

**Spec:** `docs/superpowers/specs/2026-04-06-feature-flags-design.md`

---

## 前提

- ブランチ `feat/structured-output` 上で作業中
- uncommitted changes あり: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/structured.rs`（structured feature の WIP）
- 既存テストが全パス済みの状態で開始すること

---

### Task 1: WIP の structured 変更をコミット

現在の uncommitted changes（`structured` feature の部分実装）を先にコミットし、クリーンな状態にする。

**Files:**
- Stage: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/structured.rs`

- [ ] **Step 1: 現在のテストがパスすることを確認**

Run: `cargo test --all-features`
Expected: All tests pass

- [ ] **Step 2: コミット**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/structured.rs
git commit -m "feat: add generate_schema helper behind structured feature"
```

Note: この変更は既にコミット済みの可能性がある（`git log` に `8003524` として見える）。`git status` で未コミットファイルがあるかを確認し、なければスキップ。

---

### Task 2: StreamEvent を types.rs から stream.rs に移動

`StreamEvent` enum を `stream.rs` に移動し、`types.rs` をコア型のみにする。この時点では feature gate はまだ付けない（コンパイルを壊さないため）。

**Files:**
- Modify: `src/types.rs` — `StreamEvent` enum（42〜143行目）を削除
- Modify: `src/stream.rs` — `StreamEvent` enum を追加、`pub use` 調整
- Modify: `src/lib.rs` — re-export パスを `types::StreamEvent` → `stream::StreamEvent` に変更
- Modify: `src/client.rs:15` — `use crate::types::StreamEvent` → `use crate::stream::StreamEvent`
- Modify: `src/conversation.rs:9` — `use crate::types::StreamEvent` → `use crate::stream::StreamEvent`

- [ ] **Step 1: `StreamEvent` を `stream.rs` に移動**

`src/types.rs` から `StreamEvent` enum（doc comment 含む 42〜143行目）を削除する。

`src/stream.rs` の先頭付近に以下を追加（既存の `use crate::types::{ClaudeResponse, StreamEvent, strip_ansi};` を修正）:

```rust
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
```

- [ ] **Step 2: import パスを修正**

`src/lib.rs` の re-export を変更:

```rust
// Before
pub use types::{ClaudeResponse, StreamEvent, Usage};
// After
pub use stream::StreamEvent;
pub use types::{ClaudeResponse, Usage};
```

`src/client.rs:15` を変更:

```rust
// Before
use crate::types::{ClaudeResponse, StreamEvent, strip_ansi};
// After
use crate::stream::StreamEvent;
use crate::types::{ClaudeResponse, strip_ansi};
```

`src/conversation.rs:9` を変更:

```rust
// Before
use crate::types::{ClaudeResponse, StreamEvent};
// After
use crate::stream::StreamEvent;
use crate::types::ClaudeResponse;
```

- [ ] **Step 3: テスト実行**

Run: `cargo test`
Expected: All tests pass（移動のみ、ロジック変更なし）

- [ ] **Step 4: コミット**

```bash
git add src/types.rs src/stream.rs src/lib.rs src/client.rs src/conversation.rs
git commit -m "refactor: move StreamEvent from types.rs to stream.rs"
```

---

### Task 3: Cargo.toml に feature flags を定義

依存を optional にし、`[features]` セクションを設定する。

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Cargo.toml を書き換え**

`[dependencies]` セクションを以下に変更:

```toml
[dependencies]
tokio = { version = "1", features = ["process", "io-util", "rt-multi-thread", "macros", "time"] }
tokio-stream = { version = "0.1", optional = true }
async-stream = { version = "0.3", optional = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = { version = "0.1", optional = true }
schemars = { version = "1", optional = true }
```

既存の `[features]` セクション（`structured = ["dep:schemars"]` のみ）を以下に書き換え:

```toml
[features]
default = ["stream", "structured", "tracing"]
stream = ["dep:tokio-stream", "dep:async-stream"]
structured = ["dep:schemars"]
tracing = ["dep:tracing"]
```

docs.rs メタデータを追��:

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

examples に `required-features` を追加:

```toml
[[example]]
name = "simple"

[[example]]
name = "stream"
required-features = ["stream"]

[[example]]
name = "stream-all"
required-features = ["stream"]

[[example]]
name = "multi_turn"
```

- [ ] **Step 2: --all-features でビルド確��**

Run: `cargo check --all-features`
Expected: OK（まだ cfg 属性を付けてないので全コードがコンパイルされる）

- [ ] **Step 3: コミット**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: define feature flags in Cargo.toml"
```

---

### Task 4: tracing feature gate を適用

`client.rs` の `tracing::*` 呼び出しを条件付きマクロに置き換える。

**Files:**
- Modify: `src/client.rs`

- [ ] **Step 1: tracing マクロを定義し、呼び出しを置換**

`src/client.rs` の先頭（`use` 文の前）にマクロを追加:

```rust
macro_rules! trace_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)*);
    };
}
macro_rules! trace_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::error!($($arg)*);
    };
}
macro_rules! trace_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)*);
    };
}
```

`use tracing;` 行がもしあれば削除（現在は `tracing::debug!` のように完全修飾で呼んでいるので `use` 行はないはず）。

以下を置換（`client.rs` 内の全7箇所）:

| Before | After |
|---|---|
| `tracing::debug!(args = ?args, "spawning claude CLI stream");` | `trace_debug!(args = ?args, "spawning claude CLI stream");` |
| `tracing::debug!(args = ?args, "executing claude CLI");` | `trace_debug!(args = ?args, "executing claude CLI");` |
| `tracing::error!(error = %err, "claude CLI failed");` (4箇所) | `trace_error!(error = %err, "claude CLI failed");` |
| `tracing::info!("claude CLI returned successfully");` | `trace_info!("claude CLI returned successfully");` |

- [ ] **Step 2: feature ON/OFF 両方で��ルド確認**

Run: `cargo check --all-features && cargo check --no-default-features`
Expected: 両方 OK

- [ ] **Step 3: テスト実行**

Run: `cargo test --all-features`
Expected: All tests pass

- [ ] **Step 4: コミット**

```bash
git add src/client.rs
git commit -m "feat: gate tracing calls behind tracing feature"
```

---

### Task 5: stream feature gate を適用

`stream` モジュール、`ask_stream` メソッド、関連する import を feature gate する。

**Files:**
- Modify: `src/lib.rs`
- Modify: `src/client.rs`
- Modify: `src/conversation.rs`

- [ ] **Step 1: lib.rs に cfg を追加**

```rust
mod client;
mod config;
mod conversation;
mod error;
#[cfg(feature = "stream")]
mod stream;
#[cfg(feature = "structured")]
mod structured;
mod types;

pub use client::{ClaudeClient, CommandRunner, DefaultRunner};
pub use config::{ClaudeConfig, ClaudeConfigBuilder, effort, permission_mode};
pub use conversation::Conversation;
pub use error::ClaudeError;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use stream::StreamEvent;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use tokio_stream::StreamExt;
#[cfg(feature = "structured")]
#[cfg_attr(docsrs, doc(cfg(feature = "structured")))]
pub use structured::generate_schema;
pub use types::{ClaudeResponse, Usage};
```

- [ ] **Step 2: client.rs の stream 関連を cfg gate**

import セクション — stream 専用の use を分離:

```rust
#[cfg(test)]
use mockall::automock;

use std::process::Output;

use tokio::process::Command as TokioCommand;

use crate::config::ClaudeConfig;
use crate::conversation::Conversation;
use crate::error::ClaudeError;
use crate::types::{ClaudeResponse, strip_ansi};

#[cfg(feature = "stream")]
use std::pin::Pin;
#[cfg(feature = "stream")]
use tokio::io::BufReader;
#[cfg(feature = "stream")]
use tokio_stream::Stream;
#[cfg(feature = "stream")]
use crate::stream::{StreamEvent, parse_stream};
```

`impl ClaudeClient` ブロック（`ask_stream` を含むもの）に cfg を追加:

```rust
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
impl ClaudeClient {
    /// Sends a prompt and returns a stream of events.
    /// ... (existing doc comment)
    pub async fn ask_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        // ... (existing implementation, unchanged)
    }
}
```

- [ ] **Step 3: conversation.rs の stream 関連を cfg gate**

import セクション — stream 専用の use を分離:

```rust
use std::sync::{Arc, Mutex};

use crate::client::{ClaudeClient, CommandRunner, DefaultRunner};
use crate::config::{ClaudeConfig, ClaudeConfigBuilder};
use crate::error::ClaudeError;
use crate::types::ClaudeResponse;

#[cfg(feature = "stream")]
use std::pin::Pin;
#[cfg(feature = "stream")]
use tokio_stream::Stream;
#[cfg(feature = "stream")]
use crate::stream::StreamEvent;
```

`wrap_stream` 関数と `impl Conversation`（stream メソッド含む）に cfg を追加:

```rust
/// Wraps a stream to transparently capture `session_id` from
/// [`StreamEvent::SystemInit`] and [`StreamEvent::Result`].
#[cfg(feature = "stream")]
fn wrap_stream(
    inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>,
    session_id: Arc<Mutex<Option<String>>>,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> {
    // ... (existing implementation, unchanged)
}

#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
impl Conversation {
    /// Sends a prompt and returns a stream of events.
    /// ... (existing doc comment)
    pub async fn ask_stream(
        // ... (existing implementation, unchanged)
    }

    /// Sends a prompt with per-turn config overrides and returns a stream.
    /// ... (existing doc comment)
    pub async fn ask_stream_with<F>(
        // ... (existing implementation, unchanged)
    }
}
```

- [ ] **Step 4: conversation.rs のテストに stream 用 cfg を追加**

テストモジュール内で、`StreamEvent` を使うテスト関数に `#[cfg(feature = "stream")]` を追加:

```rust
#[cfg(test)]
mod tests {
    // ... (existing common test code)

    // Non-stream tests stay as-is (session_id_initially_none, ask_captures_session_id, etc.)

    #[cfg(feature = "stream")]
    use crate::types::Usage;
    #[cfg(feature = "stream")]
    use crate::stream::StreamEvent;

    #[cfg(feature = "stream")]
    #[tokio::test]
    async fn wrap_stream_captures_session_id_from_system_init() {
        // ... (existing test, unchanged)
    }

    #[cfg(feature = "stream")]
    #[tokio::test]
    async fn wrap_stream_updates_session_id_from_result() {
        // ... (existing test, unchanged)
    }
}
```

Note: `Usage` の use は `wrap_stream_updates_session_id_from_result` テストで `Usage` struct を直接構築しているため必要。

- [ ] **Step 5: feature ON/OFF 両方でテスト**

Run: `cargo test --all-features && cargo test --no-default-features`
Expected: 両方パス

- [ ] **Step 6: clippy チェック**

Run: `cargo clippy --all-features -- -D warnings && cargo clippy --no-default-features -- -D warnings`
Expected: 警告なし

- [ ] **Step 7: コミット**

```bash
git add src/lib.rs src/client.rs src/conversation.rs
git commit -m "feat: gate stream module and methods behind stream feature"
```

---

### Task 6: structured feature gate の整理

WIP で追加済みの `structured` feature gate が正しいことを確認し、`docsrs` cfg_attr を追加する。

**Files:**
- Modify: `src/lib.rs` — docsrs 属性追加（Task 5 で対応済みのはず）
- Verify: `src/structured.rs` — 変更不要

- [ ] **Step 1: structured feature の動作確認**

Run: `cargo test --features structured`
Expected: `structured` モジュールのテスト（`generate_schema_returns_valid_json`, `generate_schema_contains_properties`）がパス

Run: `cargo test --no-default-features`
Expected: `structured` テストがスキップされ、他のテストがパス

- [ ] **Step 2: コミット（変更があれば）**

Task 5 の lib.rs 変更で `structured` の `docsrs` 対応も含めているため、追加コミット不要の場合はスキップ。

---

### Task 7: CI を更新

feature matrix を test と clippy ジョブに追加し、doc ジョブに `--all-features` を追加する。

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: ci.yml を書き換え**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test (Rust ${{ matrix.rust }}, features ${{ matrix.features }})
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        rust: [stable, "1.93"]
        features: ["", "--no-default-features", "--all-features"]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: ${{ matrix.rust }}
      - uses: Swatinem/rust-cache@v2
      - run: cargo test ${{ matrix.features }}

  clippy:
    name: Clippy (features ${{ matrix.features }})
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        features: ["", "--no-default-features", "--all-features"]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy ${{ matrix.features }} -- -D warnings

  fmt:
    name: Rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --check

  doc:
    name: Documentation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo doc --all-features --no-deps
```

- [ ] **Step 2: コミット**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add feature flag matrix to test and clippy jobs"
```

---

### Task 8: CLAUDE.md を更新

Architecture セクションに feature flags の説明を追記する。

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: CLAUDE.md に feature flags セクションを追記**

`### Architecture` セクションのファイル構成の後に以下を追加:

```markdown
### Feature Flags

```toml
[features]
default = ["stream", "structured", "tracing"]
stream = ["dep:tokio-stream", "dep:async-stream"]  # ask_stream, StreamEvent, Conversation stream methods
structured = ["dep:schemars"]                       # generate_schema helper
tracing = ["dep:tracing"]                           # debug/error/info logging in client.rs
```

- `default-features = false` で最小構成（`ask()` / `ask_structured()` のみ）
- `StreamEvent` は `stream.rs` モジュール内に定義（`stream` feature でゲート）
- tracing ��� `client.rs` 内の条件付きマクロ（`trace_debug!` 等）で吸収
```

- [ ] **Step 2: コミット**

```bash
git add CLAUDE.md
git commit -m "docs: add feature flags section to CLAUDE.md"
```

---

### Task 9: 最終検証

全 feature 組み合わせでのビルド・テスト・lint を実行する。

**Files:** なし（検証のみ）

- [ ] **Step 1: 全パターンでテスト**

```bash
cargo test --all-features
cargo test --no-default-features
cargo test
```

Expected: 全パス

- [ ] **Step 2: 全パターンで clippy**

```bash
cargo clippy --all-features -- -D warnings
cargo clippy --no-default-features -- -D warnings
cargo clippy -- -D warnings
```

Expected: 警告なし

- [ ] **Step 3: フォーマット確認**

Run: `cargo fmt --check`
Expected: OK

- [ ] **Step 4: ドキュメント生成確認**

Run: `cargo doc --all-features --no-deps`
Expected: 警告なし

- [ ] **Step 5: 個別 feature の単体ビルド確認**

```bash
cargo check --no-default-features --features stream
cargo check --no-default-features --features structured
cargo check --no-default-features --features tracing
```

Expected: 全 OK（feature 間に不要な依存がないことの確認）
