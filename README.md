# claude-code-rs

A Rust library for executing [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) (`claude --print`) as a subprocess and handling results in a type-safe way.

Supports both single-shot JSON responses and real-time streaming via `--output-format stream-json`.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
claude-code-rs = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Prerequisites

The `claude` CLI must be installed and available in your `PATH`. See the [Claude Code documentation](https://docs.anthropic.com/en/docs/claude-code) for installation instructions.

## Usage

### Simple (single-shot)

```rust
#[tokio::main]
async fn main() {
    let client = claude_code_rs::ClaudeClient::new(claude_code_rs::ClaudeConfig::default());
    match client.ask("Say hello").await {
        Ok(resp) => println!("{}", resp.result),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

### Streaming

```rust
use claude_code_rs::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = claude_code_rs::ClaudeConfig::builder()
        .max_turns(1)
        .include_partial_messages(true)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);
    let mut stream = client.ask_stream("Say hello").await?;
    while let Some(event) = stream.next().await {
        match event {
            Ok(claude_code_rs::StreamEvent::Text(text)) => print!("{text}"),
            Ok(claude_code_rs::StreamEvent::Result(resp)) => {
                println!("\nCost: ${:.6}", resp.total_cost_usd);
            }
            Ok(_) => {}
            Err(e) => eprintln!("Error: {e}"),
        }
    }
    Ok(())
}
```

### Multi-turn conversation

Use `resume` to continue a conversation across multiple turns by passing the session ID from a previous response:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Turn 1
    let config = claude_code_rs::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);
    let resp1 = client.ask("What is 2+2?").await?;

    // Turn 2: resume with session ID
    let config2 = claude_code_rs::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .resume(&resp1.session_id)
        .build();
    let client2 = claude_code_rs::ClaudeClient::new(config2);
    let resp2 = client2.ask("What was my previous question?").await?;
    println!("{}", resp2.result);

    Ok(())
}
```

## Context Minimization Defaults

By default, `claude-code-rs` applies a minimal configuration to reduce unnecessary context sent to the CLI. This keeps costs down and avoids side effects from user-level settings:

| Default | CLI Flag | Effect |
|---|---|---|
| No session persistence | `--no-session-persistence` | Sessions are not saved to disk |
| No settings loaded | `--setting-sources ""` | Ignores all user/project settings files |
| Strict MCP config | `--strict-mcp-config` | Only uses explicitly provided MCP servers |
| Empty MCP config | `--mcp-config '{"mcpServers":{}}'` | No MCP servers enabled |
| No built-in tools | `--tools ""` | Disables all built-in tools |
| No slash commands | `--disable-slash-commands` | Disables all skills/slash commands |
| Empty system prompt | `--system-prompt ""` | No default system prompt |

All of these can be overridden via `ClaudeConfigBuilder`. For example, to re-enable session persistence:

```rust
let config = claude_code_rs::ClaudeConfig::builder()
    .no_session_persistence(false)
    .build();
```

## Minimum Supported Rust Version

Rust 1.93+ (edition 2024).

## License

MIT
