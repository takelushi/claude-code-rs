# Architecture

## File Structure

```plain
src/
  lib.rs           # pub API re-export
  client.rs        # ClaudeClient (core CLI execution: ask, ask_structured, ask_stream), check_cli, check_cli_version
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

## Feature Flags

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

## Security Considerations

### `cli_path` — no input validation

`ClaudeConfig::cli_path` accepts arbitrary strings without validation.
This is safe because `tokio::process::Command::new()` calls `execvp` directly
without invoking a shell. Shell metacharacters (`;`, `&&`, `$()`, backticks, etc.)
are treated as literal parts of the executable name and cannot trigger injection.
Arguments are likewise passed via `args()`, not string concatenation.
Path existence is validated at execution time by the OS; a missing binary
produces `ClaudeError::CliNotFound`.

## Error Variants

`ClaudeError` variants:

- `CliNotFound` — `claude` command not found in PATH
- `NonZeroExit { code, stderr }` — CLI returned a non-zero exit code
- `ParseError` — Failed to deserialize JSON / stream-json response
- `Timeout` — No response within the specified duration
- `Io` — I/O error from process spawn, stdout/stderr reads, etc.
- `StructuredOutputError { raw_result, source }` — CLI succeeded but JSON deserialization of result failed
