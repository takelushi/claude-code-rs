# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.0 (2026-04-07)


### Features

* crates.io publication readiness improvements ([#1](https://github.com/takelushi/claude-code-rs/issues/1)) ([8a7c96b](https://github.com/takelushi/claude-code-rs/commit/8a7c96b1bde0ce47ceac09b40ae9bea780645c8c))

## [0.1.0] - 2026-04-06

Initial release.

### Added

- Single-shot JSON responses via `ClaudeClient::ask`
- Real-time streaming via `ClaudeClient::ask_stream` (stream-json)
- Multi-turn conversation API with automatic session management (`Conversation`)
- Structured output with JSON Schema via `ClaudeClient::ask_structured` and `generate_schema`
- Feature flags: `stream`, `structured`, `tracing` (all enabled by default)
- Context minimization defaults for cost-efficient CLI usage
- CLI availability check via `check_cli`
- Timeout support and stream idle timeout
- `ChildGuard` RAII wrapper for reliable process cleanup on stream drop

[0.1.0]: https://github.com/takelushi/claude-code-rs/releases/tag/v0.1.0
