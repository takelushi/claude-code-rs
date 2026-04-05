#[tokio::main]
async fn main() {
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Say hello".into());
    let client = claude_code_rs::ClaudeClient::new(claude_code_rs::ClaudeConfig::default());
    match client.ask(&prompt).await {
        Ok(resp) => println!("{resp:#?}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}
