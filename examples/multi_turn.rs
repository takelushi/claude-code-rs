// Multi-turn conversation example using the Conversation API.
// Automatically manages session_id across turns via --resume.
// Requires no_session_persistence(false) so the CLI saves the session to disk.
//
// Usage: cargo run --example multi_turn

#[tokio::main]
async fn main() {
    let config = claude_code::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = claude_code::ClaudeClient::new(config);
    let mut conv = client.conversation();

    // Turn 1
    println!("[Turn 1] Asking: What is 2+2?");
    let resp1 = match conv.ask("What is 2+2? Answer in one word.").await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };
    println!("[Turn 1] Response: {}", resp1.result);
    println!("[Turn 1] Session: {}", resp1.session_id);

    // Turn 2: session_id is automatically managed
    println!("\n[Turn 2] Asking: What was my previous question?");
    let resp2 = match conv
        .ask("What was my previous question? Repeat it exactly.")
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };
    println!("[Turn 2] Response: {}", resp2.result);
    println!(
        "\nTotal cost: ${:.6}",
        resp1.total_cost_usd + resp2.total_cost_usd
    );
}
