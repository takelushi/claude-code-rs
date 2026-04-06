# claude-code-rs

Claude Code CLI をRustから実行するためのライブラリ。

## Project Overview

- `claude` CLI の `--print` モードをサブプロセスとして実行し、結果を型安全に扱う
- 出力フォーマット: `--output-format json` (単発) / `--output-format stream-json` (ストリーミング)
- ライセンス: MIT

## Tech Stack

- Rust 1.93+ (edition 2024)
- 非同期ランタイム: tokio
- シリアライズ: serde / serde_json
- プロセス実行: tokio::process
- ストリーミング: tokio-stream / async-stream
- エラー処理: thiserror
- テスト: cargo test + mockall 0.14

## Development

### Commands

```sh
cargo build                    # ビルド
cargo test                     # テスト実行
cargo test -- --ignored        # E2E テスト実行
cargo clippy                   # lint
cargo fmt --check              # フォーマットチェック
cargo fmt                      # フォーマット適用
cargo doc --open               # ドキュメント生成
cargo run --example simple            # 動作確認
cargo run --example stream            # ストリーミング動作確認
cargo run --example stream-all        # 全イベント表示
cargo run --example multi_turn        # 複数ターン会話
cargo run --example structured_output # 構造化出力
```

### Workflow

- TDD (Explore -> Red -> Green -> Refactor)
- `cargo clippy -- -D warnings` を通すこと
- `cargo fmt` を適用してからコミット
- pub API には doc comment を書く

### Architecture

```plain
src/
  lib.rs           # pub API re-export
  client.rs        # ClaudeClient (CLI実行の中核: ask, ask_structured, ask_stream)
  config.rs        # ClaudeConfig (--model, --system-prompt 等のオプション)
  conversation.rs  # Conversation (session_id 自動管理の複数ターン会話)
  types.rs         # ClaudeResponse (parse_result 含む), Usage 等のコア型定義
  error.rs         # エラー型
  stream.rs        # StreamEvent + stream-json のパース・イテレーション・バッファリング
  structured.rs    # generate_schema: JsonSchema → JSON Schema 文字列生成 (structured feature)
examples/
  simple.rs        # 最小限の動作確認用サンプル
  stream.rs        # ストリーミング動作確認用サンプル
  stream-all.rs         # 全イベント表示サンプル
  multi_turn.rs         # 複数ターン会話サンプル
  structured_output.rs  # 構造化出力サンプル
```

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
- tracing は `client.rs` 内の条件付きマクロ（`trace_debug!` 等）で吸収

### Error Variants

`ClaudeError` で想定するバリアント:

- `CliNotFound` — `claude` コマンドが PATH に見つからない
- `NonZeroExit { code, stderr }` — CLI が非ゼロ終了コードを返した
- `ParseError` — JSON / stream-json レスポンスのデシリアライズ失敗
- `Timeout` — 指定時間内に応答が返らなかった
- `Io` — プロセス起動・stdout/stderr 読み取り等の I/O エラー
- `StructuredOutputError { raw_result, source }` — CLI は成功したが result の JSON デシリアライズに失敗した

### Testing Strategy

- CLI 実行を `CommandRunner` trait で抽象化し、mockall でモックする
- `tests/fixtures/` に CLI の stdout を再現した JSON ファイルを配置
- ユニットテスト: モック + fixture でCLIを呼ばずに各モジュールを単体テスト
- 結合テスト / E2E: 実際に `claude` CLI を `--model haiku` で実行し、課金を最小化する
- E2E テストは `#[ignore]` を付与し、`cargo test -- --ignored` で明示的に実行する

### Documentation Policy

- バグや設計ミスを修正したら、再発防止策を本ファイルの Conventions に追記する
- 新しいモジュールや外部仕様を発見したら `docs/` に記録する
- Architecture のファイル構成が変わったら本ファイルを更新する
- 実装中に判明した Claude CLI の挙動・制約は `docs/claude-cli.md` に記録する

### Conventions

- エラーは `thiserror` で定義し、`Result<T, ClaudeError>` を返す
- Builder パターンで `ClaudeConfig` を構築
- 非同期 API を基本とし、同期ラッパーは提供しない
- `#[must_use]`, `#[non_exhaustive]` を適切に使う
- テストでは実際の `claude` CLI を呼ばない (モック or fixture)
- `mockall` の `returning` は async クロージャ非対応。非同期の遅延が必要なテスト（timeout 等）は手動で trait 実装した struct を使う
- `MockCommandRunner` は `Clone` 未対応。`Conversation` のように runner を clone するコンポーネントのテストには、手動で trait 実装した `RecordingRunner`（`Arc<Mutex>` で状態共有）を使う
- `CommandRunner::run()` は完了済みの `Output` を返すため、ストリーミングを抽象化できない。`ask_stream` は常に実プロセスを起動する `DefaultRunner` 限定
- `CommandRunner` trait に `#[allow(async_fn_in_trait)]` を付与する（ライブラリ内部用のため `Send` 境界の警告を抑制）
- コードコメント・doc comment は英語で書く
- CLI オプションの値制限（`effort`, `permission_mode` 等）は enum ではなく `String` + 定数モジュールで表現する。Claude Code CLI は活発に開発されており、enum では新しい値の追加のたびにライブラリリリースが必要になるため
- ライブラリはオプション間の排他チェック・バリデーションを行わない。バリデーションの責務は CLI コマンド側にある
