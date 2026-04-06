use std::time::Duration;

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

impl ClaudeConfig {
    /// Returns a new builder.
    #[must_use]
    pub fn builder() -> ClaudeConfigBuilder {
        ClaudeConfigBuilder::default()
    }

    /// Builds common CLI arguments shared by JSON and stream-json modes.
    fn base_args(&self) -> Vec<String> {
        let mut args = vec![
            "--print".into(),
            "--no-session-persistence".into(),
            "--setting-sources".into(),
            String::new(),
            "--strict-mcp-config".into(),
            "--mcp-config".into(),
            r#"{"mcpServers":{}}"#.into(),
            "--tools".into(),
            String::new(),
            "--disable-slash-commands".into(),
            "--system-prompt".into(),
        ];

        match &self.system_prompt {
            Some(sp) => args.push(sp.clone()),
            None => args.push(String::new()),
        }

        if let Some(model) = &self.model {
            args.push("--model".into());
            args.push(model.clone());
        }

        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".into());
            args.push(max_turns.to_string());
        }

        args
    }

    /// Builds command-line arguments for JSON output mode.
    ///
    /// Includes fixed options such as `--print --output-format json`.
    #[must_use]
    pub fn to_args(&self, prompt: &str) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--output-format".into());
        args.push("json".into());
        args.push(prompt.into());
        args
    }

    /// Builds command-line arguments for stream-json output mode.
    ///
    /// Includes `--verbose` (required for stream-json) and optionally
    /// `--include-partial-messages`.
    #[must_use]
    pub fn to_stream_args(&self, prompt: &str) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--output-format".into());
        args.push("stream-json".into());
        args.push("--verbose".into());

        if self.include_partial_messages == Some(true) {
            args.push("--include-partial-messages".into());
        }

        args.push(prompt.into());
        args
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ClaudeConfig::default();
        assert!(config.model.is_none());
        assert!(config.system_prompt.is_none());
        assert!(config.max_turns.is_none());
        assert!(config.timeout.is_none());
    }

    #[test]
    fn builder_sets_all_fields() {
        let config = ClaudeConfig::builder()
            .model("haiku")
            .system_prompt("You are helpful")
            .max_turns(3)
            .timeout(Duration::from_secs(30))
            .build();

        assert_eq!(config.model.as_deref(), Some("haiku"));
        assert_eq!(config.system_prompt.as_deref(), Some("You are helpful"));
        assert_eq!(config.max_turns, Some(3));
        assert_eq!(config.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn to_args_minimal() {
        let config = ClaudeConfig::default();
        let args = config.to_args("hello");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"json".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--disable-slash-commands".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        // system-prompt defaults to empty string
        let sp_idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[sp_idx + 1], "");
        // model, max-turns should not be present
        assert!(!args.contains(&"--model".to_string()));
        assert!(!args.contains(&"--max-turns".to_string()));
        // prompt should be the last argument
        assert_eq!(args.last().unwrap(), "hello");
    }

    #[test]
    fn to_args_with_options() {
        let config = ClaudeConfig::builder()
            .model("haiku")
            .system_prompt("Be concise")
            .max_turns(5)
            .build();
        let args = config.to_args("test prompt");

        let model_idx = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[model_idx + 1], "haiku");

        let sp_idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[sp_idx + 1], "Be concise");

        let mt_idx = args.iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[mt_idx + 1], "5");

        assert_eq!(args.last().unwrap(), "test prompt");
    }

    #[test]
    fn to_stream_args_minimal() {
        let config = ClaudeConfig::default();
        let args = config.to_stream_args("hello");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(!args.contains(&"json".to_string()));
        assert!(!args.contains(&"--include-partial-messages".to_string()));
        assert_eq!(args.last().unwrap(), "hello");
    }

    #[test]
    fn to_stream_args_with_partial_messages() {
        let config = ClaudeConfig::builder()
            .include_partial_messages(true)
            .build();
        let args = config.to_stream_args("hello");

        assert!(args.contains(&"--include-partial-messages".to_string()));
    }

    #[test]
    fn builder_sets_include_partial_messages() {
        let config = ClaudeConfig::builder()
            .include_partial_messages(true)
            .build();
        assert_eq!(config.include_partial_messages, Some(true));
    }

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

        assert_eq!(
            config.append_system_prompt.as_deref(),
            Some("extra context")
        );
        assert_eq!(config.fallback_model.as_deref(), Some("haiku"));
        assert_eq!(config.effort.as_deref(), Some("high"));
        assert_eq!(config.max_budget_usd, Some(1.0));
        assert_eq!(config.allowed_tools, vec!["Bash", "Edit"]);
        assert_eq!(config.disallowed_tools, vec!["Write"]);
        assert_eq!(config.tools.as_deref(), Some("Bash,Edit"));
        assert_eq!(config.mcp_config, vec!["config.json"]);
        assert_eq!(config.setting_sources.as_deref(), Some("user,project"));
        assert_eq!(config.settings.as_deref(), Some("settings.json"));
        assert_eq!(config.json_schema.as_deref(), Some(r#"{"type":"object"}"#));
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
}
