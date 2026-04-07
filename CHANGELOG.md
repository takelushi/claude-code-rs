# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1](https://github.com/takelushi/claude-code-rs/compare/claude-code-v0.1.0...claude-code-v0.1.1) (2026-04-07)


### Features

* add cli_path option for custom Claude CLI binary path ([#5](https://github.com/takelushi/claude-code-rs/issues/5)) ([91563d9](https://github.com/takelushi/claude-code-rs/commit/91563d9f639f2c040cae3fdb3c2deb689709bc52))

## 0.1.0 (2026-04-07)

Initial release of claude-code — a Rust library for executing Claude Code CLI as a subprocess.

### Features

* Single-shot JSON responses via `ask()`
* Real-time streaming via `ask_stream()`
* Multi-turn conversation API
* Structured output with JSON Schema
* Configurable via builder pattern with sensible defaults
