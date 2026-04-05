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
}

impl ClaudeConfig {
    /// Returns a new builder.
    #[must_use]
    pub fn builder() -> ClaudeConfigBuilder {
        ClaudeConfigBuilder::default()
    }

    /// Builds command-line arguments from this configuration.
    ///
    /// Includes fixed options such as `--print --output-format json`.
    #[must_use]
    pub fn to_args(&self, prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--print".into(),
            "--output-format".into(),
            "json".into(),
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

    /// Builds the [`ClaudeConfig`].
    #[must_use]
    pub fn build(self) -> ClaudeConfig {
        ClaudeConfig {
            model: self.model,
            system_prompt: self.system_prompt,
            max_turns: self.max_turns,
            timeout: self.timeout,
        }
    }
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
}
