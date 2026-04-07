use std::time::Duration;

/// Conditional tracing macro for warnings.
macro_rules! trace_warn {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::warn!($($arg)*);
    };
}

/// Default CLI command name.
const DEFAULT_CLI_PATH: &str = "claude";

/// Preset defines the base set of CLI flags injected before
/// builder attributes and `extra_args`.
///
/// # Examples
///
/// ```
/// use claude_code::Preset;
///
/// // Reusable custom preset
/// let my_preset = Preset::Custom(vec![
///     "--print".into(),
///     "--no-session-persistence".into(),
/// ]);
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub enum Preset {
    /// All context-minimization defaults (current behavior).
    ///
    /// Injects: `--print`, `--no-session-persistence`, `--strict-mcp-config`,
    /// `--disable-slash-commands`, `--setting-sources ""`, `--mcp-config '{}'`,
    /// `--tools ""`, `--system-prompt ""`.
    #[default]
    Normal,

    /// Only flags required for the library's parsing to work.
    ///
    /// Injects: `--print`. Format flags (`--output-format`, `--verbose`)
    /// are added by `to_args()` / `to_stream_args()` regardless of preset.
    Minimal,

    /// No auto-injected flags. User has full control via builder attributes
    /// and `extra_args`.
    Bare,

    /// User-defined base args. These are injected before builder attributes
    /// and `extra_args`.
    Custom(Vec<String>),
}

/// Configuration options for Claude CLI execution.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ClaudeConfig {
    /// Preset that determines the base set of auto-injected CLI flags.
    /// Defaults to [`Preset::Normal`].
    pub preset: Preset,
    /// Path to the `claude` CLI binary. Defaults to `"claude"` (resolved via `PATH`).
    ///
    /// Use this to specify an absolute path when the binary is not on `PATH`,
    /// or to select a specific version of the CLI.
    ///
    /// # Security
    ///
    /// No validation is performed on this value. `tokio::process::Command::new()`
    /// invokes `execvp` directly without a shell, so shell injection is not possible.
    pub cli_path: Option<String>,
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
    /// Idle timeout for streams. If no event arrives within this duration,
    /// the stream yields [`ClaudeError::Timeout`](crate::ClaudeError::Timeout)
    /// and terminates. Library-only; not a CLI flag.
    pub stream_idle_timeout: Option<Duration>,
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
    /// Returns the CLI binary path, defaulting to `"claude"`.
    #[must_use]
    pub fn cli_path_or_default(&self) -> &str {
        self.cli_path.as_deref().unwrap_or(DEFAULT_CLI_PATH)
    }

    /// Returns a new builder.
    #[must_use]
    pub fn builder() -> ClaudeConfigBuilder {
        ClaudeConfigBuilder::default()
    }

    /// Creates a builder pre-filled with this configuration's values.
    #[must_use]
    pub fn to_builder(&self) -> ClaudeConfigBuilder {
        ClaudeConfigBuilder {
            preset: self.preset.clone(),
            cli_path: self.cli_path.clone(),
            model: self.model.clone(),
            system_prompt: self.system_prompt.clone(),
            append_system_prompt: self.append_system_prompt.clone(),
            max_turns: self.max_turns,
            timeout: self.timeout,
            stream_idle_timeout: self.stream_idle_timeout,
            fallback_model: self.fallback_model.clone(),
            effort: self.effort.clone(),
            max_budget_usd: self.max_budget_usd,
            allowed_tools: self.allowed_tools.clone(),
            disallowed_tools: self.disallowed_tools.clone(),
            tools: self.tools.clone(),
            mcp_config: self.mcp_config.clone(),
            setting_sources: self.setting_sources.clone(),
            settings: self.settings.clone(),
            json_schema: self.json_schema.clone(),
            include_partial_messages: self.include_partial_messages,
            include_hook_events: self.include_hook_events,
            permission_mode: self.permission_mode.clone(),
            dangerously_skip_permissions: self.dangerously_skip_permissions,
            add_dir: self.add_dir.clone(),
            file: self.file.clone(),
            resume: self.resume.clone(),
            session_id: self.session_id.clone(),
            bare: self.bare,
            no_session_persistence: self.no_session_persistence,
            disable_slash_commands: self.disable_slash_commands,
            strict_mcp_config: self.strict_mcp_config,
            extra_args: self.extra_args.clone(),
        }
    }

    /// Builds common CLI arguments shared by JSON and stream-json modes.
    ///
    /// Argument generation priority:
    /// 1. Preset base args
    /// 2. Builder attributes
    /// 3. `--output-format` / `--verbose` (added by `to_args()` / `to_stream_args()`)
    /// 4. `extra_args`
    /// 5. prompt (added by `to_args()` / `to_stream_args()`)
    fn base_args(&self) -> Vec<String> {
        let mut args = self.preset_args();
        self.push_builder_attrs(&mut args);
        args
    }

    /// Returns the base flags determined by the preset.
    fn preset_args(&self) -> Vec<String> {
        match &self.preset {
            Preset::Normal => self.normal_preset_args(),
            Preset::Minimal => self.minimal_preset_args(),
            Preset::Bare => Vec::new(),
            Preset::Custom(custom_args) => self.filtered_custom_args(custom_args),
        }
    }

    /// Normal preset: all context-minimization defaults.
    fn normal_preset_args(&self) -> Vec<String> {
        let mut args = vec!["--print".into()];

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

        args
    }

    /// Minimal preset: only `--print`.
    fn minimal_preset_args(&self) -> Vec<String> {
        vec!["--print".into()]
    }

    /// Filters a custom preset's args by removing flags that builder attributes
    /// explicitly disabled (e.g., `no_session_persistence == Some(false)`).
    fn filtered_custom_args(&self, custom_args: &[String]) -> Vec<String> {
        custom_args
            .iter()
            .filter(|arg| {
                let s = arg.as_str();
                !(self.no_session_persistence == Some(false) && s == "--no-session-persistence"
                    || self.strict_mcp_config == Some(false) && s == "--strict-mcp-config"
                    || self.disable_slash_commands == Some(false)
                        && s == "--disable-slash-commands")
            })
            .cloned()
            .collect()
    }

    /// Appends builder-attribute flags to the args vector.
    ///
    /// For Normal preset, context-minimization flags are already in the preset
    /// args with their defaults. For other presets, these flags are only added
    /// when explicitly set by the user.
    fn push_builder_attrs(&self, args: &mut Vec<String>) {
        let is_normal = matches!(self.preset, Preset::Normal);

        // --- Context-minimization flags (preset-aware) ---
        // For Normal: handled in normal_preset_args() with defaults.
        // For others: only when explicitly set.
        if !is_normal {
            if self.no_session_persistence == Some(true) {
                args.push("--no-session-persistence".into());
            }

            if let Some(ref val) = self.setting_sources {
                args.push("--setting-sources".into());
                args.push(val.clone());
            }

            if self.strict_mcp_config == Some(true) {
                args.push("--strict-mcp-config".into());
            }

            if !self.mcp_config.is_empty() {
                for cfg in &self.mcp_config {
                    args.push("--mcp-config".into());
                    args.push(cfg.clone());
                }
            }

            if let Some(ref val) = self.tools {
                args.push("--tools".into());
                args.push(val.clone());
            }

            if self.disable_slash_commands == Some(true) {
                args.push("--disable-slash-commands".into());
            }
        }

        // system_prompt: Normal defaults to "" (empty); other presets only when set
        if is_normal {
            args.push("--system-prompt".into());
            args.push(self.system_prompt.clone().unwrap_or_default());
        } else if let Some(ref val) = self.system_prompt {
            args.push("--system-prompt".into());
            args.push(val.clone());
        }

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
    }

    /// Builds command-line arguments for JSON output mode.
    ///
    /// Includes fixed options such as `--print --output-format json`.
    #[must_use]
    pub fn to_args(&self, prompt: &str) -> Vec<String> {
        let mut args = self.base_args();
        args.push("--output-format".into());
        args.push("json".into());
        args.extend(self.extra_args.iter().cloned());
        self.warn_if_no_print(&args);
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
        args.extend(self.extra_args.iter().cloned());
        self.warn_if_no_print(&args);
        args.push(prompt.into());
        args
    }

    /// Emits a tracing warning if `--print` / `-p` is not in the final args.
    fn warn_if_no_print(&self, args: &[String]) {
        if !args.iter().any(|a| a == "--print" || a == "-p") {
            trace_warn!(
                "args do not contain --print; the CLI may start in interactive mode and hang"
            );
        }
    }
}

/// Builder for [`ClaudeConfig`].
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfigBuilder {
    preset: Preset,
    cli_path: Option<String>,
    model: Option<String>,
    system_prompt: Option<String>,
    append_system_prompt: Option<String>,
    max_turns: Option<u32>,
    timeout: Option<Duration>,
    stream_idle_timeout: Option<Duration>,
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
    /// Sets the preset that determines which CLI flags are auto-injected.
    ///
    /// Defaults to [`Preset::Normal`].
    #[must_use]
    pub fn preset(mut self, preset: Preset) -> Self {
        self.preset = preset;
        self
    }

    /// Sets the path to the `claude` CLI binary.
    ///
    /// When not set, `"claude"` is resolved via `PATH`.
    #[must_use]
    pub fn cli_path(mut self, path: impl Into<String>) -> Self {
        self.cli_path = Some(path.into());
        self
    }

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

    /// Sets the idle timeout for streams.
    ///
    /// If no event arrives within this duration, the stream yields
    /// [`ClaudeError::Timeout`](crate::ClaudeError::Timeout) and terminates.
    #[must_use]
    pub fn stream_idle_timeout(mut self, timeout: Duration) -> Self {
        self.stream_idle_timeout = Some(timeout);
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
            preset: self.preset,
            cli_path: self.cli_path,
            model: self.model,
            system_prompt: self.system_prompt,
            append_system_prompt: self.append_system_prompt,
            max_turns: self.max_turns,
            timeout: self.timeout,
            stream_idle_timeout: self.stream_idle_timeout,
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
        assert!(config.cli_path.is_none());
        assert!(config.model.is_none());
        assert!(config.system_prompt.is_none());
        assert!(config.max_turns.is_none());
        assert!(config.timeout.is_none());
    }

    #[test]
    fn cli_path_or_default_returns_claude_when_none() {
        let config = ClaudeConfig::default();
        assert_eq!(config.cli_path_or_default(), "claude");
    }

    #[test]
    fn cli_path_or_default_returns_custom_path() {
        let config = ClaudeConfig::builder()
            .cli_path("/usr/local/bin/claude-v2")
            .build();
        assert_eq!(config.cli_path_or_default(), "/usr/local/bin/claude-v2");
    }

    #[test]
    fn builder_sets_cli_path() {
        let config = ClaudeConfig::builder().cli_path("/opt/claude").build();
        assert_eq!(config.cli_path.as_deref(), Some("/opt/claude"));
    }

    #[test]
    fn builder_sets_stream_idle_timeout() {
        let config = ClaudeConfig::builder()
            .stream_idle_timeout(Duration::from_secs(60))
            .build();
        assert_eq!(config.stream_idle_timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn default_stream_idle_timeout_is_none() {
        let config = ClaudeConfig::default();
        assert!(config.stream_idle_timeout.is_none());
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
        let config = ClaudeConfig::builder().strict_mcp_config(false).build();
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

    #[test]
    fn disallowed_tools_multiple() {
        let config = ClaudeConfig::builder()
            .disallowed_tools(["Write", "Bash"])
            .build();
        let args = config.to_args("test");
        let idx = args.iter().position(|a| a == "--disallowedTools").unwrap();
        assert_eq!(args[idx + 1], "Write");
        assert_eq!(args[idx + 2], "Bash");
    }

    #[test]
    fn to_builder_round_trip_fields() {
        let original = ClaudeConfig::builder()
            .cli_path("/custom/claude")
            .model("haiku")
            .system_prompt("test")
            .max_turns(5)
            .timeout(Duration::from_secs(30))
            .stream_idle_timeout(Duration::from_secs(45))
            .no_session_persistence(false)
            .resume("session-123")
            .build();

        let rebuilt = original.to_builder().build();

        assert_eq!(rebuilt.cli_path, original.cli_path);
        assert_eq!(rebuilt.model, original.model);
        assert_eq!(rebuilt.system_prompt, original.system_prompt);
        assert_eq!(rebuilt.max_turns, original.max_turns);
        assert_eq!(rebuilt.timeout, original.timeout);
        assert_eq!(rebuilt.stream_idle_timeout, original.stream_idle_timeout);
        assert_eq!(
            rebuilt.no_session_persistence,
            original.no_session_persistence
        );
        assert_eq!(rebuilt.resume, original.resume);
    }

    #[test]
    fn to_builder_round_trip_args() {
        let config = ClaudeConfig::builder()
            .model("haiku")
            .max_turns(3)
            .effort("high")
            .allowed_tools(["Bash", "Read"])
            .no_session_persistence(false)
            .build();

        let rebuilt = config.to_builder().build();
        assert_eq!(config.to_args("hi"), rebuilt.to_args("hi"));
    }

    // --- Preset tests ---

    #[test]
    fn default_preset_is_normal() {
        let config = ClaudeConfig::default();
        assert_eq!(config.preset, Preset::Normal);
    }

    #[test]
    fn builder_default_preset_is_normal() {
        let config = ClaudeConfig::builder().build();
        assert_eq!(config.preset, Preset::Normal);
    }

    #[test]
    fn explicit_normal_preset_matches_default_to_args() {
        let default_config = ClaudeConfig::default();
        let explicit_config = ClaudeConfig::builder().preset(Preset::Normal).build();
        assert_eq!(
            default_config.to_args("test"),
            explicit_config.to_args("test")
        );
    }

    #[test]
    fn explicit_normal_preset_matches_default_to_stream_args() {
        let default_config = ClaudeConfig::default();
        let explicit_config = ClaudeConfig::builder().preset(Preset::Normal).build();
        assert_eq!(
            default_config.to_stream_args("test"),
            explicit_config.to_stream_args("test")
        );
    }

    #[test]
    fn to_builder_preserves_preset() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Minimal)
            .model("haiku")
            .build();
        let rebuilt = config.to_builder().build();
        assert_eq!(rebuilt.preset, Preset::Minimal);
    }

    #[test]
    fn minimal_preset_to_args() {
        let config = ClaudeConfig::builder().preset(Preset::Minimal).build();
        let args = config.to_args("test");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"json".to_string()));
        assert_eq!(args.last().unwrap(), "test");

        // Context-minimization flags must NOT be present
        assert!(!args.contains(&"--no-session-persistence".to_string()));
        assert!(!args.contains(&"--strict-mcp-config".to_string()));
        assert!(!args.contains(&"--disable-slash-commands".to_string()));
        assert!(!args.contains(&"--setting-sources".to_string()));
        assert!(!args.contains(&"--mcp-config".to_string()));
        assert!(!args.contains(&"--tools".to_string()));
        assert!(!args.contains(&"--system-prompt".to_string()));
    }

    #[test]
    fn minimal_preset_to_stream_args() {
        let config = ClaudeConfig::builder().preset(Preset::Minimal).build();
        let args = config.to_stream_args("test");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert_eq!(args.last().unwrap(), "test");

        assert!(!args.contains(&"--no-session-persistence".to_string()));
        assert!(!args.contains(&"--system-prompt".to_string()));
    }

    #[test]
    fn minimal_preset_with_builder_add() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Minimal)
            .no_session_persistence(true)
            .model("haiku")
            .build();
        let args = config.to_args("test");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--model".to_string()));
    }

    #[test]
    fn minimal_preset_with_system_prompt() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Minimal)
            .system_prompt("Be helpful")
            .build();
        let args = config.to_args("test");

        let idx = args.iter().position(|a| a == "--system-prompt").unwrap();
        assert_eq!(args[idx + 1], "Be helpful");
    }

    #[test]
    fn bare_preset_to_args() {
        let config = ClaudeConfig::builder().preset(Preset::Bare).build();
        let args = config.to_args("test");

        // Only --output-format json and prompt
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"json".to_string()));
        assert_eq!(args.last().unwrap(), "test");

        // No preset flags at all
        assert!(!args.contains(&"--print".to_string()));
        assert!(!args.contains(&"--no-session-persistence".to_string()));
        assert!(!args.contains(&"--strict-mcp-config".to_string()));
        assert!(!args.contains(&"--disable-slash-commands".to_string()));
        assert!(!args.contains(&"--setting-sources".to_string()));
        assert!(!args.contains(&"--mcp-config".to_string()));
        assert!(!args.contains(&"--tools".to_string()));
        assert!(!args.contains(&"--system-prompt".to_string()));
    }

    #[test]
    fn bare_preset_to_stream_args() {
        let config = ClaudeConfig::builder().preset(Preset::Bare).build();
        let args = config.to_stream_args("test");

        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert_eq!(args.last().unwrap(), "test");

        assert!(!args.contains(&"--print".to_string()));
    }

    #[test]
    fn bare_preset_with_extra_args() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Bare)
            .extra_args(["--print", "--cli-mode"])
            .build();
        let args = config.to_args("test");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--cli-mode".to_string()));
    }

    #[test]
    fn extra_args_after_format() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Bare)
            .extra_args(["--new-flag"])
            .build();
        let args = config.to_args("test");

        // extra_args should appear after --output-format (for last-wins override)
        let format_idx = args.iter().position(|a| a == "--output-format").unwrap();
        let flag_idx = args.iter().position(|a| a == "--new-flag").unwrap();
        assert!(flag_idx > format_idx);
    }

    // --- Custom preset tests ---

    #[test]
    fn custom_preset_to_args() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Custom(vec![
                "--print".into(),
                "--no-session-persistence".into(),
            ]))
            .build();
        let args = config.to_args("test");

        assert!(args.contains(&"--print".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--output-format".to_string()));
        assert_eq!(args.last().unwrap(), "test");

        // Flags NOT in the custom preset should be absent
        assert!(!args.contains(&"--strict-mcp-config".to_string()));
        assert!(!args.contains(&"--disable-slash-commands".to_string()));
    }

    #[test]
    fn custom_preset_is_reusable() {
        let preset = Preset::Custom(vec!["--print".into(), "--no-session-persistence".into()]);

        let config1 = ClaudeConfig::builder()
            .preset(preset.clone())
            .model("haiku")
            .build();
        let config2 = ClaudeConfig::builder()
            .preset(preset)
            .model("sonnet")
            .build();

        let args1 = config1.to_args("test");
        let args2 = config2.to_args("test");

        // Both should have the preset flags
        assert!(args1.contains(&"--print".to_string()));
        assert!(args2.contains(&"--print".to_string()));
        assert!(args1.contains(&"--no-session-persistence".to_string()));
        assert!(args2.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn custom_preset_builder_override_remove() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Custom(vec![
                "--print".into(),
                "--no-session-persistence".into(),
            ]))
            .no_session_persistence(false) // remove from custom preset
            .build();
        let args = config.to_args("test");

        assert!(args.contains(&"--print".to_string()));
        assert!(!args.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn custom_preset_builder_override_remove_strict_mcp() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Custom(vec![
                "--print".into(),
                "--strict-mcp-config".into(),
            ]))
            .strict_mcp_config(false)
            .build();
        let args = config.to_args("test");

        assert!(!args.contains(&"--strict-mcp-config".to_string()));
    }

    #[test]
    fn custom_preset_builder_override_remove_disable_slash_commands() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Custom(vec![
                "--print".into(),
                "--disable-slash-commands".into(),
            ]))
            .disable_slash_commands(false)
            .build();
        let args = config.to_args("test");

        assert!(!args.contains(&"--disable-slash-commands".to_string()));
    }

    // --- Priority override tests ---

    #[test]
    fn priority_extra_args_appended_last() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Normal)
            .extra_args(["--new-flag"])
            .build();
        let args = config.to_args("test");

        let prompt_idx = args.iter().position(|a| a == "test").unwrap();
        let flag_idx = args.iter().position(|a| a == "--new-flag").unwrap();
        assert!(flag_idx < prompt_idx);
        assert!(flag_idx > 0); // not the first arg
    }

    #[test]
    fn priority_extra_args_overrides_format() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Normal)
            .extra_args(["--output-format", "new"])
            .build();
        let args = config.to_args("test");

        // Both the library-injected and user-specified --output-format present
        let format_positions: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--output-format")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(format_positions.len(), 2);
        // User-specified comes after library-injected
        assert!(format_positions[1] > format_positions[0]);
    }

    #[test]
    fn priority_full_stack() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Minimal)
            .model("haiku")
            .extra_args(["--model", "sonnet"])
            .build();
        let args = config.to_args("test");

        // Both --model values present
        let model_positions: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--model")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(model_positions.len(), 2);
        // haiku (builder attr) before sonnet (extra_args)
        assert_eq!(args[model_positions[0] + 1], "haiku");
        assert_eq!(args[model_positions[1] + 1], "sonnet");
    }

    // --- Boolean flag override tests ---

    #[test]
    fn bool_flag_none_follows_normal_preset() {
        // Normal includes these flags by default when None
        let config = ClaudeConfig::builder().preset(Preset::Normal).build();
        let args = config.to_args("test");
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert!(args.contains(&"--disable-slash-commands".to_string()));
    }

    #[test]
    fn bool_flag_none_follows_minimal_preset() {
        // Minimal does NOT include these flags when None
        let config = ClaudeConfig::builder().preset(Preset::Minimal).build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--no-session-persistence".to_string()));
        assert!(!args.contains(&"--strict-mcp-config".to_string()));
        assert!(!args.contains(&"--disable-slash-commands".to_string()));
    }

    #[test]
    fn bool_flag_true_adds_to_minimal() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Minimal)
            .no_session_persistence(true)
            .strict_mcp_config(true)
            .disable_slash_commands(true)
            .build();
        let args = config.to_args("test");
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert!(args.contains(&"--disable-slash-commands".to_string()));
    }

    #[test]
    fn bool_flag_false_removes_from_normal() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Normal)
            .no_session_persistence(false)
            .strict_mcp_config(false)
            .disable_slash_commands(false)
            .build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--no-session-persistence".to_string()));
        assert!(!args.contains(&"--strict-mcp-config".to_string()));
        assert!(!args.contains(&"--disable-slash-commands".to_string()));
    }

    // --- Hang protection: --print presence tests ---

    #[test]
    fn normal_preset_contains_print() {
        let config = ClaudeConfig::builder().preset(Preset::Normal).build();
        let args = config.to_args("test");
        assert!(args.contains(&"--print".to_string()));
    }

    #[test]
    fn minimal_preset_contains_print() {
        let config = ClaudeConfig::builder().preset(Preset::Minimal).build();
        let args = config.to_args("test");
        assert!(args.contains(&"--print".to_string()));
    }

    #[test]
    fn bare_preset_no_print() {
        let config = ClaudeConfig::builder().preset(Preset::Bare).build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--print".to_string()));
    }

    #[test]
    fn bare_preset_with_print_in_extra_args() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Bare)
            .extra_args(["-p"])
            .build();
        let args = config.to_args("test");
        assert!(args.contains(&"-p".to_string()));
    }

    #[test]
    fn custom_preset_without_print() {
        let config = ClaudeConfig::builder()
            .preset(Preset::Custom(vec!["--no-session-persistence".into()]))
            .build();
        let args = config.to_args("test");
        assert!(!args.contains(&"--print".to_string()));
    }

    #[test]
    fn warn_if_no_print_does_not_panic() {
        // Verify warn_if_no_print doesn't panic or cause issues
        // when --print is absent (Bare preset)
        let config = ClaudeConfig::builder().preset(Preset::Bare).build();
        let _args = config.to_args("test");
        let _stream_args = config.to_stream_args("test");
    }
}
