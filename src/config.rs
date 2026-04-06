use std::time::Duration;

/// Configuration options for Claude CLI execution.
#[derive(Debug, Clone, Default)]
pub struct ClaudeConfig {
    /// Model to use (`--model`).
    pub model: Option<String>,
    /// System prompt (`--system-prompt`). Defaults to empty string when `None`.
    pub system_prompt: Option<String>,
    /// Maximum number of turns (`--max-turns`).
    pub max_turns: Option<u32>,
    /// Timeout duration. No timeout when `None`.
    pub timeout: Option<Duration>,
    /// Include partial message chunks in stream output (`--include-partial-messages`).
    pub include_partial_messages: Option<bool>,
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
    max_turns: Option<u32>,
    timeout: Option<Duration>,
    include_partial_messages: Option<bool>,
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

    /// Enables or disables partial message chunks in stream output.
    #[must_use]
    pub fn include_partial_messages(mut self, enabled: bool) -> Self {
        self.include_partial_messages = Some(enabled);
        self
    }

    /// Builds the [`ClaudeConfig`].
    #[must_use]
    pub fn build(self) -> ClaudeConfig {
        ClaudeConfig {
            model: self.model,
            system_prompt: self.system_prompt,
            max_turns: self.max_turns,
            timeout: self.timeout,
            include_partial_messages: self.include_partial_messages,
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
}
