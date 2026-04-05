# claude-code-rs

Claude Code CLI をRustから実行するためのライブラリ。

## Features

- `claude --print` をサブプロセスとして実行
- JSON / stream-json 出力の型安全なパース
- Builder パターンによる柔軟なオプション設定
- tokio ベースの非同期 API

## Installation

```toml
[dependencies]
claude-code-rs = "0.1"
```

## Usage

```rust
use claude_code_rs::{ClaudeClient, ClaudeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClaudeConfig::builder()
        .model("sonnet")
        .build();

    let client = ClaudeClient::new(config);
    let response = client.ask("Hello, Claude!").await?;

    println!("{}", response.text());
    Ok(())
}
```

> **Note:** API は開発中のため、変更される可能性があります。

## Requirements

- Rust 1.93+
- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) がインストール済みであること

## License

MIT
