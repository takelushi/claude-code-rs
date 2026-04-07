# Claude CLI Behavior and Constraints

Notes on Claude Code CLI behaviors and constraints discovered during implementation.

## Tested CLI Version

| Item | Value |
| --- | --- |
| CLI version | 2.1.92 |
| Verified on | 2026-04-07 |

This is the version used during development and testing. The library may work with other versions but they have not been verified.

## ANSI Escape Sequence Contamination

ANSI escape sequences (e.g., `\x1b[?1004l`) may appear in stdout when using `--output-format json` / `stream-json`. These must be stripped before JSON parsing.

## Context Minimization Configuration

The following option combination minimizes the context injected by Claude Code (reduced to approximately 100 tokens):

| Option | What it reduces |
| --- | --- |
| `--system-prompt ''` | Empties the default system prompt |
| `--setting-sources ''` | Skips loading user/project/local settings |
| `--strict-mcp-config` | Ignores MCP configurations not specified via `--mcp-config` |
| `--mcp-config '{"mcpServers":{}}'` | Sets MCP servers to zero |
| `--tools ''` | Excludes all built-in tool definitions (~9000 token reduction) |
| `--disable-slash-commands` | Disables slash commands (skills) |

The remaining ~100 tokens are from Claude Code's hardcoded base prompt (`currentDate` + `You are a Claude agent...`).

## stream-json Requires --verbose

`--output-format stream-json` requires `--verbose`; otherwise the CLI returns an error.

## --max-turns Behavior

Setting `--max-turns 1` stops after a single response without tool use. Useful for minimizing costs in E2E tests.

## --no-session-persistence

Prevents sessions from being saved to disk. Use when session resumption via `--resume` is not needed (e.g., single-shot library calls).

### Constraint with --resume

The library defaults to `--no-session-persistence` (sessions are not persisted). When `--resume <session_id>` is specified in this state, the CLI returns an error because the session does not exist on disk.

For multi-turn conversations (the `Conversation` API), you must explicitly set `no_session_persistence(false)` to enable session persistence.

## stream-json Event Types

Events output as NDJSON via `--output-format stream-json --verbose`:

| type | subtype / content type | Description |
| --- | --- | --- |
| `system` | `init` | Session initialization info (session_id, model, etc.) |
| `system` | `hook_started` / `hook_response` | Hook execution (skipped by the library) |
| `assistant` | content[].type = `thinking` | Model's thinking process |
| `assistant` | content[].type = `text` | Text response chunk |
| `assistant` | content[].type = `tool_use` | Tool invocation |
| `user` | content[].type = `tool_result` | Tool execution result |
| `rate_limit_event` | — | Rate limit information |
| `result` | `success` | Final result (same structure as `--output-format json`) |
| `stream_event` | (various, see below) | Anthropic Messages API SSE events (only with `--include-partial-messages`) |

### Multiple Elements in the content Array

A single `assistant` / `user` event's `.message.content[]` may contain multiple blocks. The library yields each element as a separate `StreamEvent`.

### Relationship Between assistant Events and stream_events

When `--include-partial-messages` is enabled, both `assistant` events (complete messages) and `stream_event` events (token-level chunks) are sent. Since the same text arrives twice, the library distinguishes them as follows:

| Source | StreamEvent Variant | Purpose |
| --- | --- | --- |
| `stream_event` / `text_delta` | `Text` | Real-time display |
| `stream_event` / `thinking_delta` | `Thinking` | Real-time display |
| `assistant` / text | `AssistantText` | Retrieve complete text |
| `assistant` / thinking | `AssistantThinking` | Retrieve complete text |

### stream_event Types (Real-time Streaming)

When `--include-partial-messages` is enabled, Anthropic Messages API SSE events are wrapped in `stream_event` type and sent.

Structure: `{"type": "stream_event", "event": {"type": "<event_type>", ...}}`

#### Event Type Reference

| event.type | StreamEvent Variant | Description |
| --- | --- | --- |
| `message_start` | `MessageStart` | Message start (model name, ID) |
| `content_block_start` | `ContentBlockStart` | Block start (index, block_type) |
| `content_block_delta` | Various delta variants | Token-level chunks (see below) |
| `content_block_stop` | `ContentBlockStop` | Block end (index) |
| `message_delta` | `MessageDelta` | stop_reason, etc. |
| `message_stop` | `MessageStop` | Message complete |
| `ping` | `Ping` | Keepalive |
| `error` | `Error` | Error notification |

#### content_block_delta Delta Types

| event.delta.type | StreamEvent Variant | Description |
| --- | --- | --- |
| `text_delta` | `Text` | Text chunk (`.delta.text`) |
| `thinking_delta` | `Thinking` | Thinking chunk (`.delta.thinking`) |
| `input_json_delta` | `InputJsonDelta` | Partial tool input JSON (`.delta.partial_json`) |
| `signature_delta` | `SignatureDelta` | Thinking signature (`.delta.signature`) |
| `citations_delta` | `CitationsDelta` | Citation information (`.delta.citation`) |

#### Event Delivery Order

```plain
message_start
→ content_block_start (index=0, type=thinking)
→ thinking_delta (multiple)
→ signature_delta
→ content_block_stop (index=0)
→ content_block_start (index=1, type=text)
→ text_delta (multiple)
→ content_block_stop (index=1)
→ message_delta (stop_reason)
→ message_stop
```

### --include-partial-messages

With this option, additional `stream_event` type events are sent, streaming text in real-time as token-level chunks. Without it (default), only completed messages are sent as `assistant` events.

For real-time display, enable `include_partial_messages(true)` and use the `Text` / `Thinking` variants. For complete text only, use `AssistantText` / `AssistantThinking`.

## tokio Child Drop Behavior

tokio's `Child` does not kill the process on drop. It is merely detached and may remain as a zombie until the parent process exits.

The library addresses this by using a `ChildGuard` RAII wrapper in `ask_stream` that sends SIGKILL via `start_kill()` when the stream is dropped. `start_kill()` sends the signal synchronously (not async), so it can be called from within the `Drop` trait.

`start_kill()` only sends the signal without calling `wait()`, so the process temporarily becomes a zombie after receiving SIGKILL. tokio's internal process reaper automatically cleans it up, so this is not a practical issue.

## CLI Option Support Status

Classification of all `claude` CLI options as of v2.1.92. The library operates in `--print` mode only.

### Supported

These options have dedicated `ClaudeConfigBuilder` methods:

| CLI Option | Builder Method |
| --- | --- |
| `--model` | `model()` |
| `--system-prompt` | `system_prompt()` |
| `--append-system-prompt` | `append_system_prompt()` |
| `--max-turns` | `max_turns()` |
| `--fallback-model` | `fallback_model()` |
| `--effort` | `effort()` |
| `--max-budget-usd` | `max_budget_usd()` |
| `--allowedTools` | `allowed_tools()` / `add_allowed_tool()` |
| `--disallowedTools` | `disallowed_tools()` / `add_disallowed_tool()` |
| `--tools` | `tools()` |
| `--mcp-config` | `mcp_configs()` / `add_mcp_config()` |
| `--setting-sources` | `setting_sources()` |
| `--settings` | `settings()` |
| `--json-schema` | `json_schema()` |
| `--include-partial-messages` | `include_partial_messages()` |
| `--include-hook-events` | `include_hook_events()` |
| `--permission-mode` | `permission_mode()` |
| `--dangerously-skip-permissions` | `dangerously_skip_permissions()` |
| `--add-dir` | `add_dirs()` / `add_dir()` |
| `--file` | `files()` / `file()` |
| `--resume` | `resume()` |
| `--session-id` | `session_id()` |
| `--bare` | `bare()` |
| `--no-session-persistence` | `no_session_persistence()` |
| `--disable-slash-commands` | `disable_slash_commands()` |
| `--strict-mcp-config` | `strict_mcp_config()` |

### Known Unsupported

Relevant to `--print` mode but not yet implemented as builder methods. All of these can be passed via `extra_args()`.

| CLI Option | Description |
| --- | --- |
| `--agent` | Agent for the current session |
| `--agents` | JSON object defining custom agents |
| `--betas` | Beta headers for API requests |
| `--continue` | Continue most recent conversation |
| `--fork-session` | Create new session ID when resuming |
| `--input-format` | Input format (`text` or `stream-json`) |
| `--name` | Session display name |
| `--allow-dangerously-skip-permissions` | Enable permission bypass as an option |
| `--verbose` | Explicit verbose mode (auto-added for stream-json) |
| `--debug` | Enable debug mode with optional category filtering |
| `--debug-file` | Write debug logs to a specific file path |

### Interactive-Only (Not Applicable)

These options are for interactive CLI sessions and do not apply to `--print` mode:

`--chrome`, `--no-chrome`, `--ide`, `--tmux`, `--worktree`, `--from-pr`, `--remote-control-session-name-prefix`, `--replay-user-messages`, `--plugin-dir`

### Managed Internally

The following options are injected automatically by the library. Do not pass them via `extra_args()` as duplicating them may cause unpredictable CLI behavior.

| CLI Option | When Applied |
| --- | --- |
| `--print` | Always |
| `--output-format` | Always (`json` for `ask`, `stream-json` for `ask_stream`) |
| `--verbose` | Automatically added when using `ask_stream` |

## Updating for New CLI Versions

### Automated workflow

The `cli-version-check.yml` workflow runs weekly (Monday 00:00 UTC) and detects new Claude CLI releases via the npm registry. When a new version is found it:

1. Runs `cargo test` and `cargo clippy` — on failure, `claude-code-action` creates a fix PR
2. Diffs `claude --help` output against `.claude-cli-help-output` — on changes, `claude-code-action` creates PRs for option changes and/or documentation updates
3. Creates a version bump PR updating `.claude-cli-version` and `.claude-cli-help-output`

All PRs target `develop`. The workflow uses Max plan authentication (`CLAUDE_CODE_OAUTH_TOKEN`).

Tracked files in the repository root:

| File | Purpose |
| --- | --- |
| `.claude-cli-version` | Last checked CLI version |
| `.claude-cli-help-output` | Last captured `claude --help` output for diffing |

### Manual checklist

For maintainers reviewing automated PRs or updating manually:

1. Run `claude --version` and `claude --help` to identify changes
2. Compare `--help` output against the option support status tables above
3. Categorize new options into Supported, Known Unsupported, or Interactive-Only
4. Run `cargo test` to check for regressions in output parsing
5. Update the tested version and date in:
   - `src/lib.rs` (`TESTED_CLI_VERSION` constant)
   - `README.md` (Compatibility section)
   - This file (Tested CLI Version table)
6. If the output format (`--output-format json` / `stream-json`) has changed, update types in `src/types.rs` and `src/stream.rs`
