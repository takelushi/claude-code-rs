# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.0 (2026-04-07)

Initial release of claude-code — a Rust library for executing Claude Code CLI as a subprocess.

### Features

* Single-shot JSON responses via `ask()`
* Real-time streaming via `ask_stream()`
* Multi-turn conversation API
* Structured output with JSON Schema
* Configurable via builder pattern with sensible defaults
