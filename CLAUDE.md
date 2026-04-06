# claude-code

A Rust library for executing Claude Code CLI as a subprocess.

## Project Overview

- Runs `claude` CLI in `--print` mode as a subprocess and handles results in a type-safe manner
- Output formats: `--output-format json` (single-shot) / `--output-format stream-json` (streaming)
- License: MIT
- Published on crates.io as `claude-code`

## Tech Stack

- Rust 1.93+ (edition 2024)
- Async runtime: tokio
- Serialization: serde / serde_json
- Process execution: tokio::process
- Streaming: tokio-stream / async-stream
- Error handling: thiserror
- Testing: cargo test + mockall 0.14

## Development

### Commands

```sh
cargo build                    # Build
cargo test                     # Run tests
cargo test -- --ignored        # Run E2E tests
cargo clippy                   # Lint
cargo fmt --check              # Check formatting
cargo fmt                      # Apply formatting
cargo doc --open               # Generate docs
cargo publish --dry-run        # Verify publishability
cargo run --example simple            # Basic usage
cargo run --example stream            # Streaming usage
cargo run --example stream-all        # All stream events
cargo run --example multi_turn        # Multi-turn conversation
cargo run --example structured_output # Structured output
```

### Workflow

- TDD (Explore -> Red -> Green -> Refactor)
- Must pass `cargo clippy -- -D warnings`
- Apply `cargo fmt` before committing
- Write doc comments for all pub API items

### Architecture

```plain
src/
  lib.rs           # pub API re-export
  client.rs        # ClaudeClient (core CLI execution: ask, ask_structured, ask_stream), check_cli
  config.rs        # ClaudeConfig (--model, --system-prompt, and other options)
  conversation.rs  # Conversation (automatic session_id management for multi-turn)
  types.rs         # ClaudeResponse (including parse_result), Usage, and other core types
  error.rs         # Error types
  stream.rs        # StreamEvent + stream-json parsing, iteration, and buffering
  structured.rs    # generate_schema: JsonSchema → JSON Schema string generation (structured feature)
examples/
  simple.rs        # Minimal usage example
  stream.rs        # Streaming usage example
  stream-all.rs    # All stream events example
  multi_turn.rs    # Multi-turn conversation example
  structured_output.rs  # Structured output example
```

### Feature Flags

```toml
[features]
default = ["stream", "structured", "tracing"]
stream = ["dep:tokio-stream", "dep:async-stream"]  # ask_stream, StreamEvent, Conversation stream methods
structured = ["dep:schemars"]                       # generate_schema helper
tracing = ["dep:tracing"]                           # debug/error/info logging in client.rs
```

- `default-features = false` for minimal build (`ask()` / `ask_structured()` only)
- `StreamEvent` is defined in the `stream.rs` module (gated by `stream` feature)
- tracing is absorbed via conditional macros (`trace_debug!` etc.) in `client.rs`

### Error Variants

`ClaudeError` variants:

- `CliNotFound` — `claude` command not found in PATH
- `NonZeroExit { code, stderr }` — CLI returned a non-zero exit code
- `ParseError` — Failed to deserialize JSON / stream-json response
- `Timeout` — No response within the specified duration
- `Io` — I/O error from process spawn, stdout/stderr reads, etc.
- `StructuredOutputError { raw_result, source }` — CLI succeeded but JSON deserialization of result failed

### Testing Strategy

- CLI execution is abstracted via the `CommandRunner` trait and mocked with mockall
- `tests/fixtures/` contains JSON files reproducing CLI stdout
- Unit tests: use mocks + fixtures to test each module without calling the CLI
- Integration / E2E: run the actual `claude` CLI with `--model haiku` to minimize costs
- E2E tests are marked with `#[ignore]` and run explicitly via `cargo test -- --ignored`

### Documentation Policy

- All documentation (CLAUDE.md, docs/, README, etc.) must be written in English
- All code comments and doc comments must be written in English
- When fixing bugs or design mistakes, add prevention measures to the Conventions section
- Record newly discovered modules or external specs in `docs/`
- Update this file when the Architecture file structure changes
- Record observed Claude CLI behaviors and constraints in `docs/claude-cli.md`

### Conventions

- Define errors with `thiserror` and return `Result<T, ClaudeError>`
- Build `ClaudeConfig` using the builder pattern
- Async API only; no synchronous wrappers
- Use `#[must_use]` and `#[non_exhaustive]` appropriately
- Tests must not call the actual `claude` CLI (use mocks or fixtures)
- `mockall`'s `returning` does not support async closures; for tests requiring async delays (e.g., timeout), manually implement the trait on a struct
- `MockCommandRunner` does not support `Clone`; for components that clone the runner (e.g., `Conversation`), use a manually implemented `RecordingRunner` with `Arc<Mutex>` for shared state
- `CommandRunner::run()` returns a completed `Output`, so it cannot abstract streaming; `ask_stream` is limited to `DefaultRunner` which always spawns a real process
- `CommandRunner` trait uses `#[allow(async_fn_in_trait)]` to suppress `Send` bound warnings (internal use only)
- CLI option value constraints (`effort`, `permission_mode`, etc.) use `String` + constant modules instead of enums. Claude Code CLI is actively developed, and enums would require a library release for each new value
- The library does not perform mutual exclusion checks or validation between options. Validation is the responsibility of the CLI command
