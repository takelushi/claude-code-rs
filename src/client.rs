/// Conditional tracing macros — compile to nothing when the `tracing` feature is disabled.
macro_rules! trace_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::debug!($($arg)*);
    };
}
macro_rules! trace_error {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::error!($($arg)*);
    };
}
macro_rules! trace_info {
    ($($arg:tt)*) => {
        #[cfg(feature = "tracing")]
        tracing::info!($($arg)*);
    };
}

#[cfg(test)]
use mockall::automock;

use std::process::Output;

use tokio::process::Command as TokioCommand;

use crate::config::ClaudeConfig;
use crate::conversation::Conversation;
use crate::error::ClaudeError;
use crate::types::{ClaudeResponse, strip_ansi};

#[cfg(feature = "stream")]
use crate::stream::{StreamEvent, parse_stream};
#[cfg(feature = "stream")]
use std::pin::Pin;
#[cfg(feature = "stream")]
use tokio::io::BufReader;
#[cfg(feature = "stream")]
use tokio_stream::Stream;

/// Trait abstracting CLI execution. Mockable in tests.
#[allow(async_fn_in_trait)]
#[cfg_attr(test, automock)]
pub trait CommandRunner: Send + Sync {
    /// Runs the `claude` command with the given arguments.
    async fn run(&self, args: &[String]) -> std::io::Result<Output>;
}

/// Runs `claude` via `tokio::process::Command`.
#[derive(Debug, Clone)]
pub struct DefaultRunner {
    cli_path: String,
}

impl DefaultRunner {
    /// Creates a runner with a custom CLI binary path.
    #[must_use]
    pub fn new(cli_path: impl Into<String>) -> Self {
        Self {
            cli_path: cli_path.into(),
        }
    }
}

impl Default for DefaultRunner {
    fn default() -> Self {
        Self {
            cli_path: "claude".into(),
        }
    }
}

impl CommandRunner for DefaultRunner {
    async fn run(&self, args: &[String]) -> std::io::Result<Output> {
        TokioCommand::new(&self.cli_path).args(args).output().await
    }
}

/// RAII guard that kills the child process on drop.
///
/// tokio's `Child` does NOT kill the process on drop — it detaches.
/// This guard ensures the CLI subprocess is killed when the stream
/// is dropped (e.g., client disconnection).
#[cfg(feature = "stream")]
struct ChildGuard(Option<tokio::process::Child>);

#[cfg(feature = "stream")]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.0 {
            let _ = child.start_kill();
        }
    }
}

/// Claude Code CLI client.
#[derive(Debug, Clone)]
pub struct ClaudeClient<R: CommandRunner = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
}

impl ClaudeClient {
    /// Creates a new client with the default [`DefaultRunner`].
    #[must_use]
    pub fn new(config: ClaudeConfig) -> Self {
        let runner = DefaultRunner::new(config.cli_path_or_default());
        Self { config, runner }
    }
}

#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
impl ClaudeClient {
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
    /// Use [`crate::ClaudeConfigBuilder::stream_idle_timeout`] to set an idle timeout.
    /// If no event arrives within the specified duration, the stream yields
    /// [`ClaudeError::Timeout`] and terminates.
    pub async fn ask_stream(
        &self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        let args = self.config.to_stream_args(prompt);

        trace_debug!(args = ?args, "spawning claude CLI stream");

        let mut child = TokioCommand::new(self.config.cli_path_or_default())
            .args(&args)
            .stdin(std::process::Stdio::null())
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

        trace_debug!(args = ?args, "executing claude CLI");

        let io_result: std::io::Result<Output> = if let Some(timeout) = self.config.timeout {
            tokio::time::timeout(timeout, self.runner.run(&args))
                .await
                .map_err(|_| {
                    let err = ClaudeError::Timeout;
                    trace_error!(error = %err, "claude CLI failed");
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
            trace_error!(error = %err, "claude CLI failed");
            err
        })?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let err = ClaudeError::NonZeroExit { code, stderr };
            trace_error!(error = %err, "claude CLI failed");
            return Err(err);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_str = strip_ansi(&stdout);
        let response: ClaudeResponse = serde_json::from_str(json_str).map_err(|e| {
            let err = ClaudeError::ParseError(e);
            trace_error!(error = %err, "claude CLI failed");
            err
        })?;

        trace_info!("claude CLI returned successfully");
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

/// Checks that the `claude` CLI is available and returns its version string.
///
/// Runs `claude --version` and returns the trimmed stdout on success.
/// To check a binary at a custom path, use [`check_cli_with_path`].
///
/// # Errors
///
/// - [`ClaudeError::CliNotFound`] if `claude` is not in PATH.
/// - [`ClaudeError::NonZeroExit`] if the command fails.
/// - [`ClaudeError::Io`] for other I/O errors.
pub async fn check_cli() -> Result<String, ClaudeError> {
    check_cli_with_path("claude").await
}

/// Checks that the CLI at the given path is available and returns its version string.
///
/// Runs `<cli_path> --version` and returns the trimmed stdout on success.
///
/// # Errors
///
/// - [`ClaudeError::CliNotFound`] if the binary is not found.
/// - [`ClaudeError::NonZeroExit`] if the command fails.
/// - [`ClaudeError::Io`] for other I/O errors.
pub async fn check_cli_with_path(cli_path: &str) -> Result<String, ClaudeError> {
    let output = TokioCommand::new(cli_path)
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

/// Parses a version string like `"2.1.92"` or `"claude-code 2.1.92"` into `(major, minor, patch)`.
///
/// Returns `None` if no valid semver triple is found.
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    // Take the last whitespace-delimited token to handle prefixes like "claude-code 2.1.92"
    let ver = version.split_whitespace().next_back()?;
    let mut parts = ver.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// Result of comparing the installed CLI version against [`TESTED_CLI_VERSION`](crate::TESTED_CLI_VERSION).
///
/// Each variant carries the raw version string returned by `claude --version`.
/// The library does not judge any status as an error — callers decide how to
/// handle each case (e.g. log a warning, reject, or ignore).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CliVersionStatus {
    /// Installed version exactly matches `TESTED_CLI_VERSION`.
    Exact(String),
    /// Installed version is newer than `TESTED_CLI_VERSION`.
    Newer(String),
    /// Installed version is older than `TESTED_CLI_VERSION`.
    Older(String),
    /// Installed version string could not be parsed.
    Unknown(String),
}

/// Compares an `installed` version string against a `tested` version string.
fn compare_version(installed: &str, tested: &str) -> CliVersionStatus {
    let tested_tuple = parse_version(tested).unwrap_or((0, 0, 0));
    match parse_version(installed) {
        None => CliVersionStatus::Unknown(installed.to_string()),
        Some(v) if v == tested_tuple => CliVersionStatus::Exact(installed.to_string()),
        Some(v) if v > tested_tuple => CliVersionStatus::Newer(installed.to_string()),
        Some(_) => CliVersionStatus::Older(installed.to_string()),
    }
}

/// Checks the installed `claude` CLI version against [`TESTED_CLI_VERSION`](crate::TESTED_CLI_VERSION).
///
/// Runs `claude --version` and returns a [`CliVersionStatus`] indicating
/// whether the installed version is exact, newer, older, or unparseable.
///
/// # Errors
///
/// - [`ClaudeError::CliNotFound`] if `claude` is not in PATH.
/// - [`ClaudeError::NonZeroExit`] if the command fails.
/// - [`ClaudeError::Io`] for other I/O errors.
pub async fn check_cli_version() -> Result<CliVersionStatus, ClaudeError> {
    check_cli_version_with_path("claude").await
}

/// Checks the CLI at the given path against [`TESTED_CLI_VERSION`](crate::TESTED_CLI_VERSION).
///
/// Returns a [`CliVersionStatus`] indicating the comparison result.
///
/// # Errors
///
/// - [`ClaudeError::CliNotFound`] if the binary is not found.
/// - [`ClaudeError::NonZeroExit`] if the command fails.
/// - [`ClaudeError::Io`] for other I/O errors.
pub async fn check_cli_version_with_path(cli_path: &str) -> Result<CliVersionStatus, ClaudeError> {
    let version = check_cli_with_path(cli_path).await?;
    Ok(compare_version(&version, crate::TESTED_CLI_VERSION))
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

    /// Verifies that shell metacharacters in `cli_path` are not interpreted.
    ///
    /// `Command::new()` uses `execvp` directly (no shell), so a path like
    /// `"claude; echo pwned"` is treated as a literal filename lookup and
    /// fails with `NotFound` — not as a shell command.
    #[tokio::test]
    async fn cli_path_with_shell_metacharacters_is_not_interpreted() {
        let malicious = "claude; echo pwned";
        let err = check_cli_with_path(malicious).await.unwrap_err();
        assert!(matches!(err, ClaudeError::CliNotFound));
    }

    #[tokio::test]
    async fn cli_path_with_command_substitution_is_not_interpreted() {
        let malicious = "$(echo claude)";
        let err = check_cli_with_path(malicious).await.unwrap_err();
        assert!(matches!(err, ClaudeError::CliNotFound));
    }

    #[test]
    fn parse_version_semver() {
        assert_eq!(parse_version("2.1.92"), Some((2, 1, 92)));
    }

    #[test]
    fn parse_version_with_prefix() {
        assert_eq!(parse_version("claude-code 2.1.92"), Some((2, 1, 92)));
    }

    #[test]
    fn parse_version_invalid() {
        assert_eq!(parse_version("not-a-version"), None);
    }

    #[test]
    fn parse_version_empty() {
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn parse_version_two_components() {
        assert_eq!(parse_version("2.1"), None);
    }

    #[test]
    fn parse_version_four_components() {
        // splitn(3, '.') yields ["2", "1", "92.1"] — "92.1" fails u64 parse
        assert_eq!(parse_version("2.1.92.1"), None);
    }

    #[test]
    fn compare_version_exact() {
        let status = compare_version("2.1.92", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Exact(_)));
    }

    #[test]
    fn compare_version_newer() {
        let status = compare_version("2.2.0", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Newer(_)));
    }

    #[test]
    fn compare_version_older() {
        let status = compare_version("2.0.0", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Older(_)));
    }

    #[test]
    fn compare_version_major_newer() {
        let status = compare_version("3.0.0", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Newer(_)));
    }

    #[test]
    fn compare_version_major_older() {
        let status = compare_version("1.9.99", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Older(_)));
    }

    #[test]
    fn compare_version_unparseable() {
        let status = compare_version("garbage", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Unknown(_)));
    }

    #[test]
    fn compare_version_with_prefix() {
        let status = compare_version("claude-code 2.1.92", "2.1.92");
        assert!(matches!(status, CliVersionStatus::Exact(_)));
    }

    #[test]
    fn cli_version_status_preserves_version_string() {
        let status = compare_version("2.2.0", "2.1.92");
        match status {
            CliVersionStatus::Newer(v) => assert_eq!(v, "2.2.0"),
            other => panic!("expected Newer, got {other:?}"),
        }
    }
}
