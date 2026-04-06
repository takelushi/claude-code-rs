mod client;
mod config;
mod conversation;
mod error;
#[cfg(feature = "stream")]
mod stream;
#[cfg(feature = "structured")]
mod structured;
mod types;

pub use client::{ClaudeClient, CommandRunner, DefaultRunner, check_cli};
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
