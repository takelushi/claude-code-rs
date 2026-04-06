# Structured Output ヘルパー設計

## 概要

`ClaudeConfig` の `json_schema` フィールド（既存）を活用し、CLI レスポンスの `result` 文字列を任意の型 `T` に型安全にデシリアライズする高レベル API を提供する。

## 背景・動機

- `ClaudeConfig` に `json_schema: Option<String>` は追加済み → `--json-schema` CLI フラグにマッピングされる
- 現状、ユーザーは `ask()` で `ClaudeResponse` を受け取った後、`result` 文字列を手動で `serde_json::from_str` する必要がある
- 型安全なデシリアライズヘルパーと、`schemars` によるスキーマ自動生成を提供することで DX を向上させる

## 設計方針

### Feature Flag

`structured` feature で `schemars` クレートをオプショナル依存にする。

```toml
[features]
structured = ["dep:schemars"]

[dependencies]
schemars = { version = "1", optional = true }
```

| API | feature なし | `structured` 有効 |
|-----|-------------|-----------------|
| `ClaudeResponse::parse_result::<T>()` | 使える | 使える |
| `ClaudeClient::ask_structured::<T>()` | 使える | 使える |
| `generate_schema::<T>()` | なし | 使える |

### エラー型

`ClaudeError` に新しいバリアント `StructuredOutputError` を追加する。

```rust
#[non_exhaustive]
pub enum ClaudeError {
    // ... 既存バリアント ...

    /// CLI succeeded but the `result` field could not be deserialized
    /// into the target type.
    StructuredOutputError {
        raw_result: String,
        source: serde_json::Error,
    },
}
```

- `raw_result`: CLI が返した生の result 文字列（デバッグ用）
- `source`: デシリアライズエラーの詳細
- 既存の `ParseError` は CLI レスポンス全体の JSON パース失敗用のまま、意味を分離する

### コア API: `ClaudeResponse::parse_result::<T>()`

```rust
impl ClaudeResponse {
    /// Deserializes the `result` field into a strongly-typed value.
    pub fn parse_result<T: DeserializeOwned>(&self) -> Result<T, ClaudeError> {
        serde_json::from_str(&self.result).map_err(|e| ClaudeError::StructuredOutputError {
            raw_result: self.result.clone(),
            source: e,
        })
    }
}
```

- trait bound は `DeserializeOwned` のみ → feature flag 不要、常に使える
- stream でも非 stream でも `ClaudeResponse` を手に入れたら呼べる
- `is_error` のチェックはしない（バリデーションの責務は CLI 側、CLAUDE.md の方針に従う）

### スキーマ生成: `generate_schema::<T>()`

```rust
// src/structured.rs
/// Generates a JSON Schema string from a type implementing `JsonSchema`.
#[cfg(feature = "structured")]
pub fn generate_schema<T: JsonSchema>() -> Result<String, ClaudeError> {
    let schema = schema_for!(T);
    serde_json::to_string(&schema).map_err(ClaudeError::from)
}
```

- `structured` feature でゲート
- シリアライズ失敗時は既存の `ParseError(serde_json::Error)` を返す

### 便利メソッド: `ClaudeClient::ask_structured::<T>()`

```rust
// src/client.rs
impl<R: CommandRunner> ClaudeClient<R> {
    /// Sends a prompt and deserializes the result into `T`.
    ///
    /// Requires `json_schema` to be set on the config beforehand.
    /// Use `generate_schema::<T>()` to auto-generate it.
    pub async fn ask_structured<T: DeserializeOwned>(
        &self,
        prompt: &str,
    ) -> Result<T, ClaudeError> {
        let response = self.ask(prompt).await?;
        response.parse_result()
    }
}
```

- feature gate なし（`DeserializeOwned` のみで `schemars` 不要 → 手動スキーマユーザーも使える）
- スキーマ設定はしない（ユーザーが config 構築時に設定する責務）

## 使用例

### 自動スキーマ（`structured` feature 有効時）

```rust
#[derive(Deserialize, JsonSchema)]
struct Answer { value: i32 }

let config = ClaudeConfig::builder()
    .json_schema(generate_schema::<Answer>()?)
    .build();
let client = ClaudeClient::new(config);
let answer: Answer = client.ask_structured("What is 2+2?").await?;
```

### 手動スキーマ（feature なしでも使える）

```rust
#[derive(Deserialize)]
struct Answer { value: i32 }

let config = ClaudeConfig::builder()
    .json_schema(r#"{"type":"object","properties":{"value":{"type":"integer"}}}"#)
    .build();
let client = ClaudeClient::new(config);
let resp = client.ask("What is 2+2?").await?;
let answer: Answer = resp.parse_result()?;
```

### ストリーミング + structured output

```rust
let mut stream = client.ask_stream("What is 2+2?").await?;
let mut response = None;
while let Some(event) = stream.next().await {
    match event? {
        StreamEvent::Text(t) => print!("{t}"),
        StreamEvent::Result(r) => { response = Some(r); }
        _ => {}
    }
}
let answer: Answer = response.unwrap().parse_result()?;
```

## ファイル構成

```
src/
  lib.rs          # structured モジュール追加
  client.rs       # ask_structured メソッド追加（cfg gated）
  config.rs       # 変更なし（json_schema は既存）
  types.rs        # parse_result メソッド追加
  error.rs        # StructuredOutputError バリアント追加
  structured.rs   # 新規: generate_schema のみ
  stream.rs       # 変更なし
```

| ファイル | 変更内容 |
|---------|---------|
| `Cargo.toml` | `structured` feature + `schemars` optional dep |
| `src/lib.rs` | `mod structured` + `pub use` 追加 |
| `src/error.rs` | `StructuredOutputError` バリアント追加 |
| `src/types.rs` | `parse_result::<T>()` メソッド追加 |
| `src/structured.rs` | 新規: `generate_schema::<T>()` |
| `src/client.rs` | `ask_structured::<T>()` メソッド追加 |
| `CLAUDE.md` | Architecture セクション更新 |

## テスト戦略

### `parse_result`（src/types.rs）
- 正常系: 有効な JSON → `T` にデシリアライズ
- 異常系: 不正な JSON → `StructuredOutputError`（`raw_result` 保持を検証）
- 異常系: 型不一致 → `StructuredOutputError`

### `generate_schema`（src/structured.rs, feature gated）
- 正常系: 有効な JSON Schema 文字列が返る
- 正常系: 生成されたスキーマに期待するプロパティが含まれる

### `ask_structured`（src/client.rs）
- 正常系: モック + fixture で `T` が返る
- 異常系: CLI 成功だが result が `T` に合わない → `StructuredOutputError`
- 異常系: CLI 失敗 → 既存エラー（`NonZeroExit` 等）がそのまま返る

全テストで実際の CLI は呼ばない（`MockCommandRunner` + fixture）。

## Conversation API との競合について

- `client.rs` への変更は `#[cfg(feature = "structured")]` で囲んだ独立した `impl` ブロック追加のみ
- 既存メソッドの変更はないため、並行作業との競合リスクは最小限
- 実装フェーズでは最新の develop ブランチを確認してから進める

## スコープ外

- ストリーミング専用の structured ヘルパー（`parse_result` で十分カバー）
- `is_error` チェック（CLI 側の責務）
- enum による値制限（既存方針に従い `String` ベース）
