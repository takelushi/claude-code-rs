# Testing Strategy

- CLI execution is abstracted via the `CommandRunner` trait and mocked with mockall
- `tests/fixtures/` contains JSON files reproducing CLI stdout
- Unit tests: use mocks + fixtures to test each module without calling the CLI
- Integration / E2E: run the actual `claude` CLI with `--model haiku` to minimize costs
- E2E tests are marked with `#[ignore]` and run explicitly via `cargo test -- --ignored`
