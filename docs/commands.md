# Commands

## Build & Test

```sh
cargo build                    # Build
cargo test                     # Run tests
cargo test -- --ignored        # Run E2E tests
cargo clippy                   # Lint
cargo fmt --check              # Check formatting
cargo fmt                      # Apply formatting
```

## Documentation

```sh
cargo doc --open               # Generate docs
```

## Publishing

```sh
cargo publish --dry-run        # Verify publishability
```

## Examples

```sh
cargo run --example simple            # Basic usage
cargo run --example stream            # Streaming usage
cargo run --example stream-all        # All stream events
cargo run --example multi_turn        # Multi-turn conversation
cargo run --example structured_output # Structured output
```
