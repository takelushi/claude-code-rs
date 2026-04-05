# Minimal Prototype Design

## Overview

Claude Code CLI (`claude --print --output-format json`) をサブプロセスとして実行し、結果を型安全に扱う最小プロトタイプ。

## Scope

### In Scope

- JSON 単発出力 (`--output-format json`) のパースと型定義
- `ClaudeConfig` Builder: `model`, `system_prompt`, `max_turns` のみ可変
- 固定付与オプション（コンテキスト最小化構成）
- エラー型 5 バリアント: `CliNotFound`, `NonZeroExit`, `ParseError`, `Timeout`, `Io`
- `CommandRunner` trait によるプロセス実行の抽象化
- モック + fixture によるユニットテスト

### Out of Scope

- stream-json (`--output-format stream-json`) 対応 → 次イテレーション
- `stream.rs` の実装 → 次イテレーション
- `--allowed-tools`, `--max-budget-usd`, `--permission-mode` 等の追加オプション
- 同期 API

## Architecture

```
src/
  lib.rs        # pub API re-export
  error.rs      # ClaudeError (5 variants)
  types.rs      # ClaudeResponse, Usage
  config.rs     # ClaudeConfig + Builder
  client.rs     # CommandRunner trait, ClaudeClient
  stream.rs     # (空 — 次イテレーション)
```

### Data Flow

```
User
  │
  ▼
ClaudeClient::ask("prompt")
  │
  ├─ ClaudeConfig → コマンド引数組み立て
  │
  ├─ tokio::time::timeout で包む
  │    │
  │    ▼
  │  CommandRunner::run(args)
  │    │
  │    ▼
  │  claude --print --output-format json
  │    --no-session-persistence
  │    --system-prompt '' --setting-sources ''
  │    --strict-mcp-config --mcp-config '{"mcpServers":{}}'
  │    --tools '' --disable-slash-commands
  │    [--model ...] [--max-turns ...] "prompt"
  │    │
  │    ▼
  │  Output { stdout, stderr, exit_code }
  │
  ├─ exit code チェック → NonZeroExit
  │    └─ ErrorKind::NotFound → CliNotFound
  │
  ├─ serde_json::from_str → ClaudeResponse
  │
  ▼
Result<ClaudeResponse, ClaudeError>
```

## Module Details

### `error.rs`

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClaudeError {
    #[error("claude CLI not found in PATH")]
    CliNotFound,

    #[error("claude exited with code {code}: {stderr}")]
    NonZeroExit { code: i32, stderr: String },

    #[error("failed to parse response")]
    ParseError(#[from] serde_json::Error),

    #[error("request timed out")]
    Timeout,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

### `types.rs`

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct ClaudeResponse {
    pub result: String,
    pub is_error: bool,
    pub duration_ms: u64,
    pub num_turns: u32,
    pub session_id: String,
    pub total_cost_usd: f64,
    pub stop_reason: String,
    pub usage: Usage,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}
```

- 未知フィールドは無視（`deny_unknown_fields` を付けない）
- CLI 出力の主要フィールドのみ抽出
- stdout に ANSI エスケープシーケンスが混入する場合があるため、JSON パース前にストリップする

### `config.rs`

```rust
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfig {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub timeout: Option<Duration>,
}
```

- `ClaudeConfig::builder()` で `ClaudeConfigBuilder` を返す
- Builder は各フィールドの setter + `build()` を持つ
- 以下のオプションは `ClaudeClient` が固定付与（コンテキスト最小化）:
  - `--print` — 非対話モード
  - `--output-format json` — JSON 出力
  - `--no-session-persistence` — セッション保存しない
  - `--system-prompt` — `config.system_prompt` が `Some` なら指定値、`None` なら `''`
  - `--setting-sources ''` — 設定ファイル読み込みスキップ
  - `--strict-mcp-config` — 外部 MCP 設定を無視
  - `--mcp-config '{"mcpServers":{}}'` — MCP サーバーをゼロに
  - `--tools ''` — ビルトインツール定義を除外（約9000トークン削減）
  - `--disable-slash-commands` — スキル無効化

### `client.rs`

```rust
pub trait CommandRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;
}

pub struct ClaudeClient<R: CommandRunner = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
}
```

- `ClaudeClient::new(config)` — `DefaultRunner` で構築
- `ClaudeClient::with_runner(config, runner)` — テスト用モック注入
- `async fn ask(&self, prompt: &str) -> Result<ClaudeResponse, ClaudeError>`

#### `DefaultRunner`

`tokio::process::Command` で `claude` を実行し、`Output` を返す。

#### CLI 存在チェック

事前の `which` チェックは行わない。`Command::output()` の `io::ErrorKind::NotFound` を捕捉して `ClaudeError::CliNotFound` に変換する。

#### Timeout

`ask()` 全体を `tokio::time::timeout` で包む。`config.timeout` が `None` の場合はタイムアウトなし。

## Testing Strategy

### Unit Tests

- **`error.rs`**: エラーメッセージ・From 変換のテスト
- **`types.rs`**: fixture JSON のデシリアライズテスト
- **`config.rs`**: Builder のデフォルト値・各 setter・引数組み立てのテスト
- **`client.rs`**: `CommandRunner` をモックし、正常系・異常系をテスト

### Fixtures

- `tests/fixtures/success.json` — 正常な CLI 出力
- `tests/fixtures/error.json` — `is_error: true` の出力

### E2E Tests

- `#[ignore]` 付与、`--model haiku` で課金最小化
- `cargo test -- --ignored` で明示的に実行

## Known Concerns

### `mockall` と async fn in trait の互換性

`mockall` 0.13 はネイティブ async fn in trait に対応していない可能性がある。実装時に以下の選択肢から判断する:

1. `async-trait` クレートを追加
2. `mockall` を 0.14 に更新
3. trait をやめて fixture ベースのテストに切り替え

## Implementation Order

ボトムアップで実装:

1. `error.rs` — エラー型定義 + テスト
2. `types.rs` — レスポンス型 + fixture テスト
3. `config.rs` — Builder パターン + テスト
4. `client.rs` — CommandRunner trait + ClaudeClient + モックテスト
5. E2E テスト (`#[ignore]`)
