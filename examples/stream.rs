use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let prompt = std::env::args().nth(1).unwrap_or_else(|| "Say hello".into());
    let config = claude_code_rs::ClaudeConfig::builder()
        .max_turns(1)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);

    let mut stream = match client.ask_stream(&prompt).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    while let Some(event) = stream.next().await {
        match event {
            Ok(claude_code_rs::StreamEvent::Text(text)) => print!("{text}"),
            Ok(claude_code_rs::StreamEvent::Result(resp)) => {
                println!("\n---");
                println!("Cost: ${:.6}", resp.total_cost_usd);
                println!("Tokens: {} in / {} out", resp.usage.input_tokens, resp.usage.output_tokens);
            }
            Ok(_) => {}
            Err(e) => eprintln!("\nStream error: {e}"),
        }
    }
    println!();
}
