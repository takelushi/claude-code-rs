use claude_code_rs::{ClaudeClient, ClaudeConfig};
use std::time::Duration;

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
