# Feature Flags 設計

## 概要

claude-code-rs の optional 機能（ストリーミング、Structured Output スキーマ生成、tracing）を Cargo feature flags で制御し、不要な依存を避けられるようにする。

## 背景

現在すべての依存が無条件にコンパイルされるため、コア機能（`ask()` のみ）だけ使いたいユーザーも `tokio-stream`、`async-stream`、`schemars`、`tracing` をビルドしなければならない。feature flags でこれらをオプショナルにする。

## 設計方針

- **モジュール境界ゲート** — `#[cfg]` はモジュール宣言と `impl` ブロック単位で適用。型定義・enum variant レベルの細粒度 cfg は避ける
- **全部入りデフォルト** — `default = ["stream", "structured", "tracing"]`。最小構成は `default-features = false` で指定
- **コア機能は常に有効** — `ask()`, `ask_structured()`, `parse_result()`, `Conversation::ask()` は feature gate なし

## Feature 定義

### `[features]` セクション

```toml
[features]
default = ["stream", "structured", "tracing"]
stream = ["dep:tokio-stream", "dep:async-stream"]
structured = ["dep:schemars"]
tracing = ["dep:tracing"]
```

### 各 feature のスコープ

| Feature | 依存 crate | ゲートされるもの |
|---|---|---|
| `stream` | `tokio-stream`, `async-stream` | `stream` モジュール（`StreamEvent` 型 + パーサー）、`ClaudeClient::ask_stream()`、`Conversation::ask_stream()` / `ask_stream_with()` / `wrap_stream()`、`StreamExt` re-export |
| `structured` | `schemars` | `structured` モジュール（`generate_schema()` のみ） |
| `tracing` | `tracing` | `client.rs` 内のログ出力（マクロ経由） |

### コア（常に有効）

- `tokio`, `serde`, `serde_json`, `thiserror`
- `client.rs`: `ask()`, `ask_structured()`, `with_runner()`, `new()`
- `config.rs`: 全体
- `types.rs`: `ClaudeResponse`, `Usage`, `strip_ansi()`
- `error.rs`: 全バリアント（`StructuredOutputError` 含む）
- `conversation.rs`: `ask()`, `ask_with()`, `session_id()`, `conversation()`, `conversation_resume()`

## 詳細設計

### Cargo.toml

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

### lib.rs

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
pub use stream::StreamEvent;
#[cfg(feature = "stream")]
pub use tokio_stream::StreamExt;
#[cfg(feature = "structured")]
pub use structured::generate_schema;
pub use types::{ClaudeResponse, Usage};
```

### docs.rs 対応

feature-gated なアイテムに `#[cfg_attr(docsrs, doc(cfg(...)))]` を付与し、docs.rs 上でどの feature が必要か表示する。

```rust
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use stream::StreamEvent;
```

`Cargo.toml` に docs.rs 用のメタデータを追加：

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

対象：`StreamEvent`, `StreamExt`, `generate_schema`, feature-gated な `impl` ブロック内のメソッド（`ask_stream` 等）。

### StreamEvent の移動

`types.rs` から `stream.rs` に `StreamEvent` enum を移動する。`stream.rs` は `StreamEvent` 定義 + パーサー（`parse_event`, `parse_stream`）をまとめて持つ。

`types.rs` には `ClaudeResponse`, `Usage`, `strip_ansi()` のみ残す。

### client.rs の impl ブロック分離

```rust
// 常に有効
impl ClaudeClient {
    pub fn new(config: ClaudeConfig) -> Self { ... }
}

// stream feature
#[cfg(feature = "stream")]
impl ClaudeClient {
    pub async fn ask_stream(&self, prompt: &str) -> Result<...> { ... }
}

// 常に有効
impl<R: CommandRunner> ClaudeClient<R> {
    pub fn with_runner(config: ClaudeConfig, runner: R) -> Self { ... }
    pub async fn ask(&self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> { ... }
    pub async fn ask_structured<T>(&self, prompt: &str) -> Result<T, ClaudeError> { ... }
}

// 常に有効
impl<R: CommandRunner + Clone> ClaudeClient<R> {
    pub fn conversation(&self) -> Conversation<R> { ... }
    pub fn conversation_resume(&self, session_id: impl Into<String>) -> Conversation<R> { ... }
}
```

### tracing のマクロ吸収

`client.rs` 冒頭に条件付きマクロを定義し、`tracing::*` 呼び出しを置き換え：

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

### conversation.rs の cfg 適用

```rust
// 常に有効
impl<R: CommandRunner> Conversation<R> {
    pub fn session_id(&self) -> Option<String> { ... }
}

impl<R: CommandRunner + Clone> Conversation<R> {
    pub(crate) fn with_runner(...) -> Self { ... }
    pub(crate) fn with_runner_resume(...) -> Self { ... }
    pub async fn ask(&mut self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> { ... }
    pub async fn ask_with<F>(...) -> Result<ClaudeResponse, ClaudeError> { ... }
}

// stream feature
#[cfg(feature = "stream")]
impl Conversation {
    pub async fn ask_stream(...) -> Result<...> { ... }
    pub async fn ask_stream_with<F>(...) -> Result<...> { ... }
}

#[cfg(feature = "stream")]
fn wrap_stream(...) -> ... { ... }
```

stream 用の `use` 文も `#[cfg(feature = "stream")]` でゲート。

### examples

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

### CI

test と clippy ジョブに feature matrix を追加：

```yaml
strategy:
  matrix:
    features: ["", "--no-default-features", "--all-features"]
```

`fmt` は feature 無関係のため変更なし。`doc` ジョブは `cargo doc --all-features --no-deps` に変更し、全 feature-gated アイテムのドキュメントが生成されることを確認する。

## ユーザーへの影響

```toml
# 従来通り（全機能）
claude-code-rs = "0.1"

# 最小構成（ask() のみ）
claude-code-rs = { version = "0.1", default-features = false }

# ストリーミングだけ
claude-code-rs = { version = "0.1", default-features = false, features = ["stream"] }
```

デフォルトが全部入りのため、既存ユーザーへの破壊的変更はない。

## テスト方針

- 既存のユニットテストは feature gate の内側に配置（各モジュールの `#[cfg(test)]` はそのまま）
- CI で `--no-default-features` / デフォルト / `--all-features` の3パターンをテスト
- `--no-default-features` で `cargo check` が通ることを確認
