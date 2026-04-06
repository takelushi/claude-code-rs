# ClaudeConfig 拡張 実装プラン

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ClaudeConfig` を拡張し、`claude -p` の全オプションに型付きフィールドで対応する。未知のオプション用に `extra_args` エスケープハッチも提供する。

**Architecture:** 既存の `ClaudeConfig` にフラットにフィールドを追加。`#[non_exhaustive]` により non-breaking change。`base_args()` をリライトし、`None`/空Vec はコンテキスト最小化デフォルトを維持、`Some`/非空Vec で上書き可能にする。

**Tech Stack:** Rust 1.93+, tokio, serde, Builder パターン

**Spec:** `docs/superpowers/specs/2026-04-06-config-expansion-design.md`

---

### Task 1: 定数モジュール + re-export

**Files:**
- Modify: `src/config.rs` (末尾に追加)
- Modify: `src/lib.rs:8`

- [ ] **Step 1: `src/config.rs` 末尾に定数モジュールを追加**

`src/config.rs` の `#[cfg(test)] mod tests` ブロックの**直前**に以下を追加:

```rust
/// Known values for the `--effort` CLI option.
pub mod effort {
    /// Low effort.
    pub const LOW: &str = "low";
    /// Medium effort.
    pub const MEDIUM: &str = "medium";
    /// High effort.
    pub const HIGH: &str = "high";
    /// Maximum effort.
    pub const MAX: &str = "max";
}

/// Known values for the `--permission-mode` CLI option.
pub mod permission_mode {
    /// Default permission mode.
    pub const DEFAULT: &str = "default";
    /// Accept edits without confirmation.
    pub const ACCEPT_EDITS: &str = "acceptEdits";
    /// Automatic permission handling.
    pub const AUTO: &str = "auto";
    /// Bypass all permission checks.
    pub const BYPASS_PERMISSIONS: &str = "bypassPermissions";
    /// Never ask for permission.
    pub const DONT_ASK: &str = "dontAsk";
    /// Plan mode.
    pub const PLAN: &str = "plan";
}
```

- [ ] **Step 2: `src/lib.rs` に re-export を追加**

`src/lib.rs` の `pub use config::` 行を以下に変更:

```rust
pub use config::{effort, permission_mode, ClaudeConfig, ClaudeConfigBuilder};
```

- [ ] **Step 3: ビルド確認 + コミット**

Run: `cargo build`
Expected: 成功

```bash
git add src/config.rs src/lib.rs
git commit -m "feat: effort / permission_mode 定数モジュールを追加"
```

---

### Task 2: ClaudeConfig + ClaudeConfigBuilder フィールド拡張

**Files:**
- Modify: `src/config.rs:1-148` (struct + builder + build)

- [ ] **Step 1: テストを書く（Red）**

`src/config.rs` の `mod tests` ブロック末尾に以下のテストを追加:

```rust
    #[test]
    fn all_new_fields_in_builder() {
        let config = ClaudeConfig::builder()
            .append_system_prompt("extra context")
            .fallback_model("haiku")
            .effort("high")
            .max_budget_usd(1.0)
            .allowed_tools(["Bash", "Edit"])
            .disallowed_tools(["Write"])
            .tools("Bash,Edit")
            .mcp_configs(["config.json"])
            .setting_sources("user,project")
            .settings("settings.json")
            .json_schema(r#"{"type":"object"}"#)
            .include_hook_events(true)
            .permission_mode("auto")
            .dangerously_skip_permissions(true)
            .add_dirs(["/path/a"])
            .file("spec:file.txt")
            .resume("session-123")
            .session_id("uuid-456")
            .bare(true)
            .no_session_persistence(false)
            .disable_slash_commands(false)
            .strict_mcp_config(false)
            .extra_args(["--custom", "val"])
            .build();

        assert_eq!(config.append_system_prompt.as_deref(), Some("extra context"));
        assert_eq!(config.fallback_model.as_deref(), Some("haiku"));
        assert_eq!(config.effort.as_deref(), Some("high"));
        assert_eq!(config.max_budget_usd, Some(1.0));
        assert_eq!(config.allowed_tools, vec!["Bash", "Edit"]);
        assert_eq!(config.disallowed_tools, vec!["Write"]);
        assert_eq!(config.tools.as_deref(), Some("Bash,Edit"));
        assert_eq!(config.mcp_config, vec!["config.json"]);
        assert_eq!(config.setting_sources.as_deref(), Some("user,project"));
        assert_eq!(config.settings.as_deref(), Some("settings.json"));
        assert_eq!(
            config.json_schema.as_deref(),
            Some(r#"{"type":"object"}"#)
        );
        assert_eq!(config.include_hook_events, Some(true));
        assert_eq!(config.permission_mode.as_deref(), Some("auto"));
        assert_eq!(config.dangerously_skip_permissions, Some(true));
        assert_eq!(config.add_dir, vec!["/path/a"]);
        assert_eq!(config.file, vec!["spec:file.txt"]);
        assert_eq!(config.resume.as_deref(), Some("session-123"));
        assert_eq!(config.session_id.as_deref(), Some("uuid-456"));
        assert_eq!(config.bare, Some(true));
        assert_eq!(config.no_session_persistence, Some(false));
        assert_eq!(config.disable_slash_commands, Some(false));
        assert_eq!(config.strict_mcp_config, Some(false));
        assert_eq!(config.extra_args, vec!["--custom", "val"]);
    }
```

- [ ] **Step 2: テストが失敗する（コンパイルエラー）ことを確認**

Run: `cargo test --lib config::tests::all_new_fields_in_builder`
Expected: コンパイルエラー（`append_system_prompt` メソッドが存在しない等）

- [ ] **Step 3: `ClaudeConfig` 構造体にフィールドを追加**

`src/config.rs` の `ClaudeConfig` 構造体を以下に置換:

```rust
/// Configuration options for Claude CLI execution.
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfig {
    /// Model to use (`--model`).
    pub model: Option<String>,
    /// System prompt (`--system-prompt`). Defaults to empty string when `None`.
    pub system_prompt: Option<String>,
    /// Append to the default system prompt (`--append-system-prompt`).
    pub append_system_prompt: Option<String>,
    /// Maximum number of turns (`--max-turns`).
    pub max_turns: Option<u32>,
    /// Timeout duration. No timeout when `None`. Library-only; not a CLI flag.
    pub timeout: Option<Duration>,
    /// Fallback model when default is overloaded (`--fallback-model`).
    pub fallback_model: Option<String>,
    /// Effort level (`--effort`). Use [`effort`] constants for known values.
    pub effort: Option<String>,
    /// Maximum dollar amount for API calls (`--max-budget-usd`).
    pub max_budget_usd: Option<f64>,
    /// Tools to allow (`--allowedTools`).
    pub allowed_tools: Vec<String>,
    /// Tools to deny (`--disallowedTools`).
    pub disallowed_tools: Vec<String>,
    /// Built-in tool set override (`--tools`). Defaults to `""` (none) when `None`.
    pub tools: Option<String>,
    /// MCP server configs (`--mcp-config`). Defaults to `{"mcpServers":{}}` when empty.
    pub mcp_config: Vec<String>,
    /// Setting sources to load (`--setting-sources`). Defaults to `""` (none) when `None`.
    pub setting_sources: Option<String>,
    /// Path to settings file or JSON string (`--settings`).
    pub settings: Option<String>,
    /// JSON Schema for structured output (`--json-schema`).
    pub json_schema: Option<String>,
    /// Include partial message chunks in stream output (`--include-partial-messages`).
    pub include_partial_messages: Option<bool>,
    /// Include hook events in stream output (`--include-hook-events`).
    pub include_hook_events: Option<bool>,
    /// Permission mode (`--permission-mode`). Use [`permission_mode`] constants for known values.
    pub permission_mode: Option<String>,
    /// Bypass all permission checks (`--dangerously-skip-permissions`).
    pub dangerously_skip_permissions: Option<bool>,
    /// Additional directories for tool access (`--add-dir`).
    pub add_dir: Vec<String>,
    /// File resources to download at startup (`--file`).
    pub file: Vec<String>,
    /// Resume a conversation by session ID (`--resume`).
    pub resume: Option<String>,
    /// Use a specific session ID (`--session-id`).
    pub session_id: Option<String>,
    /// Minimal mode (`--bare`).
    pub bare: Option<bool>,
    /// Disable session persistence (`--no-session-persistence`). Enabled by default.
    pub no_session_persistence: Option<bool>,
    /// Disable all skills (`--disable-slash-commands`). Enabled by default.
    pub disable_slash_commands: Option<bool>,
    /// Only use MCP servers from `--mcp-config` (`--strict-mcp-config`). Enabled by default.
    pub strict_mcp_config: Option<bool>,
    /// Arbitrary CLI arguments for forward compatibility.
    ///
    /// Appended before the prompt. Use typed fields when available;
    /// duplicating a typed field here may cause unpredictable CLI behavior.
    pub extra_args: Vec<String>,
}
```

- [ ] **Step 4: `ClaudeConfigBuilder` にフィールド + メソッドを追加**

`ClaudeConfigBuilder` 構造体とその `impl` ブロックを以下に置換:

```rust
/// Builder for [`ClaudeConfig`].
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfigBuilder {
    model: Option<String>,
    system_prompt: Option<String>,
    append_system_prompt: Option<String>,
    max_turns: Option<u32>,
    timeout: Option<Duration>,
    fallback_model: Option<String>,
    effort: Option<String>,
    max_budget_usd: Option<f64>,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
    tools: Option<String>,
    mcp_config: Vec<String>,
    setting_sources: Option<String>,
    settings: Option<String>,
    json_schema: Option<String>,
    include_partial_messages: Option<bool>,
    include_hook_events: Option<bool>,
    permission_mode: Option<String>,
    dangerously_skip_permissions: Option<bool>,
    add_dir: Vec<String>,
    file: Vec<String>,
    resume: Option<String>,
    session_id: Option<String>,
    bare: Option<bool>,
    no_session_persistence: Option<bool>,
    disable_slash_commands: Option<bool>,
    strict_mcp_config: Option<bool>,
    extra_args: Vec<String>,
}

impl ClaudeConfigBuilder {
    /// Sets the model.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the system prompt.
    #[must_use]
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Sets the append system prompt.
    #[must_use]
    pub fn append_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.append_system_prompt = Some(prompt.into());
        self
    }

    /// Sets the maximum number of turns.
    #[must_use]
    pub fn max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = Some(max_turns);
        self
    }

    /// Sets the timeout duration.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the fallback model.
    #[must_use]
    pub fn fallback_model(mut self, model: impl Into<String>) -> Self {
        self.fallback_model = Some(model.into());
        self
    }

    /// Sets the effort level. See [`effort`] constants for known values.
    #[must_use]
    pub fn effort(mut self, effort: impl Into<String>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    /// Sets the maximum budget in USD.
    #[must_use]
    pub fn max_budget_usd(mut self, budget: f64) -> Self {
        self.max_budget_usd = Some(budget);
        self
    }

    /// Sets allowed tools (replaces any previous values).
    #[must_use]
    pub fn allowed_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single allowed tool.
    #[must_use]
    pub fn add_allowed_tool(mut self, tool: impl Into<String>) -> Self {
        self.allowed_tools.push(tool.into());
        self
    }

    /// Sets disallowed tools (replaces any previous values).
    #[must_use]
    pub fn disallowed_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.disallowed_tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single disallowed tool.
    #[must_use]
    pub fn add_disallowed_tool(mut self, tool: impl Into<String>) -> Self {
        self.disallowed_tools.push(tool.into());
        self
    }

    /// Sets the built-in tool set. `""` disables all, `"default"` enables all.
    #[must_use]
    pub fn tools(mut self, tools: impl Into<String>) -> Self {
        self.tools = Some(tools.into());
        self
    }

    /// Sets MCP server configs (replaces any previous values).
    #[must_use]
    pub fn mcp_configs(mut self, configs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.mcp_config = configs.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single MCP server config.
    #[must_use]
    pub fn add_mcp_config(mut self, config: impl Into<String>) -> Self {
        self.mcp_config.push(config.into());
        self
    }

    /// Sets the setting sources to load.
    #[must_use]
    pub fn setting_sources(mut self, sources: impl Into<String>) -> Self {
        self.setting_sources = Some(sources.into());
        self
    }

    /// Sets the path to a settings file or JSON string.
    #[must_use]
    pub fn settings(mut self, settings: impl Into<String>) -> Self {
        self.settings = Some(settings.into());
        self
    }

    /// Sets the JSON Schema for structured output.
    #[must_use]
    pub fn json_schema(mut self, schema: impl Into<String>) -> Self {
        self.json_schema = Some(schema.into());
        self
    }

    /// Enables or disables partial message chunks in stream output.
    #[must_use]
    pub fn include_partial_messages(mut self, enabled: bool) -> Self {
        self.include_partial_messages = Some(enabled);
        self
    }

    /// Enables or disables hook events in stream output.
    #[must_use]
    pub fn include_hook_events(mut self, enabled: bool) -> Self {
        self.include_hook_events = Some(enabled);
        self
    }

    /// Sets the permission mode. See [`permission_mode`] constants for known values.
    #[must_use]
    pub fn permission_mode(mut self, mode: impl Into<String>) -> Self {
        self.permission_mode = Some(mode.into());
        self
    }

    /// Enables or disables bypassing all permission checks.
    #[must_use]
    pub fn dangerously_skip_permissions(mut self, enabled: bool) -> Self {
        self.dangerously_skip_permissions = Some(enabled);
        self
    }

    /// Sets additional directories (replaces any previous values).
    #[must_use]
    pub fn add_dirs(mut self, dirs: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.add_dir = dirs.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single additional directory.
    #[must_use]
    pub fn add_dir(mut self, dir: impl Into<String>) -> Self {
        self.add_dir.push(dir.into());
        self
    }

    /// Sets file resources (replaces any previous values).
    #[must_use]
    pub fn files(mut self, files: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.file = files.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single file resource.
    #[must_use]
    pub fn file(mut self, file: impl Into<String>) -> Self {
        self.file.push(file.into());
        self
    }

    /// Sets the session ID to resume.
    #[must_use]
    pub fn resume(mut self, session_id: impl Into<String>) -> Self {
        self.resume = Some(session_id.into());
        self
    }

    /// Sets a specific session ID.
    #[must_use]
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = Some(id.into());
        self
    }

    /// Enables or disables bare/minimal mode.
    #[must_use]
    pub fn bare(mut self, enabled: bool) -> Self {
        self.bare = Some(enabled);
        self
    }

    /// Enables or disables session persistence.
    /// Enabled by default; set to `false` to allow session persistence.
    #[must_use]
    pub fn no_session_persistence(mut self, enabled: bool) -> Self {
        self.no_session_persistence = Some(enabled);
        self
    }

    /// Enables or disables slash commands.
    /// Disabled by default; set to `false` to enable slash commands.
    #[must_use]
    pub fn disable_slash_commands(mut self, enabled: bool) -> Self {
        self.disable_slash_commands = Some(enabled);
        self
    }

    /// Enables or disables strict MCP config mode.
    /// Enabled by default; set to `false` to allow non-`--mcp-config` MCP servers.
    #[must_use]
    pub fn strict_mcp_config(mut self, enabled: bool) -> Self {
        self.strict_mcp_config = Some(enabled);
        self
    }

    /// Sets arbitrary extra CLI arguments (replaces any previous values).
    ///
    /// These are appended before the prompt. Use typed fields when available;
    /// duplicating a typed field here may cause unpredictable CLI behavior.
    #[must_use]
    pub fn extra_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.extra_args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a single extra CLI argument.
    #[must_use]
    pub fn add_extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    /// Builds the [`ClaudeConfig`].
    #[must_use]
    pub fn build(self) -> ClaudeConfig {
        ClaudeConfig {
            model: self.model,
            system_prompt: self.system_prompt,
            append_system_prompt: self.append_system_prompt,
            max_turns: self.max_turns,
            timeout: self.timeout,
            fallback_model: self.fallback_model,
            effort: self.effort,
            max_budget_usd: self.max_budget_usd,
            allowed_tools: self.allowed_tools,
            disallowed_tools: self.disallowed_tools,
            tools: self.tools,
            mcp_config: self.mcp_config,
            setting_sources: self.setting_sources,
            settings: self.settings,
            json_schema: self.json_schema,
            include_partial_messages: self.include_partial_messages,
            include_hook_events: self.include_hook_events,
            permission_mode: self.permission_mode,
            dangerously_skip_permissions: self.dangerously_skip_permissions,
            add_dir: self.add_dir,
            file: self.file,
            resume: self.resume,
            session_id: self.session_id,
            bare: self.bare,
            no_session_persistence: self.no_session_persistence,
            disable_slash_commands: self.disable_slash_commands,
            strict_mcp_config: self.strict_mcp_config,
            extra_args: self.extra_args,
        }
    }
}
```

- [ ] **Step 5: テストが通ることを確認 + コミット**

Run: `cargo test --lib config::tests::all_new_fields_in_builder`
Expected: PASS

Run: `cargo test --lib config::tests`
Expected: 既存テスト8つ + 新規1つ = 全 PASS

```bash
git add src/config.rs
git commit -m "feat: ClaudeConfig に全 CLI オプションのフィールドを追加"
```

---

### Task 3: base_args() リライト + テスト

**Files:**
- Modify: `src/config.rs` (base_args, tests)

- [ ] **Step 1: テストを書く（Red）**

`src/config.rs` の `mod tests` ブロック末尾に以下を追加:

```rust
    #[test]
    fn default_uses_minimal_context() {
        let config = ClaudeConfig::default();
        let args = config.to_args("test");

        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert!(args.contains(&"--disable-slash-commands".to_string()));

        let ss_idx = args.iter().position(|a| a == "--setting-sources").unwrap();
        assert_eq!(args[ss_idx + 1], "");

        let mcp_idx = args.iter().position(|a| a == "--mcp-config").unwrap();
        assert_eq!(args[mcp_idx + 1], r#"{"mcpServers":{}}"#);

        let tools_idx = args.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(args[tools_idx + 1], "");
    }

    #[test]
    fn override_no_session_persistence_false() {
        let config = ClaudeConfig::builder()
            .no_session_persistence(false)
            .build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn override_strict_mcp_config_false() {
        let config = ClaudeConfig::builder()
            .strict_mcp_config(false)
            .build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--strict-mcp-config".to_string()));
    }

    #[test]
    fn override_disable_slash_commands_false() {
        let config = ClaudeConfig::builder()
            .disable_slash_commands(false)
            .build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--disable-slash-commands".to_string()));
    }

    #[test]
    fn override_tools() {
        let config = ClaudeConfig::builder().tools("Bash,Edit").build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(args[idx + 1], "Bash,Edit");
    }

    #[test]
    fn override_setting_sources() {
        let config = ClaudeConfig::builder()
            .setting_sources("user,project")
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--setting-sources").unwrap();
        assert_eq!(args[idx + 1], "user,project");
    }

    #[test]
    fn override_mcp_config() {
        let config = ClaudeConfig::builder()
            .mcp_configs(["path/config.json"])
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--mcp-config").unwrap();
        assert_eq!(args[idx + 1], "path/config.json");
        assert!(!args.contains(&r#"{"mcpServers":{}}"#.to_string()));
    }

    #[test]
    fn effort_with_constant() {
        let config = ClaudeConfig::builder().effort(effort::HIGH).build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(args[idx + 1], "high");
    }

    #[test]
    fn effort_with_custom_string() {
        let config = ClaudeConfig::builder().effort("ultra").build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(args[idx + 1], "ultra");
    }

    #[test]
    fn allowed_tools_multiple() {
        let config = ClaudeConfig::builder()
            .allowed_tools(["Bash(git:*)", "Edit", "Read"])
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--allowedTools").unwrap();
        assert_eq!(args[idx + 1], "Bash(git:*)");
        assert_eq!(args[idx + 2], "Edit");
        assert_eq!(args[idx + 3], "Read");
    }

    #[test]
    fn bare_flag() {
        let config = ClaudeConfig::builder().bare(true).build();
        let args = config.to_args("test");
        assert!(args.contains(&"--bare".to_string()));
    }

    #[test]
    fn dangerously_skip_permissions_flag() {
        let config = ClaudeConfig::builder()
            .dangerously_skip_permissions(true)
            .build();
        let args = config.to_args("test");
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn resume_session() {
        let config = ClaudeConfig::builder().resume("session-abc").build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[idx + 1], "session-abc");
    }

    #[test]
    fn session_id_field() {
        let config = ClaudeConfig::builder()
            .session_id("550e8400-e29b-41d4-a716-446655440000")
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--session-id").unwrap();
        assert_eq!(args[idx + 1], "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn json_schema_field() {
        let schema = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
        let config = ClaudeConfig::builder().json_schema(schema).build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--json-schema").unwrap();
        assert_eq!(args[idx + 1], schema);
    }

    #[test]
    fn add_dir_multiple() {
        let config = ClaudeConfig::builder()
            .add_dirs(["/path/a", "/path/b"])
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--add-dir").unwrap();
        assert_eq!(args[idx + 1], "/path/a");
        assert_eq!(args[idx + 2], "/path/b");
    }

    #[test]
    fn file_multiple() {
        let config = ClaudeConfig::builder()
            .files(["file_abc:doc.txt", "file_def:img.png"])
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--file").unwrap();
        assert_eq!(args[idx + 1], "file_abc:doc.txt");
        assert_eq!(args[idx + 2], "file_def:img.png");
    }

    #[test]
    fn extra_args_before_prompt() {
        let config = ClaudeConfig::builder()
            .extra_args(["--custom-flag", "value"])
            .build();
        let args = config.to_args("my prompt");
        let custom_idx = args.iter().position(|a| a == "--custom-flag").unwrap();
        let prompt_idx = args.iter().position(|a| a == "my prompt").unwrap();
        assert!(custom_idx < prompt_idx);
        assert_eq!(args[custom_idx + 1], "value");
    }

    #[test]
    fn extra_args_with_typed_fields() {
        let config = ClaudeConfig::builder()
            .model("sonnet")
            .extra_args(["--custom", "val"])
            .build();
        let args = config.to_args("test");
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"sonnet".to_string()));
        assert!(args.contains(&"--custom".to_string()));
        assert!(args.contains(&"val".to_string()));
    }
```

- [ ] **Step 2: テストが失敗することを確認**

Run: `cargo test --lib config::tests`
Expected: `default_uses_minimal_context` は PASS するが、override 系テストと新フィールド系テストは FAIL（base_args() が新フィールドを参照していないため）

- [ ] **Step 3: `base_args()` をリライト**

`src/config.rs` の `base_args()` メソッドを以下に置換:

```rust
    /// Builds common CLI arguments shared by JSON and stream-json modes.
    fn base_args(&self) -> Vec<String> {
        let mut args = vec!["--print".into()];

        // --- Context minimization defaults (overridable) ---

        // no_session_persistence: None → enabled, Some(false) → disabled
        if self.no_session_persistence != Some(false) {
            args.push("--no-session-persistence".into());
        }

        // setting_sources: None → "" (minimal), Some(val) → val
        args.push("--setting-sources".into());
        args.push(self.setting_sources.clone().unwrap_or_default());

        // strict_mcp_config: None → enabled, Some(false) → disabled
        if self.strict_mcp_config != Some(false) {
            args.push("--strict-mcp-config".into());
        }

        // mcp_config: [] → '{"mcpServers":{}}' (minimal), non-empty → user values
        if self.mcp_config.is_empty() {
            args.push("--mcp-config".into());
            args.push(r#"{"mcpServers":{}}"#.into());
        } else {
            for cfg in &self.mcp_config {
                args.push("--mcp-config".into());
                args.push(cfg.clone());
            }
        }

        // tools: None → "" (minimal), Some(val) → val
        args.push("--tools".into());
        args.push(self.tools.clone().unwrap_or_default());

        // disable_slash_commands: None → enabled, Some(false) → disabled
        if self.disable_slash_commands != Some(false) {
            args.push("--disable-slash-commands".into());
        }

        // --- Standard options ---

        args.push("--system-prompt".into());
        args.push(self.system_prompt.clone().unwrap_or_default());

        if let Some(ref val) = self.append_system_prompt {
            args.push("--append-system-prompt".into());
            args.push(val.clone());
        }

        if let Some(ref val) = self.model {
            args.push("--model".into());
            args.push(val.clone());
        }

        if let Some(ref val) = self.fallback_model {
            args.push("--fallback-model".into());
            args.push(val.clone());
        }

        if let Some(ref val) = self.effort {
            args.push("--effort".into());
            args.push(val.clone());
        }

        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".into());
            args.push(max_turns.to_string());
        }

        if let Some(budget) = self.max_budget_usd {
            args.push("--max-budget-usd".into());
            args.push(budget.to_string());
        }

        if !self.allowed_tools.is_empty() {
            args.push("--allowedTools".into());
            args.extend(self.allowed_tools.iter().cloned());
        }

        if !self.disallowed_tools.is_empty() {
            args.push("--disallowedTools".into());
            args.extend(self.disallowed_tools.iter().cloned());
        }

        if let Some(ref val) = self.settings {
            args.push("--settings".into());
            args.push(val.clone());
        }

        if let Some(ref val) = self.json_schema {
            args.push("--json-schema".into());
            args.push(val.clone());
        }

        if self.include_hook_events == Some(true) {
            args.push("--include-hook-events".into());
        }

        if let Some(ref val) = self.permission_mode {
            args.push("--permission-mode".into());
            args.push(val.clone());
        }

        if self.dangerously_skip_permissions == Some(true) {
            args.push("--dangerously-skip-permissions".into());
        }

        if !self.add_dir.is_empty() {
            args.push("--add-dir".into());
            args.extend(self.add_dir.iter().cloned());
        }

        if !self.file.is_empty() {
            args.push("--file".into());
            args.extend(self.file.iter().cloned());
        }

        if let Some(ref val) = self.resume {
            args.push("--resume".into());
            args.push(val.clone());
        }

        if let Some(ref val) = self.session_id {
            args.push("--session-id".into());
            args.push(val.clone());
        }

        if self.bare == Some(true) {
            args.push("--bare".into());
        }

        // extra_args: appended before prompt
        args.extend(self.extra_args.iter().cloned());

        args
    }
```

- [ ] **Step 4: 全テストが通ることを確認**

Run: `cargo test --lib config::tests`
Expected: 全 PASS（既存7 + Task2の1 + Task3の19 = 27テスト）

- [ ] **Step 5: clippy + fmt + コミット**

```bash
cargo clippy -- -D warnings
cargo fmt
git add src/config.rs
git commit -m "feat: base_args() を上書き可能なデフォルトにリライト"
```

---

### Task 4: ドキュメント更新 + 最終検証

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: 全テスト + clippy + fmt を実行**

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

Expected: 全 PASS、警告なし、フォーマット差分なし

- [ ] **Step 2: `CLAUDE.md` を更新**

Conventions セクションに以下を追記:

```markdown
- CLI オプションの値制限（`effort`, `permission_mode` 等）は enum ではなく `String` + 定数モジュールで表現する。Claude Code CLI は活発に開発されており、enum では新しい値の追加のたびにライブラリリリースが必要になるため
- ライブラリはオプション間の排他チェック・バリデーションを行わない。バリデーションの責務は CLI コマンド側にある
```

- [ ] **Step 3: コミット**

```bash
git add CLAUDE.md
git commit -m "docs: config 拡張の設計原則を CLAUDE.md に追記"
```

- [ ] **Step 4: 最終確認**

Run: `cargo doc --no-deps`
Expected: ドキュメント生成成功。`effort`, `permission_mode` モジュールと全 builder メソッドが表示される
