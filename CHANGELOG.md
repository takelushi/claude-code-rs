# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2](https://github.com/takelushi/claude-code-rs/compare/claude-code-v0.1.1...claude-code-v0.1.2) (2026-04-08)


### Features

* add CLI version check against TESTED_CLI_VERSION ([#16](https://github.com/takelushi/claude-code-rs/issues/16)) ([8631ed1](https://github.com/takelushi/claude-code-rs/commit/8631ed1b7e890971a579dd525407e96d83d0dd8e))
* add Preset system and hang protection ([#22](https://github.com/takelushi/claude-code-rs/issues/22)) ([8efb449](https://github.com/takelushi/claude-code-rs/commit/8efb449c08e09397b17ba5c2646936cde6ede623))


### Bug Fixes

* add #[non_exhaustive] to CliVersionStatus enum ([#16](https://github.com/takelushi/claude-code-rs/issues/16)) ([a23d35d](https://github.com/takelushi/claude-code-rs/commit/a23d35d88dff3b86270f34f616d4eed0c156e57e))

## [0.1.1](https://github.com/takelushi/claude-code-rs/compare/claude-code-v0.1.0...claude-code-v0.1.1) (2026-04-07)


### Features

* add `cli_path` option for custom Claude CLI binary path ([#5](https://github.com/takelushi/claude-code-rs/issues/5)) ([91563d9](https://github.com/takelushi/claude-code-rs/commit/91563d9f639f2c040cae3fdb3c2deb689709bc52))
* add `check_cli_with_path()` public function for custom binary health checks
* add `TESTED_CLI_VERSION` constant (v2.1.92)

### Documentation

* document CLI v2.1.92 compatibility and full option support status ([#6](https://github.com/takelushi/claude-code-rs/issues/6))
* add `CONTRIBUTING.md` with commit conventions, branch policy, and merge strategy

### CI

* add branch policy enforcement for main branch
* auto-merge main into develop after release-please release ([#11](https://github.com/takelushi/claude-code-rs/issues/11))
* extend CI triggers to include develop branch

## 0.1.0 (2026-04-07)

Initial release of claude-code — a Rust library for executing Claude Code CLI as a subprocess.

### Features

* Single-shot JSON responses via `ask()`
* Real-time streaming via `ask_stream()`
* Multi-turn conversation API
* Structured output with JSON Schema
* Configurable via builder pattern with sensible defaults
