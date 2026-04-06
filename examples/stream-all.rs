use std::io::Write;

use claude_code_rs::StreamExt;

#[tokio::main]
async fn main() {
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Say hello".into());
    // include_partial_messages(true) enables real-time token-level deltas.
    // AssistantText/AssistantThinking (complete messages) are skipped below
    // since the same content is already displayed via Text/Thinking deltas.
    let config = claude_code_rs::ClaudeConfig::builder()
        .max_turns(1)
        .include_partial_messages(true)
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
            Ok(claude_code_rs::StreamEvent::SystemInit { session_id, model }) => {
                eprintln!("[init] session={session_id} model={model}");
            }
            Ok(claude_code_rs::StreamEvent::Thinking(text)) => {
                eprint!("{text}");
                std::io::stderr().flush().unwrap();
            }
            Ok(claude_code_rs::StreamEvent::Text(text)) => {
                print!("{text}");
                std::io::stdout().flush().unwrap();
            }
            Ok(claude_code_rs::StreamEvent::ToolUse { id, name, input }) => {
                eprintln!("[tool_use] {name} (id={id}) input={input}");
            }
            Ok(claude_code_rs::StreamEvent::ToolResult {
                tool_use_id,
                content,
            }) => {
                eprintln!("[tool_result] id={tool_use_id} content={content}");
            }
            Ok(claude_code_rs::StreamEvent::InputJsonDelta(partial)) => {
                eprint!("{partial}");
                std::io::stderr().flush().unwrap();
            }
            Ok(claude_code_rs::StreamEvent::AssistantThinking(_)) => {}
            Ok(claude_code_rs::StreamEvent::AssistantText(_)) => {}
            Ok(claude_code_rs::StreamEvent::SignatureDelta(_)) => {}
            Ok(claude_code_rs::StreamEvent::CitationsDelta(val)) => {
                eprintln!("[citation] {val}");
            }
            Ok(claude_code_rs::StreamEvent::MessageStart { model, id }) => {
                eprintln!("[message_start] model={model} id={id}");
            }
            Ok(claude_code_rs::StreamEvent::ContentBlockStart { index, block_type }) => {
                eprintln!("[block_start] index={index} type={block_type}");
            }
            Ok(claude_code_rs::StreamEvent::ContentBlockStop { index }) => {
                eprintln!("[block_stop] index={index}");
            }
            Ok(claude_code_rs::StreamEvent::MessageDelta { stop_reason }) => {
                eprintln!("[message_delta] stop_reason={stop_reason:?}");
            }
            Ok(claude_code_rs::StreamEvent::MessageStop) => {
                eprintln!("[message_stop]");
            }
            Ok(claude_code_rs::StreamEvent::Ping) => {
                eprintln!("[ping]");
            }
            Ok(claude_code_rs::StreamEvent::Error {
                error_type,
                message,
            }) => {
                eprintln!("[error] {error_type}: {message}");
            }
            Ok(claude_code_rs::StreamEvent::RateLimit { resets_at }) => {
                eprintln!("[rate_limit] resets_at={resets_at}");
            }
            Ok(claude_code_rs::StreamEvent::Result(resp)) => {
                println!("\n---");
                println!("Cost: ${:.6}", resp.total_cost_usd);
                println!(
                    "Tokens: {} in / {} out",
                    resp.usage.input_tokens, resp.usage.output_tokens
                );
            }
            Ok(claude_code_rs::StreamEvent::Unknown(val)) => {
                eprintln!("[unknown] {val}");
            }
            Err(e) => eprintln!("\nStream error: {e}"),
            _ => {}
        }
    }
    println!();
}
