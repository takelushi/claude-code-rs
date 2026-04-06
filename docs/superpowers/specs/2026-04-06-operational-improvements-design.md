# Operational Improvements 設計

APIサーバーユースケースに向けた、ストリーミングの堅牢性・運用性改善。

## 1. ChildGuard（drop時自動kill）

### 背景

`ask_stream` で spawn した `tokio::process::Child` が `async_stream::stream!` クロージャにmoveされている。tokio の `Child` は drop 時にプロセスを kill せず detach するため、ストリームを途中で drop するとCLIプロセスがゾンビ的に残る。

### 設計

`client.rs` 内に `ChildGuard` 構造体を追加する。

```rust
/// RAII guard that kills the child process on drop.
struct ChildGuard(Option<tokio::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.0 {
            let _ = child.start_kill();
        }
    }
}
```

- `start_kill()` は非同期ではなく即座に SIGKILL を送るので `Drop` 内で呼べる
- `ask_stream` 内で `child` を `ChildGuard` に包んでからクロージャにmoveする
- 正常終了時（`child.wait().await` 後）は guard から取り出し済みなので二重killしない
- 外部公開しない内部実装（`pub(crate)` 以下）

### 変更箇所

- `src/client.rs`

## 2. ストリーム idle timeout

### 背景

`ask()` には `ClaudeConfig::timeout` によるタイムアウト機構があるが、`ask_stream()` には適用されない。CLIプロセスが応答しなくなった場合にストリームが無限にブロックされる。

### 設計

`ClaudeConfig` に `stream_idle_timeout: Option<Duration>` フィールドを追加する。

```rust
// ClaudeConfig
pub stream_idle_timeout: Option<Duration>,

// ClaudeConfigBuilder
pub fn stream_idle_timeout(mut self, duration: Duration) -> Self
```

`ask_stream` 内のストリームループで、`event_stream.next()` を `tokio::time::timeout()` でラップする。

- タイムアウト発生時: `yield Err(ClaudeError::Timeout)` してストリーム終了
- `ChildGuard` の drop で子プロセスもkillされる
- `None` の場合: 現状と同じ（タイムアウトなし）

`Conversation::ask_stream` へは `ClaudeConfig` 経由で自然に伝播する。

### 変更箇所

- `src/config.rs` — フィールド + builder メソッド追加
- `src/client.rs` — ストリームループ内に timeout ロジック追加

## 3. ヘルスチェック（`check_cli`）

### 背景

`claude` CLI が PATH に存在するかを事前確認する手段がない。APIサーバー起動時やヘルスチェックエンドポイントで必要になる。

### 設計

フリー関数として提供する。

```rust
/// Checks that the `claude` CLI is available and returns its version string.
pub async fn check_cli() -> Result<String, ClaudeError>
```

- `tokio::process::Command::new("claude").arg("--version").output().await` を実行
- `NotFound` → `ClaudeError::CliNotFound`
- 非ゼロ終了 → `ClaudeError::NonZeroExit`
- 成功 → stdout を trim して `String` で返す（例: `"claude 1.0.34"`）

config に依存しないため `ClaudeClient` のメソッドにはしない。

### 配置

- `src/client.rs` に関数定義
- `src/lib.rs` で `pub use client::check_cli` として re-export

### テスト

- ユニットテスト: なし（実CLIに依存するため）
- E2Eテスト: `#[ignore]` 付きで `check_cli` が `Ok` を返すことを確認

## 4. ドキュメント更新

### 対象

`docs/claude-cli.md` に以下を追記:

- tokio の `Child` は drop 時にプロセスを kill しない（detach される）挙動
- ライブラリでは `ChildGuard` で `start_kill()` を呼んで対処している旨

### 追記しないもの

- APIサーバー構築ガイド（ライブラリのスコープ外）
- OpenAI API とのパラメータ差異（CLI制約であり既知）
