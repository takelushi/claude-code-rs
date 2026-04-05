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

`CommandRunner` trait には変更を加えない。`ask_stream()` 内で直接 `tokio::process::Command::spawn()` を呼ぶ。

`Child` のモックは困難なため、テスト戦略を分離する:
- **stream.rs のパースロジック**: `impl AsyncBufRead` を受け取る関数として実装し、テストでは `Cursor<Vec<u8>>` を渡す
- **ask_stream() の結合**: E2E テストでカバー

## 引数生成

`ClaudeConfig` に `to_stream_args()` メソッドを追加。`to_args()` と共通部分が多いため、内部ヘルパーで重複を排除する:

```rust
impl ClaudeConfig {
    /// 共通の固定フラグ + model + max_turns + system_prompt を組み立てる。
    fn base_args(&self) -> Vec<String> { ... }

    /// JSON 用引数（既存）。base_args + --output-format json + prompt。
    pub fn to_args(&self, prompt: &str) -> Vec<String> { ... }

    /// stream-json 用引数。base_args + --output-format stream-json + --verbose + prompt。
    pub fn to_stream_args(&self, prompt: &str) -> Vec<String> { ... }
}
```

`to_stream_args()` が `to_args()` と異なる点:
- `--output-format stream-json`（`json` ではなく）
- `--verbose`（stream-json に必須）
- `--include-partial-messages`（`include_partial_messages == Some(true)` の場合）

共通の固定フラグは `base_args()` に集約: `--print`, `--no-session-persistence`, `--setting-sources ''`, `--strict-mcp-config`, `--mcp-config`, `--tools ''`, `--disable-slash-commands`, `--system-prompt`, `--model`（任意）, `--max-turns`（任意）。

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

### content 配列に複数要素がある場合

1つの `assistant` イベントの `.message.content[]` に複数の content ブロックが含まれる場合がある（例: thinking + text が1イベントに混在）。この場合、配列の各要素を個別に処理し、それぞれ対応する `StreamEvent` を yield する。つまり1つの NDJSON 行から複数の `StreamEvent` が生成される可能性がある。

## 依存追加

`Cargo.toml` に追加:

```toml
[dependencies]
tokio-stream = "0.1"
async-stream = "0.3"
```

## タイムアウト

ストリームに対するタイムアウトはライブラリ側では提供しない。`config.timeout` は `ask()` にのみ適用される。

ストリームのタイムアウト戦略はユースケースによって異なる（全体タイムアウト vs イベント間タイムアウト等）ため、利用者が `tokio_stream::StreamExt::timeout()` 等で自前対応する。

## テスト戦略

### ユニットテスト (stream.rs)

- パース関数は `impl AsyncBufRead` を受け取る設計にし、テストでは `Cursor<Vec<u8>>` を渡す
- fixture: CLI 出力を模した NDJSON テキストファイル
- 各イベント型のパースが正しいことを検証
- ANSI エスケープ混入行がスキップまたはストリップされることを検証
- 空行・不正行がスキップされることを検証
- content 配列に複数要素があるケースのテスト
- 完全なストリームシーケンスをテスト: init → thinking → text → result

### ユニットテスト (client.rs)

- `ask_stream()` は `Child` プロセスに依存するため、ユニットテストでの直接テストは行わない
- E2E テストでカバーする

### ユニットテスト (config.rs)

- `to_stream_args()` に `--verbose` と `stream-json` が含まれることを検証
- `--include-partial-messages` フラグの有無を検証
- `include_partial_messages` 付きの Builder をテスト
- `base_args()` リファクタ後も既存の `to_args()` テストがパスすることを確認

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
| `src/client.rs` | `ClaudeClient` に `ask_stream()` 追加 |
| `src/lib.rs` | `StreamEvent` をエクスポート |
| `tests/fixtures/stream_success.ndjson` | ストリーム用 fixture |
| `tests/e2e.rs` | ストリーム E2E テストを追加 |
| `examples/stream.rs` | ストリーミング使用例 |
| `CLAUDE.md` | Architecture, Commands を更新 |
| `docs/claude-cli.md` | stream-json の詳細を記録 |
