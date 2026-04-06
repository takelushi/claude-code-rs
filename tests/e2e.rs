#[cfg(feature = "stream")]
use claude_code_rs::StreamEvent;
use claude_code_rs::{ClaudeClient, ClaudeConfig};
use std::time::Duration;
#[cfg(feature = "stream")]
use tokio_stream::StreamExt;

#[tokio::test]
#[ignore] // Run explicitly with: cargo test -- --ignored
async fn e2e_ask_with_haiku() {
    let config = ClaudeConfig::builder()
        .model("haiku")
        .max_turns(1)
        .timeout(Duration::from_secs(30))
        .build();

    let client = ClaudeClient::new(config);
    let resp = client.ask("Say 'hello' and nothing else").await.unwrap();

    assert!(!resp.is_error);
    assert!(!resp.result.is_empty());
    assert!(resp.num_turns >= 1);
    assert!(resp.total_cost_usd >= 0.0);
    assert!(resp.usage.output_tokens > 0);
}

#[cfg(feature = "stream")]
#[tokio::test]
#[ignore] // Run explicitly with: cargo test -- --ignored
async fn e2e_ask_stream_with_haiku() {
    let config = ClaudeConfig::builder().model("haiku").max_turns(1).build();

    let client = ClaudeClient::new(config);
    let mut stream = client
        .ask_stream("Say 'hello' and nothing else")
        .await
        .unwrap();

    let mut got_text = false;
    let mut got_result = false;

    while let Some(event) = stream.next().await {
        match event.unwrap() {
            StreamEvent::Text(_) | StreamEvent::AssistantText(_) => got_text = true,
            StreamEvent::Result(resp) => {
                assert!(!resp.is_error);
                assert!(!resp.result.is_empty());
                got_result = true;
            }
            _ => {}
        }
    }

    assert!(got_text, "should have received at least one Text event");
    assert!(got_result, "should have received a Result event");
}
