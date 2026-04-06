# Operational Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ストリーミングの堅牢性・運用性を改善する（ChildGuard, idle timeout, check_cli, ドキュメント更新）

**Architecture:** `ask_stream` 内のプロセス管理を `ChildGuard` RAII ラッパーで安全にし、idle timeout を `ClaudeConfig` 経由で設定可能にする。`check_cli` はフリー関数として `client.rs` に配置する。

**Tech Stack:** Rust 1.93+, tokio (process, time), async-stream, tokio-stream

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/client.rs` | Modify | `ChildGuard` 追加, `ask_stream` のリファクタ, idle timeout 適用, `check_cli` 関数追加 |
| `src/config.rs` | Modify | `stream_idle_timeout` フィールド + builder メソッド追加 |
| `src/lib.rs` | Modify | `check_cli` re-export 追加 |
| `tests/e2e.rs` | Modify | `check_cli` E2E テスト追加 |
| `docs/claude-cli.md` | Modify | drop 挙動の記録追加 |

---

### Task 1: ChildGuard 構造体の追加とテスト

**Files:**
- Modify: `src/client.rs`

- [ ] **Step 1: `ChildGuard` 構造体と `Drop` 実装を書く**

`src/client.rs` の `DefaultRunner` impl の直後（L58 付近）に追加:

```rust
/// RAII guard that kills the child process on drop.
///
/// tokio's `Child` does NOT kill the process on drop — it detaches.
/// This guard ensures the CLI subprocess is killed when the stream
/// is dropped (e.g., client disconnection).
struct ChildGuard(Option<tokio::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.0 {
            let _ = child.start_kill();
        }
    }
}
```

- [ ] **Step 2: `ask_stream` 内で `ChildGuard` を使うようリファクタする**

`src/client.rs` の `ask_stream` メソッド（L94-143）を以下のように変更:

```rust
    pub async fn ask_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        let args = self.config.to_stream_args(prompt);

        trace_debug!(args = ?args, "spawning claude CLI stream");

        let mut child = TokioCommand::new("claude")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ClaudeError::CliNotFound
                } else {
                    ClaudeError::Io(e)
                }
            })?;

        let stdout = child.stdout.take().expect("stdout must be piped");
        let reader = BufReader::new(stdout);
        let event_stream = parse_stream(reader);
        let mut guard = ChildGuard(Some(child));

        Ok(Box::pin(async_stream::stream! {
            tokio::pin!(event_stream);
            while let Some(event) = tokio_stream::StreamExt::next(&mut event_stream).await {
                yield Ok(event);
            }

            // Take child out of guard to wait for exit status.
            // If stream is dropped before reaching here, guard's Drop kills the process.
            if let Some(mut child) = guard.0.take() {
                let status = child.wait().await;
                match status {
                    Ok(s) if !s.success() => {
                        let code = s.code().unwrap_or(-1);
                        let mut stderr_buf = Vec::new();
                        if let Some(mut stderr) = child.stderr.take() {
                            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_buf).await;
                        }
                        let stderr_str = String::from_utf8_lossy(&stderr_buf).into_owned();
                        yield Err(ClaudeError::NonZeroExit { code, stderr: stderr_str });
                    }
                    Err(e) => {
                        yield Err(ClaudeError::Io(e));
                    }
                    Ok(_) => {}
                }
            }
        }))
    }
```

- [ ] **Step 3: ビルドとテストが通ることを確認する**

Run: `cargo test`
Expected: 全テスト PASS（ChildGuard は内部リファクタのみで外部挙動は変わらない）

Run: `cargo clippy -- -D warnings`
Expected: warning なし

- [ ] **Step 4: コミット**

```bash
git add src/client.rs
git commit -m "feat: add ChildGuard to kill child process on stream drop"
```

---

### Task 2: stream_idle_timeout の config 追加

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: テストを書く**

`src/config.rs` の `mod tests` 内に追加:

```rust
    #[test]
    fn builder_sets_stream_idle_timeout() {
        let config = ClaudeConfig::builder()
            .stream_idle_timeout(Duration::from_secs(60))
            .build();
        assert_eq!(
            config.stream_idle_timeout,
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn default_stream_idle_timeout_is_none() {
        let config = ClaudeConfig::default();
        assert!(config.stream_idle_timeout.is_none());
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test --lib config::tests::builder_sets_stream_idle_timeout`
Expected: FAIL — `stream_idle_timeout` フィールドが存在しない

- [ ] **Step 3: `ClaudeConfig` にフィールドを追加する**

`src/config.rs` の `ClaudeConfig` 構造体（L15 の `timeout` フィールドの直後）に追加:

```rust
    /// Idle timeout for streams. If no event arrives within this duration,
    /// the stream yields [`ClaudeError::Timeout`](crate::ClaudeError::Timeout)
    /// and terminates. Library-only; not a CLI flag.
    pub stream_idle_timeout: Option<Duration>,
```

- [ ] **Step 4: `ClaudeConfigBuilder` にフィールドとメソッドを追加する**

`ClaudeConfigBuilder` 構造体（L284 の `timeout` フィールドの直後）に追加:

```rust
    stream_idle_timeout: Option<Duration>,
```

builder メソッド（L347 の `timeout` メソッドの直後）に追加:

```rust
    /// Sets the idle timeout for streams.
    ///
    /// If no event arrives within this duration, the stream yields
    /// [`ClaudeError::Timeout`](crate::ClaudeError::Timeout) and terminates.
    #[must_use]
    pub fn stream_idle_timeout(mut self, timeout: Duration) -> Self {
        self.stream_idle_timeout = Some(timeout);
        self
    }
```

- [ ] **Step 5: `to_builder()` と `build()` に `stream_idle_timeout` を追加する**

`to_builder()` メソッド内（L84 の `timeout` の直後）に追加:

```rust
            stream_idle_timeout: self.stream_idle_timeout,
```

`build()` メソッド内（L566 の `timeout` の直後）に追加:

```rust
            stream_idle_timeout: self.stream_idle_timeout,
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test --lib config::tests`
Expected: 全テスト PASS

Run: `cargo clippy -- -D warnings`
Expected: warning なし

- [ ] **Step 7: コミット**

```bash
git add src/config.rs
git commit -m "feat: add stream_idle_timeout to ClaudeConfig"
```

---

### Task 3: ask_stream に idle timeout を適用する

**Files:**
- Modify: `src/client.rs`

- [ ] **Step 1: `ask_stream` のストリームループに timeout ロジックを追加する**

`src/client.rs` の `ask_stream` メソッド内、ストリームループを以下に置き換え:

```rust
        let idle_timeout = self.config.stream_idle_timeout;

        Ok(Box::pin(async_stream::stream! {
            tokio::pin!(event_stream);

            loop {
                let next = tokio_stream::StreamExt::next(&mut event_stream);
                let maybe_event = if let Some(timeout_dur) = idle_timeout {
                    match tokio::time::timeout(timeout_dur, next).await {
                        Ok(Some(event)) => Some(event),
                        Ok(None) => None,
                        Err(_) => {
                            trace_error!("stream idle timeout");
                            yield Err(ClaudeError::Timeout);
                            return;
                        }
                    }
                } else {
                    next.await
                };

                match maybe_event {
                    Some(event) => yield Ok(event),
                    None => break,
                }
            }

            if let Some(mut child) = guard.0.take() {
                let status = child.wait().await;
                match status {
                    Ok(s) if !s.success() => {
                        let code = s.code().unwrap_or(-1);
                        let mut stderr_buf = Vec::new();
                        if let Some(mut stderr) = child.stderr.take() {
                            let _ = tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut stderr_buf).await;
                        }
                        let stderr_str = String::from_utf8_lossy(&stderr_buf).into_owned();
                        yield Err(ClaudeError::NonZeroExit { code, stderr: stderr_str });
                    }
                    Err(e) => {
                        yield Err(ClaudeError::Io(e));
                    }
                    Ok(_) => {}
                }
            }
        }))
```

- [ ] **Step 2: ビルドとテストが通ることを確認する**

Run: `cargo test`
Expected: 全テスト PASS

Run: `cargo clippy -- -D warnings`
Expected: warning なし

- [ ] **Step 3: コミット**

```bash
git add src/client.rs
git commit -m "feat: apply idle timeout to ask_stream"
```

---

### Task 4: check_cli フリー関数の追加

**Files:**
- Modify: `src/client.rs`
- Modify: `src/lib.rs`
- Modify: `tests/e2e.rs`

- [ ] **Step 1: `check_cli` 関数を実装する**

`src/client.rs` の末尾（`impl<R: CommandRunner + Clone> ClaudeClient<R>` ブロックの後、`#[cfg(test)]` の前）に追加:

```rust
/// Checks that the `claude` CLI is available and returns its version string.
///
/// Runs `claude --version` and returns the trimmed stdout on success.
///
/// # Errors
///
/// - [`ClaudeError::CliNotFound`] if `claude` is not in PATH.
/// - [`ClaudeError::NonZeroExit`] if the command fails.
/// - [`ClaudeError::Io`] for other I/O errors.
pub async fn check_cli() -> Result<String, ClaudeError> {
    let output = TokioCommand::new("claude")
        .arg("--version")
        .output()
        .await
        .map_err(|e| {
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

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(version)
}
```

- [ ] **Step 2: `lib.rs` で re-export する**

`src/lib.rs` の `pub use client::` 行を変更:

```rust
pub use client::{check_cli, ClaudeClient, CommandRunner, DefaultRunner};
```

- [ ] **Step 3: ビルドが通ることを確認する**

Run: `cargo build`
Expected: SUCCESS

- [ ] **Step 4: E2E テストを追加する**

`tests/e2e.rs` の末尾に追加:

```rust
#[tokio::test]
#[ignore] // Run explicitly with: cargo test -- --ignored
async fn e2e_check_cli() {
    let version = claude_code_rs::check_cli().await.unwrap();
    assert!(
        !version.is_empty(),
        "check_cli should return a non-empty version string"
    );
}
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test`
Expected: 全テスト PASS（E2E は `#[ignore]` なのでスキップ）

Run: `cargo clippy -- -D warnings`
Expected: warning なし

- [ ] **Step 6: コミット**

```bash
git add src/client.rs src/lib.rs tests/e2e.rs
git commit -m "feat: add check_cli free function for CLI health check"
```

---

### Task 5: ドキュメント更新

**Files:**
- Modify: `docs/claude-cli.md`

- [ ] **Step 1: `docs/claude-cli.md` に drop 挙動のセクションを追記する**

ファイル末尾に追加:

```markdown
## tokio Child の drop 挙動

tokio の `Child` は drop 時にプロセスを kill しない。detach されるだけであり、親プロセスが終了するまでゾンビ的に残る可能性がある。

ライブラリでは `ask_stream` 内で `ChildGuard` RAII ラッパーを使い、ストリーム drop 時に `start_kill()` で SIGKILL を送ることで対処している。`start_kill()` は非同期ではなく即座にシグナルを送るため `Drop` トレイト内から呼べる。
```

- [ ] **Step 2: コミット**

```bash
git add docs/claude-cli.md
git commit -m "docs: document tokio Child drop behavior and ChildGuard"
```

---

### Task 6: CLAUDE.md の更新

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Architecture セクションの説明を更新する**

`CLAUDE.md` の `client.rs` の説明を更新して `check_cli` を追加:

```
  client.rs        # ClaudeClient (CLI実行の中核: ask, ask_structured, ask_stream), check_cli
```

- [ ] **Step 2: コミット**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md architecture with check_cli"
```

---

### Task 7: 最終確認

- [ ] **Step 1: 全テスト・lint を通す**

Run: `cargo test`
Expected: 全テスト PASS

Run: `cargo clippy -- -D warnings`
Expected: warning なし

Run: `cargo fmt --check`
Expected: フォーマット差分なし

- [ ] **Step 2: E2E テストを実行する（任意）**

Run: `cargo test -- --ignored`
Expected: `e2e_check_cli` を含む全 E2E テスト PASS
