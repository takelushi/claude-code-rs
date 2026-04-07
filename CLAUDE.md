# claude-code

A Rust library for executing Claude Code CLI as a subprocess.

## Project Overview

- Runs `claude` CLI in `--print` mode as a subprocess and handles results in a type-safe manner
- Output formats: `--output-format json` (single-shot) / `--output-format stream-json` (streaming)
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
cargo build    # Build
cargo test     # Run tests
cargo clippy   # Lint
cargo fmt      # Apply formatting
```

Full command reference: `docs/commands.md`

### Workflow

- TDD (Explore -> Red -> Green -> Refactor)
- Must pass `cargo clippy -- -D warnings`
- Apply `cargo fmt` before committing
- Write doc comments for all pub API items

### Documentation Policy

- All documentation (CLAUDE.md, docs/, README, etc.) must be written in English
- All code comments and doc comments must be written in English
- When fixing bugs or design mistakes, add prevention measures to the Conventions section
- Record newly discovered modules or external specs in `docs/`
- Update this file when the Architecture file structure changes
- Record observed Claude CLI behaviors and constraints in `docs/claude-cli.md`
- Architecture, feature flags, and error variants are documented in `docs/architecture.md`
- Testing strategy details are documented in `docs/testing.md`
- Release workflow, commit conventions, and CI/CD setup are documented in `docs/releasing.md`

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
