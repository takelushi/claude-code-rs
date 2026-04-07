use std::io::Write;

use claude_code::StreamExt;

#[tokio::main]
async fn main() {
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Say hello".into());
    // include_partial_messages(true) enables real-time token-level Text/Thinking deltas.
    // Without it, only complete AssistantText/AssistantThinking messages are emitted.
    let config = claude_code::ClaudeConfig::builder()
        .max_turns(1)
        .include_partial_messages(true)
        .build();
    let client = claude_code::ClaudeClient::new(config);

    let mut stream = match client.ask_stream(&prompt).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    while let Some(event) = stream.next().await {
        match event {
            Ok(claude_code::StreamEvent::Text(text)) => {
                print!("{text}");
                std::io::stdout().flush().unwrap();
            }
            Ok(claude_code::StreamEvent::Result(resp)) => {
                println!("\n---");
                println!("Cost: ${:.6}", resp.total_cost_usd);
                println!(
                    "Tokens: {} in / {} out",
                    resp.usage.input_tokens, resp.usage.output_tokens
                );
            }
            Ok(_) => {}
            Err(e) => eprintln!("\nStream error: {e}"),
        }
    }
    println!();
}
