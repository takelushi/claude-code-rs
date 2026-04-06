# Conversation API 設計

## 背景

現状、複数ターン会話は毎回 `ClaudeConfig` / `ClaudeClient` を作り直し、`session_id` を手動で受け渡す必要があり冗長。`session_id` を自動管理する高レベル API を追加する。

### ゴールイメージ

```rust
let config = ClaudeConfig::builder()
    .no_session_persistence(false)
    .build();
let client = ClaudeClient::new(config);
let mut conv = client.conversation();
let r1 = conv.ask("What is 2+2?").await?;
let r2 = conv.ask("What was my question?").await?; // 自動で resume
```

## 設計判断

### 所有権モデル: Clone（ライフタイムフリー）

`Conversation` は `ClaudeClient` の `config` と `runner` を Clone して所有する。

**代替案と却下理由:**
- **参照（`&'a ClaudeClient`）**: ライフタイムパラメータが async で扱いにくい（spawn、struct 格納で問題）
- **`Arc<ClaudeClient>`**: `ClaudeClient` はステートレス（config + runner のみ、コネクションプール等なし）なので共有する実益がない

Clone コストは `String` 数個分 + `DefaultRunner`（ゼロサイズ構造体）で無視できる。

### オプションバリデーション: CLI 責任

`Conversation` は `no_session_persistence` 等のオプションを自動で上書きしない。バリデーションの責務は CLI コマンド側にある（既存 Convention と一貫）。

`no_session_persistence(false)` の設定はユーザー責任。未設定で `--resume` を使った場合、CLI のエラーが `NonZeroExit` として返る。doc comment で必要な設定を明記する。

### session_id 管理: `Arc<Mutex<Option<String>>>`

ストリーミング時、`ask_stream()` が返す `Stream` を消費する間に session_id を更新する必要がある。`&mut self` の借用とストリーム消費が同時に起きるため、`Arc<Mutex>` で内部共有する。

**代替案と却下理由:**
- **ストリーム全消費後に更新**: ユーザーが途中で drop した場合に session_id が失われる
- **`SystemInit` を消費して残りだけ返す**: 既存の `client.ask_stream()` と挙動が変わり、`model` 等の情報が取れなくなる

### ストリームのイベントフィルタリング: 全透過

`Conversation` の `ask_stream()` は全イベントをそのまま返す。`SystemInit` や `Result` も透過する。内部では session_id のコピーだけを取る。

## アーキテクチャ

### ファイル構成

```
src/
  conversation.rs  ← 新規: Conversation 構造体
  config.rs        ← to_builder() メソッド追加
  client.rs        ← convenience method 追加
  lib.rs           ← re-export 追加
```

### 前提: ClaudeConfig::to_builder()

`ask_with()` のクロージャは `FnOnce(ClaudeConfigBuilder) -> ClaudeConfigBuilder` を受け取る。内部で base config を `ClaudeConfigBuilder` に変換する必要があるため、`ClaudeConfig` に `to_builder()` メソッドを追加する。

```rust
impl ClaudeConfig {
    /// Creates a builder pre-filled with the current configuration values.
    pub fn to_builder(&self) -> ClaudeConfigBuilder { ... }
}
```

### Conversation 構造体

```rust
pub struct Conversation<R: CommandRunner + Clone = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
    session_id: Arc<Mutex<Option<String>>>,
}
```

### 公開 API

#### Conversation メソッド

| メソッド | シグネチャ | 説明 |
|----------|-----------|------|
| `ask` | `(&mut self, &str) -> Result<ClaudeResponse, ClaudeError>` | JSON モード（`ask_with` のショートカット） |
| `ask_with` | `(&mut self, &str, F) -> Result<ClaudeResponse, ClaudeError>` | JSON モード + config 上書き |
| `ask_stream` | `(&mut self, &str) -> Result<Pin<Box<dyn Stream<...>>>, ClaudeError>` | ストリーミング（`ask_stream_with` のショートカット） |
| `ask_stream_with` | `(&mut self, &str, F) -> Result<Pin<Box<dyn Stream<...>>>, ClaudeError>` | ストリーミング + config 上書き |
| `session_id` | `(&self) -> Option<String>` | 現在の session_id を取得 |

`F: FnOnce(ClaudeConfigBuilder) -> ClaudeConfigBuilder`

`ask()` / `ask_stream()` は対応する `_with` バリアントに identity クロージャを渡すだけ。

#### ClaudeClient への追加

| メソッド | シグネチャ | 説明 |
|----------|-----------|------|
| `conversation` | `(&self) -> Conversation<R>` | 新規 Conversation を生成 |
| `conversation_resume` | `(&self, impl Into<String>) -> Conversation<R>` | 既存セッションで再開 |

`R: Clone` 境界が必要。

## 内部フロー

### ask_with() の処理

```
1. base config を clone
2. ClaudeConfigBuilder に変換してクロージャで上書き → build
3. session_id.lock() が Some(id) → config.resume = Some(id)
4. ClaudeClient::with_runner(config, runner.clone()) で一時 client 生成
5. client.ask(prompt) を実行
6. 成功時: response.session_id を Arc<Mutex> に保存
7. ClaudeResponse を返す
```

### ask_stream_with() の処理

```
1. base config を clone
2. ClaudeConfigBuilder に変換してクロージャで上書き → build
3. session_id.lock() が Some(id) → config.resume = Some(id)
4. ClaudeClient::with_runner(config, runner.clone()) で一時 client 生成
5. client.ask_stream(prompt) を実行
6. Stream をラップ:
   - 全イベントを透過
   - SystemInit → session_id を Arc<Mutex> に保存
   - Result → session_id を Arc<Mutex> に上書き（最終値として信頼）
7. ラップした Stream を返す
```

### session_id 更新タイミング

| モード | 取得元 | タイミング |
|--------|--------|-----------|
| ask | `ClaudeResponse.session_id` | レスポンス返却時 |
| ask_stream | `StreamEvent::SystemInit` | ストリーム冒頭 |
| ask_stream | `StreamEvent::Result` | ストリーム末尾（上書き） |

## エラーハンドリング

既存の `ClaudeError` をそのまま使う。Conversation 固有のエラーバリアントは追加しない。

| ケース | 挙動 |
|--------|------|
| 初回 ask 失敗 | session_id は `None` のまま。再度 ask 可能 |
| 2ターン目以降で失敗 | session_id は前回の値を維持。リトライ可能 |
| `no_session_persistence` 未設定 | CLI エラー → `NonZeroExit` |
| ストリーム途中切断 | `SystemInit` 到達済みなら session_id 保持 |
| ストリーム drop | `SystemInit` 到達済みなら更新済み |

リトライ機構は持たない。エラー後も session_id を維持するため、ユーザーが再度 `ask()` すればリトライになる。

## テスト戦略

### ユニットテスト（MockCommandRunner）

| テスト | 検証内容 |
|--------|----------|
| 初回 ask で session_id 取得 | レスポンスから session_id が保存されること |
| 2ターン目で `--resume` 付与 | args に `--resume <id>` が含まれること |
| `ask_with()` で config 上書き | 上書き値が args に反映されること |
| `ask_with()` で base config 不変 | 上書きが次ターンに影響しないこと |
| `conversation_resume()` 初期値 | 初回から `--resume <id>` が付くこと |
| `session_id()` 初期値 | `None` を返すこと |
| ask 失敗時の session_id 維持 | エラー後も直前の session_id が残ること |
| stream で session_id 取得 | `SystemInit` から session_id がキャプチャされること |
| stream で Result 優先更新 | `Result` の session_id で上書きされること |

### E2E テスト（`#[ignore]`）

| テスト | 検証内容 |
|--------|----------|
| 2ターン会話 | `ask()` → `ask()` で会話継続 |
| stream 2ターン | `ask_stream()` → `ask()` でセッション維持 |

E2E は `--model haiku` + `max_turns(1)` で課金最小化。
