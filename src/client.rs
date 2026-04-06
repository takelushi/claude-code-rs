#[cfg(test)]
use mockall::automock;

use std::pin::Pin;
use std::process::Output;

use tokio::io::BufReader;
use tokio::process::Command as TokioCommand;
use tokio_stream::Stream;

use crate::config::ClaudeConfig;
use crate::conversation::Conversation;
use crate::error::ClaudeError;
use crate::stream::StreamEvent;
use crate::stream::parse_stream;
use crate::types::{ClaudeResponse, strip_ansi};

/// Trait abstracting CLI execution. Mockable in tests.
#[allow(async_fn_in_trait)]
#[cfg_attr(test, automock)]
pub trait CommandRunner: Send + Sync {
    /// Runs the `claude` command with the given arguments.
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;
}

/// Runs `claude` via `tokio::process::Command`.
#[derive(Debug, Clone)]
pub struct DefaultRunner;

impl CommandRunner for DefaultRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        TokioCommand::new("claude").args(args).output().await
    }
}

/// Claude Code CLI client.
#[derive(Debug)]
pub struct ClaudeClient<R: CommandRunner = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
}

impl ClaudeClient {
    /// Creates a new client with the default [`DefaultRunner`].
    #[must_use]
    pub fn new(config: ClaudeConfig) -> Self {
        Self {
            config,
            runner: DefaultRunner,
        }
    }

    /// Sends a prompt and returns a stream of events.
    ///
    /// Spawns the CLI with `--output-format stream-json` and streams events
    /// in real-time. The stream ends with a [`StreamEvent::Result`] on success.
    ///
    /// For real-time token-level streaming, enable
    /// [`crate::ClaudeConfigBuilder::include_partial_messages`]. This produces
    /// [`StreamEvent::Text`] / [`StreamEvent::Thinking`] delta chunks.
    /// Without it, only complete [`StreamEvent::AssistantText`] /
    /// [`StreamEvent::AssistantThinking`] messages are emitted.
    ///
    /// Timeout is not applied to streams. Use [`tokio_stream::StreamExt::timeout()`]
    /// if needed.
    pub async fn ask_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        let args = self.config.to_stream_args(prompt);

        tracing::debug!(args = ?args, "spawning claude CLI stream");

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

        Ok(Box::pin(async_stream::stream! {
            tokio::pin!(event_stream);
            while let Some(event) = tokio_stream::StreamExt::next(&mut event_stream).await {
                yield Ok(event);
            }

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
        }))
    }
}

impl<R: CommandRunner> ClaudeClient<R> {
    /// Creates a new client with a custom [`CommandRunner`] for testing.
    #[must_use]
    pub fn with_runner(config: ClaudeConfig, runner: R) -> Self {
        Self { config, runner }
    }

    /// Sends a prompt and deserializes the result into `T`.
    ///
    /// Requires `json_schema` to be set on the config beforehand.
    /// Use [`generate_schema`](crate::generate_schema) to auto-generate it
    /// (requires the `structured` feature).
    pub async fn ask_structured<T: serde::de::DeserializeOwned>(
        &self,
        prompt: &str,
    ) -> Result<T, ClaudeError> {
        let response = self.ask(prompt).await?;
        response.parse_result()
    }

    /// Sends a prompt and returns the response.
    pub async fn ask(&self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> {
        let args = self.config.to_args(prompt);

        tracing::debug!(args = ?args, "executing claude CLI");

        let io_result: std::io::Result<Output> = if let Some(timeout) = self.config.timeout {
            tokio::time::timeout(timeout, self.runner.run(&args))
                .await
                .map_err(|_| {
                    let err = ClaudeError::Timeout;
                    tracing::error!(error = %err, "claude CLI failed");
                    err
                })?
        } else {
            self.runner.run(&args).await
        };

        let output = io_result.map_err(|e| {
            let err = if e.kind() == std::io::ErrorKind::NotFound {
                ClaudeError::CliNotFound
            } else {
                ClaudeError::Io(e)
            };
            tracing::error!(error = %err, "claude CLI failed");
            err
        })?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let err = ClaudeError::NonZeroExit { code, stderr };
            tracing::error!(error = %err, "claude CLI failed");
            return Err(err);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_str = strip_ansi(&stdout);
        let response: ClaudeResponse = serde_json::from_str(json_str).map_err(|e| {
            let err = ClaudeError::ParseError(e);
            tracing::error!(error = %err, "claude CLI failed");
            err
        })?;

        tracing::info!("claude CLI returned successfully");
        Ok(response)
    }
}

impl<R: CommandRunner + Clone> ClaudeClient<R> {
    /// Creates a new [`Conversation`] for multi-turn interaction.
    ///
    /// The conversation manages `session_id` automatically, injecting
    /// `--resume` from the second turn onwards.
    ///
    /// Callers must set [`crate::ClaudeConfigBuilder::no_session_persistence`]`(false)`
    /// for multi-turn to work.
    #[must_use]
    pub fn conversation(&self) -> Conversation<R> {
        Conversation::with_runner(self.config.clone(), self.runner.clone())
    }

    /// Creates a [`Conversation`] that resumes an existing session.
    ///
    /// The first `ask()` / `ask_stream()` call will include `--resume`
    /// with the given session ID.
    #[must_use]
    pub fn conversation_resume(&self, session_id: impl Into<String>) -> Conversation<R> {
        Conversation::with_runner_resume(self.config.clone(), self.runner.clone(), session_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn success_output() -> Output {
        Output {
            status: ExitStatus::from_raw(0),
            stdout: include_bytes!("../tests/fixtures/success.json").to_vec(),
            stderr: Vec::new(),
        }
    }

    fn non_zero_output() -> Output {
        Output {
            status: ExitStatus::from_raw(256), // exit code 1
            stdout: Vec::new(),
            stderr: b"something went wrong".to_vec(),
        }
    }

    #[tokio::test]
    async fn ask_success() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| Ok(success_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let resp = client.ask("hello").await.unwrap();
        assert_eq!(resp.result, "Hello!");
        assert!(!resp.is_error);
    }

    #[tokio::test]
    async fn ask_cli_not_found() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "not found",
            ))
        });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::CliNotFound));
    }

    #[tokio::test]
    async fn ask_non_zero_exit() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| Ok(non_zero_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::NonZeroExit { code: 1, .. }));
    }

    #[tokio::test]
    async fn ask_parse_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| {
            Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: b"not json".to_vec(),
                stderr: Vec::new(),
            })
        });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::ParseError(_)));
    }

    /// Custom CommandRunner that always sleeps (for timeout tests).
    struct SlowRunner;

    impl CommandRunner for SlowRunner {
        async fn run(&self, _args: &[String]) -> std::io::Result<Output> {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            Ok(Output {
                status: std::os::unix::process::ExitStatusExt::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ask_timeout() {
        let config = ClaudeConfig::builder()
            .timeout(std::time::Duration::from_millis(10))
            .build();
        let client = ClaudeClient::with_runner(config, SlowRunner);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::Timeout));
    }

    #[tokio::test]
    async fn ask_io_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "denied",
            ))
        });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client.ask("hello").await.unwrap_err();
        assert!(matches!(err, ClaudeError::Io(_)));
    }

    #[tokio::test]
    async fn ask_with_ansi_escape() {
        let json = include_str!("../tests/fixtures/success.json");
        let stdout = format!("\x1b[?1004l{json}\x1b[?1004l");

        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(move |_| {
            Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: stdout.clone().into_bytes(),
                stderr: Vec::new(),
            })
        });

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let resp = client.ask("hello").await.unwrap();
        assert_eq!(resp.result, "Hello!");
    }

    #[tokio::test]
    async fn ask_passes_correct_args() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .withf(|args| {
                args.contains(&"--print".to_string())
                    && args.contains(&"--model".to_string())
                    && args.contains(&"haiku".to_string())
                    && args.last() == Some(&"test prompt".to_string())
            })
            .returning(|_| Ok(success_output()));

        let config = ClaudeConfig::builder().model("haiku").build();
        let client = ClaudeClient::with_runner(config, mock);
        client.ask("test prompt").await.unwrap();
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct TestAnswer {
        value: i32,
    }

    fn structured_success_output() -> Output {
        Output {
            status: ExitStatus::from_raw(0),
            stdout: include_bytes!("../tests/fixtures/structured_success.json").to_vec(),
            stderr: Vec::new(),
        }
    }

    #[tokio::test]
    async fn ask_structured_success() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run()
            .returning(|_| Ok(structured_success_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let answer: TestAnswer = client.ask_structured("What is 6*7?").await.unwrap();
        assert_eq!(answer, TestAnswer { value: 42 });
    }

    #[tokio::test]
    async fn ask_structured_deserialization_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| Ok(success_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client
            .ask_structured::<TestAnswer>("hello")
            .await
            .unwrap_err();
        assert!(matches!(err, ClaudeError::StructuredOutputError { .. }));
    }

    #[tokio::test]
    async fn ask_structured_cli_error() {
        let mut mock = MockCommandRunner::new();
        mock.expect_run().returning(|_| Ok(non_zero_output()));

        let client = ClaudeClient::with_runner(ClaudeConfig::default(), mock);
        let err = client
            .ask_structured::<TestAnswer>("hello")
            .await
            .unwrap_err();
        assert!(matches!(err, ClaudeError::NonZeroExit { code: 1, .. }));
    }
}
