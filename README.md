# claude-code

[![Crates.io](https://img.shields.io/crates/v/claude-code.svg)](https://crates.io/crates/claude-code)
[![docs.rs](https://docs.rs/claude-code/badge.svg)](https://docs.rs/claude-code)
[![CI](https://github.com/takelushi/claude-code-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/takelushi/claude-code-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> **Unofficial** — This library is not affiliated with or endorsed by Anthropic. "Claude" is a trademark of Anthropic.

A Rust library for executing [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) (`claude --print`) as a subprocess and handling results in a type-safe way.

Supports both single-shot JSON responses and real-time streaming via `--output-format stream-json`.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
claude-code = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Prerequisites

The `claude` CLI must be installed and available in your `PATH`. See the [Claude Code documentation](https://docs.anthropic.com/en/docs/claude-code) for installation instructions.

## Usage

### Simple (single-shot)

```rust
#[tokio::main]
async fn main() {
    let client = claude_code::ClaudeClient::new(claude_code::ClaudeConfig::default());
    match client.ask("Say hello").await {
        Ok(resp) => println!("{}", resp.result),
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

### Streaming

```rust
use claude_code::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = claude_code::ClaudeConfig::builder()
        .max_turns(1)
        .include_partial_messages(true)
        .build();
    let client = claude_code::ClaudeClient::new(config);
    let mut stream = client.ask_stream("Say hello").await?;
    while let Some(event) = stream.next().await {
        match event {
            Ok(claude_code::StreamEvent::Text(text)) => print!("{text}"),
            Ok(claude_code::StreamEvent::Result(resp)) => {
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
    let config = claude_code::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .build();
    let client = claude_code::ClaudeClient::new(config);
    let resp1 = client.ask("What is 2+2?").await?;

    // Turn 2: resume with session ID
    let config2 = claude_code::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false)
        .max_turns(1)
        .resume(&resp1.session_id)
        .build();
    let client2 = claude_code::ClaudeClient::new(config2);
    let resp2 = client2.ask("What was my previous question?").await?;
    println!("{}", resp2.result);

    Ok(())
}
```

### Conversation API

The `Conversation` API manages `session_id` automatically across turns:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = claude_code::ClaudeConfig::builder()
        .model("haiku")
        .no_session_persistence(false) // required for multi-turn
        .max_turns(1)
        .build();
    let client = claude_code::ClaudeClient::new(config);

    let mut conv = client.conversation();
    let r1 = conv.ask("What is 2+2?").await?;
    println!("Turn 1: {}", r1.result);

    let r2 = conv.ask("What was my previous question?").await?;
    println!("Turn 2: {}", r2.result);

    Ok(())
}
```

Per-turn config overrides are supported via `ask_with`:

```rust
conv.ask_with("complex question", |b| b.max_turns(5).effort("high")).await?;
```

### Structured Output

Use `generate_schema` and `ask_structured` to get typed responses:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct CityInfo {
    name: String,
    country: String,
    population: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = claude_code::generate_schema::<CityInfo>()?;
    let config = claude_code::ClaudeConfig::builder()
        .model("haiku")
        .max_turns(1)
        .json_schema(&schema)
        .build();
    let client = claude_code::ClaudeClient::new(config);

    let city: CityInfo = client.ask_structured("Tell me about Tokyo").await?;
    println!("{}: population {}", city.name, city.population);

    Ok(())
}
```

Requires the `structured` feature. Add it to your dependencies:

```toml
[dependencies]
claude-code = { version = "0.1", features = ["structured"] }
schemars = "1"
```

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `stream` | Yes | Enables `ask_stream`, `StreamEvent`, and `Conversation` stream methods. Adds `tokio-stream` and `async-stream` dependencies. |
| `structured` | No | Enables `generate_schema` helper for JSON Schema generation. Adds `schemars` dependency. |
| `tracing` | Yes | Enables debug/error/info logging via `tracing`. Adds `tracing` dependency. |

To use a minimal configuration (only `ask()`):

```toml
[dependencies]
claude-code = { version = "0.1", default-features = false }
```

## Context Minimization Defaults

By default, `claude-code` applies a minimal configuration to reduce unnecessary context sent to the CLI. This keeps costs down and avoids side effects from user-level settings:

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
let config = claude_code::ClaudeConfig::builder()
    .no_session_persistence(false)
    .build();
```

## Documentation

- [Contributing](CONTRIBUTING.md) — Commit conventions, branch policy
- [Architecture](docs/architecture.md) — Module structure, feature flags, error variants
- [Releasing](docs/releasing.md) — Release workflow, CI/CD
- [Testing](docs/testing.md) — Testing strategy
- [Claude CLI](docs/claude-cli.md) — Observed CLI behaviors and constraints

## Compatibility

Tested against Claude Code CLI <!-- cli-version -->**v2.1.104**<!-- /cli-version -->. Older or newer versions may work but have not been verified.

Not all CLI options have dedicated `ClaudeConfigBuilder` methods. Options not yet supported can be passed via `extra_args`:

```rust
let config = claude_code::ClaudeConfig::builder()
    .extra_args(vec!["--agent".into(), "reviewer".into()])
    .build();
```

See [docs/claude-cli.md](docs/claude-cli.md) for the full option support status.

## Minimum Supported Rust Version

Rust 1.93+ (edition 2024).

## License

MIT
