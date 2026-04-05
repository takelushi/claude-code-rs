# stream-json 対応 設計

## 目的

claude-code-rs にストリーミング対応を追加する。Claude Code CLI を `--output-format stream-json --verbose` で実行し、リアルタイムイベントを `tokio_stream::Stream` で公開する。

## 設計判断

| 項目 | 選択 | 理由 |
| --- | --- | --- |
| スコープ | テキスト + 最終結果 + 全イベント型 | ストリームの全イベントを可能な限り公開する |
| API スタイル | `Stream<Item = Result<StreamEvent, ClaudeError>>` | Rust async のイディオムに沿い、`StreamExt` と相性が良い |
| イベント粒度 | 7バリアント (SystemInit, Thinking, Text, ToolUse, ToolResult, RateLimit, Result) | CLI が出力する意味のあるイベントをすべて公開 |
| `--include-partial-messages` | `ClaudeConfig` でオプション指定 | ストリームの粒度はユースケース次第 |
| パースエラー処理 | パースできない行はスキップ | ANSI エスケープの混入は既知の問題。毎回エラーにするのはノイジー |
| 依存追加 | `tokio-stream`, `async-stream` | 軽量で tokio エコシステムの標準 |

## 公開 API

### `ClaudeClient::ask_stream()`

```rust
impl<R: CommandRunner> ClaudeClient<R> {
    /// Sends a prompt and returns a stream of events.
    pub async fn ask_stream(&self, prompt: &str)
        -> Result<impl Stream<Item = Result<StreamEvent, ClaudeError>>, ClaudeError>;
}
```

- 外側の `Result`: プロセス起動エラー (CliNotFound, Io)
- Stream 内の `Result`: ストリーム中のエラー (プロセス異常終了 → NonZeroExit)
- パースできない行はスキップ
- `Result` イベント到達またはプロセス終了で Stream 終了

### `StreamEvent`

```rust
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum StreamEvent {
    /// Session initialization info.
    SystemInit {
        session_id: String,
        model: String,
    },
    /// Model's thinking process (extended thinking).
    Thinking(String),
    /// Text response chunk.
    Text(String),
    /// Tool invocation by the model.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool execution result.
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    /// Rate limit information.
    RateLimit {
        resets_at: u64,
    },
    /// Final result (same structure as non-streaming response).
    Result(ClaudeResponse),
}
```

## Config 変更

`ClaudeConfig` / `ClaudeConfigBuilder` にフィールドを1つ追加:

```rust
pub struct ClaudeConfig {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub timeout: Option<Duration>,
    pub include_partial_messages: Option<bool>,  // 追加
}
```

Builder に `include_partial_messages(bool)` メソッドを追加。

## CommandRunner 変更

ストリーミング用に `spawn()` メソッドを追加（完了済み `Output` ではなくライブな `Child` プロセスを返す）:

```rust
pub trait CommandRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;
    async fn spawn(&self, args: &[String]) -> std::io::Result<Child>;  // 追加
}
```

`DefaultRunner::spawn()` の実装:

```rust
async fn spawn(&self, args: &[String]) -> std::io::Result<Child> {
    Command::new("claude")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}
```

注: `spawn()` は本質的に同期的（プロセス起動のみ）だが、trait の一貫性とモック容易性のため async にする。

## 引数生成

`ClaudeConfig` に `to_stream_args()` メソッドを追加:

```rust
impl ClaudeConfig {
    pub fn to_stream_args(&self, prompt: &str) -> Vec<String>;
}
```

`to_args()` との差分:
- `--output-format stream-json`（`json` ではなく）
- `--verbose`（stream-json に必須）
- `--include-partial-messages`（`include_partial_messages == Some(true)` の場合）

共通の固定フラグはそのまま: `--print`, `--no-session-persistence`, `--setting-sources ''`, `--strict-mcp-config`, `--mcp-config`, `--tools ''`, `--disable-slash-commands`, `--system-prompt`。

## stream.rs の責務

CLI の stdout から NDJSON を行単位でパースし `StreamEvent` に変換する:

1. `tokio::io::BufReader` + `lines()` で stdout を1行ずつ読み取り
2. 各行に `strip_ansi()` を適用
3. `serde_json::from_str::<serde_json::Value>()` で JSON パース
4. `type` フィールドで分岐し、適切な `StreamEvent` バリアントに変換
5. パース失敗行はスキップ（ANSI のみの行、空行等）
6. プロセス終了時に終了コードを確認し、非ゼロなら `ClaudeError::NonZeroExit` を yield

### イベントパースロジック

```
match json["type"].as_str() {
    "system"           → subtype "init" を抽出 → SystemInit
    "assistant"        → content[].type を確認:
                           "thinking"  → Thinking
                           "text"      → Text
                           "tool_use"  → ToolUse
    "user"             → content[].type を確認:
                           "tool_result" → ToolResult
    "rate_limit_event" → RateLimit
    "result"           → ClaudeResponse としてデシリアライズ → Result
    _                  → スキップ
}
```

`assistant` イベントの場合: `.message.content[]` 配列の最後の要素でイベント種別を判定する。

## 依存追加

`Cargo.toml` に追加:

```toml
[dependencies]
tokio-stream = "0.1"
async-stream = "0.3"
```

## テスト戦略

### ユニットテスト (stream.rs)

- fixture: CLI 出力を模した NDJSON テキストファイル
- 各イベント型のパースが正しいことを検証
- ANSI エスケープ混入行がスキップまたはストリップされることを検証
- 空行・不正行がスキップされることを検証
- 完全なストリームシーケンスをテスト: init → thinking → text → result

### ユニットテスト (client.rs)

- `CommandRunner::spawn()` をモックし、事前定義した stdout を持つ偽の `Child` を返す
- `ask_stream()` が正しいシーケンスの `StreamEvent` を返すことを検証
- プロセス起動エラーが正しい `ClaudeError` にマップされることを検証
- 部分ストリーム後の非ゼロ終了がエラーを yield することを検証

### ユニットテスト (config.rs)

- `to_stream_args()` に `--verbose` と `stream-json` が含まれることを検証
- `--include-partial-messages` フラグの有無を検証
- `include_partial_messages` 付きの Builder をテスト

### E2E テスト

- `e2e_ask_stream_with_haiku` (`#[ignore]`)
- ストリームが少なくとも1つの `Text` イベントを yield し、`Result` で終了することを検証

### Example

- `examples/stream.rs`: `StreamExt::next()` を使った最小限のストリーミング例

## ファイル変更一覧

| ファイル | 変更内容 |
| --- | --- |
| `Cargo.toml` | `tokio-stream`, `async-stream` を追加 |
| `src/stream.rs` | NDJSON パース → `StreamEvent` 変換を実装 |
| `src/types.rs` | `StreamEvent` enum を追加 |
| `src/config.rs` | `include_partial_messages`, `to_stream_args()` を追加 |
| `src/client.rs` | `CommandRunner` に `spawn()` 追加、`ClaudeClient` に `ask_stream()` 追加 |
| `src/lib.rs` | `StreamEvent` をエクスポート |
| `tests/fixtures/stream_success.ndjson` | ストリーム用 fixture |
| `tests/e2e.rs` | ストリーム E2E テストを追加 |
| `examples/stream.rs` | ストリーミング使用例 |
| `CLAUDE.md` | Architecture, Commands を更新 |
| `docs/claude-cli.md` | stream-json の詳細を記録 |
