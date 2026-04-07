use claude_code::{ClaudeClient, ClaudeConfig, Preset};

#[tokio::main]
async fn main() {
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Say hello".into());

    // Define a reusable custom preset with only the flags you need.
    let preset = Preset::Custom(vec![
        "--print".into(),
        "--no-session-persistence".into(),
        "--model".into(),
        "haiku".into(),
    ]);

    let config = ClaudeConfig::builder().preset(preset).build();

    let client = ClaudeClient::new(config);
    match client.ask(&prompt).await {
        Ok(resp) => println!("{resp:#?}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
