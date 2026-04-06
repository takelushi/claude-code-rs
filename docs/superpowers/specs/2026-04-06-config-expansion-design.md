# ClaudeConfig 拡張設計

## 概要

`ClaudeConfig` を拡張し、`claude -p` で有効なすべてのオプションに型付きフィールドで対応する。加えて、CLI のバージョンアップで追加される未知のオプションに対応するためのエスケープハッチ（`extra_args`）を提供する。

## 設計原則

### 1. CLI バージョンアップへの追従コスト最小化

Claude Code CLI は活発に開発されており、オプションの値（`effort` の選択肢等）が頻繁に追加される。このため：

- 値制限のあるオプション（`effort`, `permission_mode` 等）は **enum ではなく `String` + 定数モジュール**を採用
- enum にすると新しい値の追加のたびにライブラリのリリースが必要になるが、String なら `extra_args` に頼らずそのまま渡せる
- 定数モジュールにより IDE 補完・発見しやすさは enum と同等

### 2. バリデーションの責務は CLI 側

- ライブラリはオプション間の排他チェック・バリデーションを**行わない**
- 例: `--no-session-persistence` と `--resume` の排他チェックは CLI が行う
- ライブラリは薄いラッパーに徹し、引数をそのまま CLI に渡す
- CLI のバリデーションエラーは `ClaudeError::NonZeroExit` として返る

### 3. コンテキスト最小化デフォルト

- `None` / 空 `Vec` → ライブラリのデフォルト動作（コンテキスト最小化）を維持
- `Some` / 非空 `Vec` → ユーザー指定値で上書き
- 既存の最小化設定（`--tools ""`, `--setting-sources ""` 等）はデフォルトで有効のまま

## アプローチ

**フラットな構造体拡張**（案 A）を採用。

- 既存の `ClaudeConfig` にフィールドを追加する
- `#[non_exhaustive]` が付いているため、フィールド追加は non-breaking change
- Builder パターンで隠蔽されるため、フィールド数の多さはユーザーに影響しない
- 既存 API との後方互換性を完全に維持

他の検討案:
- 案 B（カテゴリ別ネスト構造体）: 既存 API が破壊的変更になるため不採用
- 案 C（型消去マップ方式）: 型安全性がなく Rust らしくないため不採用

## フィールド一覧

### 既存フィールド（変更なし）

| フィールド | 型 | CLI フラグ | デフォルト |
|-----------|-----|-----------|-----------|
| `model` | `Option<String>` | `--model` | `None` |
| `system_prompt` | `Option<String>` | `--system-prompt` | `None`（→ `""` に変換） |
| `max_turns` | `Option<u32>` | `--max-turns` | `None` |
| `timeout` | `Option<Duration>` | *(ライブラリ独自)* | `None` |
| `include_partial_messages` | `Option<bool>` | `--include-partial-messages` | `None` |

### 新規フィールド

| フィールド | 型 | CLI フラグ | デフォルト |
|-----------|-----|-----------|-----------|
| `append_system_prompt` | `Option<String>` | `--append-system-prompt` | `None` |
| `fallback_model` | `Option<String>` | `--fallback-model` | `None` |
| `effort` | `Option<String>` | `--effort` | `None` |
| `max_budget_usd` | `Option<f64>` | `--max-budget-usd` | `None` |
| `allowed_tools` | `Vec<String>` | `--allowedTools` | `[]` |
| `disallowed_tools` | `Vec<String>` | `--disallowedTools` | `[]` |
| `tools` | `Option<String>` | `--tools` | `None`（→ `""` に変換） |
| `mcp_config` | `Vec<String>` | `--mcp-config` | `[]`（→ `'{"mcpServers":{}}'` に変換） |
| `setting_sources` | `Option<String>` | `--setting-sources` | `None`（→ `""` に変換） |
| `settings` | `Option<String>` | `--settings` | `None` |
| `json_schema` | `Option<String>` | `--json-schema` | `None` |
| `include_hook_events` | `Option<bool>` | `--include-hook-events` | `None` |
| `permission_mode` | `Option<String>` | `--permission-mode` | `None` |
| `dangerously_skip_permissions` | `Option<bool>` | `--dangerously-skip-permissions` | `None` |
| `add_dir` | `Vec<String>` | `--add-dir` | `[]` |
| `file` | `Vec<String>` | `--file` | `[]` |
| `resume` | `Option<String>` | `--resume` | `None` |
| `session_id` | `Option<String>` | `--session-id` | `None` |
| `bare` | `Option<bool>` | `--bare` | `None` |
| `no_session_persistence` | `Option<bool>` | `--no-session-persistence` | `None`（→ デフォルト有効） |
| `disable_slash_commands` | `Option<bool>` | `--disable-slash-commands` | `None`（→ デフォルト有効） |
| `strict_mcp_config` | `Option<bool>` | `--strict-mcp-config` | `None`（→ デフォルト有効） |
| `extra_args` | `Vec<String>` | *(任意)* | `[]` |

## `base_args()` のデフォルト動作

`None` はコンテキスト最小化デフォルトを維持、`Some` で上書きする。

```rust
fn base_args(&self) -> Vec<String> {
    let mut args = vec!["--print".into()];

    // no_session_persistence: None → 有効、Some(false) → 無効
    if self.no_session_persistence != Some(false) {
        args.push("--no-session-persistence".into());
    }

    // setting_sources: None → "" (最小化)、Some("user,project") → その値
    args.push("--setting-sources".into());
    args.push(self.setting_sources.clone().unwrap_or_default());

    // strict_mcp_config: None → 有効、Some(false) → 無効
    if self.strict_mcp_config != Some(false) {
        args.push("--strict-mcp-config".into());
    }

    // mcp_config: [] → '{"mcpServers":{}}' (最小化)、指定あり → その値
    if self.mcp_config.is_empty() {
        args.push("--mcp-config".into());
        args.push(r#"{"mcpServers":{}}"#.into());
    } else {
        for cfg in &self.mcp_config {
            args.push("--mcp-config".into());
            args.push(cfg.clone());
        }
    }

    // tools: None → "" (最小化)、Some("Bash,Edit") → その値
    args.push("--tools".into());
    args.push(self.tools.clone().unwrap_or_default());

    // disable_slash_commands: None → 有効、Some(false) → 無効
    if self.disable_slash_commands != Some(false) {
        args.push("--disable-slash-commands".into());
    }

    // system_prompt: None → ""、Some → その値
    args.push("--system-prompt".into());
    args.push(self.system_prompt.clone().unwrap_or_default());

    // 以下は None なら省略
    if let Some(ref sp) = self.append_system_prompt {
        args.push("--append-system-prompt".into());
        args.push(sp.clone());
    }
    if let Some(ref model) = self.model {
        args.push("--model".into());
        args.push(model.clone());
    }
    // Option<String>: None なら省略
    // fallback_model, effort, settings, json_schema, permission_mode,
    // resume, session_id も同パターン

    // Option<u32>: max_turns
    // Option<f64>: max_budget_usd

    // Vec<String>: 非空なら展開
    // allowed_tools, disallowed_tools → 1フラグに全要素
    // add_dir, file → 1フラグに全要素
    // mcp_config → フラグ繰り返し（上のデフォルト処理で対応済み）

    // Option<bool>: Some(true) なら --flag を追加
    // bare, dangerously_skip_permissions, include_hook_events

    // extra_args はプロンプトの直前
    args.extend(self.extra_args.iter().cloned());

    args
}
```

## Builder メソッドのパターン

### `Option<String>` 型

```rust
pub fn effort(mut self, effort: impl Into<String>) -> Self {
    self.effort = Some(effort.into());
    self
}
```

### `Option<bool>` 型

```rust
pub fn bare(mut self, enabled: bool) -> Self {
    self.bare = Some(enabled);
    self
}
```

### `Vec<String>` 型（一括セット + 単体追加）

```rust
/// Sets allowed tools (replaces any previous values).
pub fn allowed_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
    self.allowed_tools = tools.into_iter().map(Into::into).collect();
    self
}

/// Adds a single allowed tool.
pub fn add_allowed_tool(mut self, tool: impl Into<String>) -> Self {
    self.allowed_tools.push(tool.into());
    self
}
```

同じパターンを `disallowed_tools`, `add_dir`, `file`, `mcp_config`, `extra_args` にも適用する。

## `Vec` 型フィールドの引数生成ルール

| フィールド | CLI 生成方式 |
|-----------|-------------|
| `allowed_tools` | `--allowedTools val1 val2 ...`（1回のフラグに全要素） |
| `disallowed_tools` | `--disallowedTools val1 val2 ...` |
| `add_dir` | `--add-dir dir1 dir2 ...` |
| `file` | `--file spec1 spec2 ...` |
| `mcp_config` | `--mcp-config cfg1 --mcp-config cfg2`（フラグ繰り返し） |

## 定数モジュール

`src/config.rs` の末尾に配置し、`lib.rs` から re-export する。

```rust
/// Known values for `--effort`.
pub mod effort {
    pub const LOW: &str = "low";
    pub const MEDIUM: &str = "medium";
    pub const HIGH: &str = "high";
    pub const MAX: &str = "max";
}

/// Known values for `--permission-mode`.
pub mod permission_mode {
    pub const DEFAULT: &str = "default";
    pub const ACCEPT_EDITS: &str = "acceptEdits";
    pub const AUTO: &str = "auto";
    pub const BYPASS_PERMISSIONS: &str = "bypassPermissions";
    pub const DONT_ASK: &str = "dontAsk";
    pub const PLAN: &str = "plan";
}
```

## `extra_args`

- 型: `Vec<String>`
- 挿入位置: `base_args()` の末尾、プロンプトの直前
- 引数順序: `[--print] [最小化デフォルト群] [型付きオプション群] [extra_args] [prompt]`
- 型付きフィールドと重複した場合の挙動は CLI 依存（ドキュメントで警告）

## テスト方針

既存テスト7つは変更不要。以下を追加:

| テスト | 検証内容 |
|-------|---------|
| `default_uses_minimal_context` | `None` 時にコンテキスト最小化デフォルトが入ること |
| `override_tools` | `tools` で `--tools` のデフォルト値が上書きされること |
| `override_no_session_persistence_false` | `Some(false)` でフラグが消えること |
| `override_setting_sources` | デフォルト `""` が上書きされること |
| `override_mcp_config` | デフォルト `'{}'` が上書きされること |
| `override_disable_slash_commands_false` | フラグが消えること |
| `override_strict_mcp_config_false` | フラグが消えること |
| `effort_with_constant` | `effort(effort::HIGH)` → `--effort high` |
| `effort_with_custom_string` | `effort("ultra")` → `--effort ultra` |
| `allowed_tools_multiple` | 複数ツールの引数生成 |
| `extra_args_before_prompt` | `extra_args` がプロンプト直前に入ること |
| `extra_args_with_typed_fields` | 型付きフィールドと共存すること |
| `all_new_fields_in_builder` | 全新規フィールドが Builder で設定できること |
| `bare_flag` | `bare(true)` → `--bare` |
| `dangerously_skip_permissions_flag` | 同上 |
| `resume_session` | `resume("id")` → `--resume id` |
| `session_id_field` | `session_id("uuid")` → `--session-id uuid` |
| `json_schema_field` | 引数生成 |
| `add_dir_multiple` | 複数ディレクトリの引数生成 |
| `file_multiple` | 複数ファイルの引数生成 |

## 公開 API の変更

- `lib.rs` に `pub use config::{effort, permission_mode}` を追加
- 破壊的変更: **ゼロ**
