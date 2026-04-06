use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio_stream::Stream;

use crate::client::{ClaudeClient, CommandRunner, DefaultRunner};
use crate::config::{ClaudeConfig, ClaudeConfigBuilder};
use crate::error::ClaudeError;
use crate::stream::StreamEvent;
use crate::types::ClaudeResponse;

/// Stateful multi-turn conversation wrapper around [`ClaudeClient`].
///
/// Manages `session_id` automatically across turns using `--resume`.
/// The base config is cloned per turn; each turn builds a temporary
/// config with `--resume <session_id>` injected.
///
/// # Design decisions
///
/// **Ownership model:** Owns cloned copies of [`ClaudeConfig`] and the runner
/// instead of borrowing `&ClaudeClient`. `ClaudeClient` is stateless (config +
/// runner only, no connection pool), so cloning is cheap and avoids lifetime
/// parameters that complicate async usage (spawn, struct storage).
///
/// **session_id storage:** Uses `Arc<Mutex<Option<String>>>` so that the
/// streaming path can update the session ID while the caller consumes the
/// returned `Stream` (which outlives the `&mut self` borrow).
///
/// # Note
///
/// Callers must set [`ClaudeConfigBuilder::no_session_persistence`]`(false)` in
/// the config for multi-turn to work. The library does not override this; option
/// validation is the CLI's responsibility.
#[derive(Debug)]
pub struct Conversation<R: CommandRunner = DefaultRunner> {
    config: ClaudeConfig,
    runner: R,
    session_id: Arc<Mutex<Option<String>>>,
}

impl<R: CommandRunner> Conversation<R> {
    /// Returns the current session ID, or `None` if no turn has completed.
    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.session_id.lock().unwrap().clone()
    }
}

impl<R: CommandRunner + Clone> Conversation<R> {
    /// Creates a new conversation (internal; use [`ClaudeClient::conversation`]).
    pub(crate) fn with_runner(config: ClaudeConfig, runner: R) -> Self {
        Self {
            config,
            runner,
            session_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Creates a conversation resuming an existing session (internal;
    /// use [`ClaudeClient::conversation_resume`]).
    pub(crate) fn with_runner_resume(
        config: ClaudeConfig,
        runner: R,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            config,
            runner,
            session_id: Arc::new(Mutex::new(Some(session_id.into()))),
        }
    }

    /// Sends a prompt and returns the response.
    ///
    /// Shorthand for `ask_with(prompt, |b| b)`.
    pub async fn ask(&mut self, prompt: &str) -> Result<ClaudeResponse, ClaudeError> {
        self.ask_with(prompt, |b| b).await
    }

    /// Sends a prompt with per-turn config overrides and returns the response.
    ///
    /// The closure receives a [`ClaudeConfigBuilder`] pre-filled with the base
    /// config. Overrides apply to this turn only; the base config is unchanged.
    pub async fn ask_with<F>(
        &mut self,
        prompt: &str,
        config_fn: F,
    ) -> Result<ClaudeResponse, ClaudeError>
    where
        F: FnOnce(ClaudeConfigBuilder) -> ClaudeConfigBuilder,
    {
        let builder = config_fn(self.config.to_builder());
        let mut config = builder.build();

        if let Some(ref id) = *self.session_id.lock().unwrap() {
            config.resume = Some(id.clone());
        }

        let client = ClaudeClient::with_runner(config, self.runner.clone());
        let response = client.ask(prompt).await?;

        *self.session_id.lock().unwrap() = Some(response.session_id.clone());

        Ok(response)
    }
}

/// Wraps a stream to transparently capture `session_id` from
/// [`StreamEvent::SystemInit`] and [`StreamEvent::Result`].
fn wrap_stream(
    inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>,
    session_id: Arc<Mutex<Option<String>>>,
) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> {
    Box::pin(async_stream::stream! {
        tokio::pin!(inner);
        while let Some(item) = tokio_stream::StreamExt::next(&mut inner).await {
            if let Ok(ref event) = item {
                match event {
                    StreamEvent::SystemInit { session_id: sid, .. } => {
                        *session_id.lock().unwrap() = Some(sid.clone());
                    }
                    StreamEvent::Result(response) => {
                        *session_id.lock().unwrap() = Some(response.session_id.clone());
                    }
                    _ => {}
                }
            }
            yield item;
        }
    })
}

impl Conversation {
    /// Sends a prompt and returns a stream of events.
    ///
    /// Shorthand for `ask_stream_with(prompt, |b| b)`.
    ///
    /// Only available for `Conversation<DefaultRunner>` (i.e., conversations
    /// created via [`ClaudeClient::new`]). The [`CommandRunner`] trait's
    /// [`run`](CommandRunner::run) method returns a completed [`std::process::Output`],
    /// which cannot support streaming; therefore streaming always spawns a
    /// real CLI subprocess.
    pub async fn ask_stream(
        &mut self,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    {
        self.ask_stream_with(prompt, |b| b).await
    }

    /// Sends a prompt with per-turn config overrides and returns a stream.
    ///
    /// The closure receives a [`ClaudeConfigBuilder`] pre-filled with the base
    /// config. Overrides apply to this turn only; the base config is unchanged.
    ///
    /// All events are passed through transparently. Internally, `session_id`
    /// is captured from [`StreamEvent::SystemInit`] and updated from
    /// [`StreamEvent::Result`].
    ///
    /// Only available for `Conversation<DefaultRunner>`. See [`ask_stream`](Self::ask_stream)
    /// for details on the streaming constraint.
    pub async fn ask_stream_with<F>(
        &mut self,
        prompt: &str,
        config_fn: F,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>>, ClaudeError>
    where
        F: FnOnce(ClaudeConfigBuilder) -> ClaudeConfigBuilder,
    {
        let builder = config_fn(self.config.to_builder());
        let mut config = builder.build();

        if let Some(ref id) = *self.session_id.lock().unwrap() {
            config.resume = Some(id.clone());
        }

        let client = ClaudeClient::new(config);
        let inner = client.ask_stream(prompt).await?;

        Ok(wrap_stream(inner, Arc::clone(&self.session_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io;
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    /// A [`CommandRunner`] that records arguments and returns pre-configured
    /// responses. Clone-compatible (unlike mockall mocks), which is required
    /// for `Conversation` since it clones the runner for each turn.
    #[derive(Clone)]
    struct RecordingRunner {
        responses: Arc<Mutex<VecDeque<io::Result<Output>>>>,
        captured_args: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl RecordingRunner {
        fn new(responses: Vec<io::Result<Output>>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(VecDeque::from(responses))),
                captured_args: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn captured_args(&self) -> Vec<Vec<String>> {
            self.captured_args.lock().unwrap().clone()
        }
    }

    impl CommandRunner for RecordingRunner {
        async fn run(&self, args: &[String]) -> io::Result<Output> {
            self.captured_args.lock().unwrap().push(args.to_vec());
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("RecordingRunner: no more responses")
        }
    }

    fn make_success_output(session_id: &str) -> io::Result<Output> {
        let json = format!(
            r#"{{"type":"result","subtype":"success","is_error":false,"duration_ms":100,"duration_api_ms":90,"num_turns":1,"result":"Hello!","stop_reason":"end_turn","session_id":"{session_id}","total_cost_usd":0.001,"usage":{{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5,"server_tool_use":{{"web_search_requests":0,"web_fetch_requests":0}}}}}}"#
        );
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: json.into_bytes(),
            stderr: Vec::new(),
        })
    }

    #[tokio::test]
    async fn session_id_initially_none() {
        let runner = RecordingRunner::new(vec![]);
        let conv = Conversation::with_runner(ClaudeConfig::default(), runner);
        assert!(conv.session_id().is_none());
    }

    #[tokio::test]
    async fn ask_captures_session_id() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner);

        let resp = conv.ask("hello").await.unwrap();
        assert_eq!(resp.session_id, "sid-001");
        assert_eq!(conv.session_id(), Some("sid-001".to_string()));
    }

    #[tokio::test]
    async fn second_turn_sends_resume() {
        let runner = RecordingRunner::new(vec![
            make_success_output("sid-001"),
            make_success_output("sid-001"),
        ]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner.clone());

        conv.ask("turn 1").await.unwrap();
        conv.ask("turn 2").await.unwrap();

        let args = runner.captured_args();
        // Turn 1: no --resume
        assert!(!args[0].contains(&"--resume".to_string()));
        // Turn 2: --resume sid-001
        let idx = args[1].iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[1][idx + 1], "sid-001");
    }

    #[tokio::test]
    async fn ask_with_overrides_config() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner.clone());

        conv.ask_with("hello", |b| b.max_turns(5)).await.unwrap();

        let args = &runner.captured_args()[0];
        let idx = args.iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[idx + 1], "5");
    }

    #[tokio::test]
    async fn ask_with_does_not_affect_base_config() {
        let runner = RecordingRunner::new(vec![
            make_success_output("sid-001"),
            make_success_output("sid-001"),
        ]);
        let config = ClaudeConfig::builder().max_turns(1).build();
        let mut conv = Conversation::with_runner(config, runner.clone());

        conv.ask_with("turn 1", |b| b.max_turns(5)).await.unwrap();
        conv.ask("turn 2").await.unwrap();

        let args = runner.captured_args();
        let idx1 = args[0].iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[0][idx1 + 1], "5");
        let idx2 = args[1].iter().position(|a| a == "--max-turns").unwrap();
        assert_eq!(args[1][idx2 + 1], "1");
    }

    #[tokio::test]
    async fn error_preserves_session_id() {
        let error_output: io::Result<Output> = Ok(Output {
            status: ExitStatus::from_raw(256), // exit code 1
            stdout: Vec::new(),
            stderr: b"error".to_vec(),
        });
        let runner = RecordingRunner::new(vec![make_success_output("sid-001"), error_output]);
        let mut conv = Conversation::with_runner(ClaudeConfig::default(), runner);

        conv.ask("turn 1").await.unwrap();
        assert_eq!(conv.session_id(), Some("sid-001".to_string()));

        let _ = conv.ask("turn 2").await;
        assert_eq!(conv.session_id(), Some("sid-001".to_string()));
    }

    #[tokio::test]
    async fn conversation_resume_sends_resume_on_first_turn() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let mut conv = Conversation::with_runner_resume(
            ClaudeConfig::default(),
            runner.clone(),
            "existing-sid",
        );

        conv.ask("hello").await.unwrap();

        let args = &runner.captured_args()[0];
        let idx = args.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[idx + 1], "existing-sid");
    }

    use crate::types::Usage;

    #[tokio::test]
    async fn wrap_stream_captures_session_id_from_system_init() {
        let session_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let events: Vec<Result<StreamEvent, ClaudeError>> = vec![
            Ok(StreamEvent::SystemInit {
                session_id: "sid-stream-001".into(),
                model: "haiku".into(),
            }),
            Ok(StreamEvent::AssistantText("Hello!".into())),
        ];
        let inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> =
            Box::pin(tokio_stream::iter(events));

        let wrapped = wrap_stream(inner, Arc::clone(&session_id));
        tokio::pin!(wrapped);

        let mut count = 0;
        while (tokio_stream::StreamExt::next(&mut wrapped).await).is_some() {
            count += 1;
        }

        assert_eq!(
            *session_id.lock().unwrap(),
            Some("sid-stream-001".to_string())
        );
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn wrap_stream_updates_session_id_from_result() {
        let session_id: Arc<Mutex<Option<String>>> =
            Arc::new(Mutex::new(Some("old-sid".to_string())));
        let response = ClaudeResponse {
            result: "Hello!".into(),
            is_error: false,
            duration_ms: 100,
            num_turns: 1,
            session_id: "new-sid".into(),
            total_cost_usd: 0.001,
            stop_reason: "end_turn".into(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        };
        let events: Vec<Result<StreamEvent, ClaudeError>> = vec![
            Ok(StreamEvent::SystemInit {
                session_id: "old-sid".into(),
                model: "haiku".into(),
            }),
            Ok(StreamEvent::Result(response)),
        ];
        let inner: Pin<Box<dyn Stream<Item = Result<StreamEvent, ClaudeError>> + Send>> =
            Box::pin(tokio_stream::iter(events));

        let wrapped = wrap_stream(inner, Arc::clone(&session_id));
        tokio::pin!(wrapped);
        while (tokio_stream::StreamExt::next(&mut wrapped).await).is_some() {}

        assert_eq!(*session_id.lock().unwrap(), Some("new-sid".to_string()));
    }

    #[tokio::test]
    async fn client_conversation_creates_working_conversation() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let config = ClaudeConfig::builder().model("haiku").build();
        let client = ClaudeClient::with_runner(config, runner);

        let mut conv = client.conversation();
        let resp = conv.ask("hello").await.unwrap();
        assert_eq!(resp.session_id, "sid-001");
    }

    #[tokio::test]
    async fn client_conversation_resume_sends_resume() {
        let runner = RecordingRunner::new(vec![make_success_output("sid-001")]);
        let client = ClaudeClient::with_runner(ClaudeConfig::default(), runner.clone());

        let mut conv = client.conversation_resume("existing-sid");
        conv.ask("hello").await.unwrap();

        let args = &runner.captured_args()[0];
        let idx = args.iter().position(|a| a == "--resume").unwrap();
        assert_eq!(args[idx + 1], "existing-sid");
    }
}
