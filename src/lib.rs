//! # claude-code
//!
//! **Unofficial** Rust client for the [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code).
//!
//! This crate runs `claude --print` as a subprocess and provides type-safe access to the results.
//! It supports single-shot JSON responses, real-time streaming via `stream-json`, multi-turn
//! conversations, and structured output with JSON Schema.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! # #[tokio::main]
//! # async fn main() -> Result<(), claude_code::ClaudeError> {
//! let client = claude_code::ClaudeClient::new(claude_code::ClaudeConfig::default());
//! let response = client.ask("Say hello").await?;
//! println!("{}", response.result);
//! # Ok(())
//! # }
//! ```
//!
//! ## Feature Flags
//!
//! | Feature | Default | Description |
//! |---|---|---|
//! | `stream` | Yes | Enables [`StreamEvent`], streaming methods, and re-exports [`StreamExt`]. |
//! | `structured` | Yes | Enables [`generate_schema`] for JSON Schema generation. |
//! | `tracing` | Yes | Enables debug/error/info logging via `tracing`. |

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

/// The Claude Code CLI version that this library was tested against.
///
/// This does not guarantee compatibility with this exact version only;
/// it indicates the version used during development and testing.
/// Older or newer CLI versions may work but have not been verified.
pub const TESTED_CLI_VERSION: &str = "2.1.92";

mod client;
mod config;
mod conversation;
mod error;
#[cfg(feature = "stream")]
mod stream;
#[cfg(feature = "structured")]
mod structured;
mod types;

pub use client::{ClaudeClient, CommandRunner, DefaultRunner, check_cli, check_cli_with_path};
pub use config::{ClaudeConfig, ClaudeConfigBuilder, effort, permission_mode};
pub use conversation::Conversation;
pub use error::ClaudeError;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use stream::StreamEvent;
#[cfg(feature = "structured")]
#[cfg_attr(docsrs, doc(cfg(feature = "structured")))]
pub use structured::generate_schema;
#[cfg(feature = "stream")]
#[cfg_attr(docsrs, doc(cfg(feature = "stream")))]
pub use tokio_stream::StreamExt;
pub use types::{ClaudeResponse, Usage};
