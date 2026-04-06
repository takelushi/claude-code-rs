//! E2E tests for Conversation API.
//! Requires `claude` CLI in PATH. Run with `cargo test -- --ignored`.

#[cfg(feature = "stream")]
use claude_code_rs::StreamExt;
use claude_code_rs::{ClaudeClient, ClaudeConfig};

#[tokio::test]
#[ignore]
async fn conversation_two_turn_ask() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = ClaudeClient::new(config);
    let mut conv = client.conversation();

    let resp1 = conv
        .ask("What is 2+2? Answer with just the number.")
        .await
        .unwrap();
    assert!(!resp1.is_error);
    assert!(resp1.result.contains('4'));
    assert!(conv.session_id().is_some());

    let resp2 = conv
        .ask("What number did you just tell me? Reply with just the number.")
        .await
        .unwrap();
    assert!(!resp2.is_error);
    assert!(resp2.result.contains('4'));
}

#[cfg(feature = "stream")]
#[tokio::test]
#[ignore]
async fn conversation_stream_then_ask() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = ClaudeClient::new(config);
    let mut conv = client.conversation();

    // Turn 1: stream
    let mut stream = conv
        .ask_stream("What is 3+3? Answer with just the number.")
        .await
        .unwrap();
    while let Some(event) = stream.next().await {
        let _ = event.unwrap();
    }
    assert!(conv.session_id().is_some());

    // Turn 2: ask (same session)
    let resp2 = conv
        .ask("What number did you just tell me? Reply with just the number.")
        .await
        .unwrap();
    assert!(!resp2.is_error);
    assert!(resp2.result.contains('6'));
}
