// Multi-turn conversation example.
// Uses --resume to continue a session across multiple ask() calls.
// Requires no_session_persistence(false) so the CLI saves the session to disk.
//
// Usage: cargo run --example multi_turn

#[tokio::main]
async fn main() {
    // Turn 1: initial question
    let config = claude_code_rs::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);

    println!("[Turn 1] Asking: What is 2+2?");
    let resp1 = match client.ask("What is 2+2? Answer in one word.").await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };
    println!("[Turn 1] Response: {}", resp1.result);
    println!("[Turn 1] Session: {}", resp1.session_id);

    // Turn 2: follow-up using --resume with the session ID from turn 1
    let config2 = claude_code_rs::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .resume(&resp1.session_id)
        .build();
    let client2 = claude_code_rs::ClaudeClient::new(config2);

    println!("\n[Turn 2] Asking: What was my previous question?");
    let resp2 = match client2
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
