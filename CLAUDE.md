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
cargo run --example simple     # 動作確認
```

### Workflow

- TDD (Explore -> Red -> Green -> Refactor)
- `cargo clippy -- -D warnings` を通すこと
- `cargo fmt` を適用してからコミット
- pub API には doc comment を書く

### Architecture

```plain
src/
  lib.rs        # pub API re-export
  client.rs     # ClaudeClient (CLI実行の中核)
  config.rs     # ClaudeConfig (--model, --system-prompt 等のオプション)
  types.rs      # JSON/stream-json 両方の型定義のみ
  error.rs      # エラー型
  stream.rs     # stream-json のパース・イテレーション・バッファリング
examples/
  simple.rs     # 最小限の動作確認用サンプル
```

### Error Variants

`ClaudeError` で想定するバリアント:

- `CliNotFound` — `claude` コマンドが PATH に見つからない
- `NonZeroExit { code, stderr }` — CLI が非ゼロ終了コードを返した
- `ParseError` — JSON / stream-json レスポンスのデシリアライズ失敗
- `Timeout` — 指定時間内に応答が返らなかった
- `Io` — プロセス起動・stdout/stderr 読み取り等の I/O エラー

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
- `CommandRunner` trait に `#[allow(async_fn_in_trait)]` を付与する（ライブラリ内部用のため `Send` 境界の警告を抑制）
- コードコメント・doc comment は英語で書く
