# Contributing to tiog

Thanks for your interest! tiog is early (v0.1) and contributions — issues, ideas, and PRs —
are very welcome.

## Development

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

CI runs rustfmt, clippy (`-D warnings`), and the test suite on every push and pull request,
so keep those green.

## How it's organized

The codebase is built in milestones; [DESIGN.md](./DESIGN.md) is the source of truth for the
architecture — the `ContextSource` model, the provider backends, the safety/redaction model,
and what's deferred (the PTY recorder).

Handy while hacking:

- `tiog --show-context` prints the exact (redacted) context tiog would send — no API call.
- A tiny local mock OpenAI-compatible server makes it easy to test providers, redaction, and
  the `--shell` exit codes without real API keys.

## Guidelines

- Keep `cargo clippy -D warnings` and `cargo fmt` clean.
- Add a unit test for new pure logic (see `src/redact.rs` and `src/risk.rs` for the pattern).
- Respect the safety model: a destructive command must never silently auto-run.
- Match the surrounding style and comment density.

By contributing, you agree that your contributions are dual-licensed under
[MIT](./LICENSE-MIT) OR [Apache-2.0](./LICENSE-APACHE), without additional terms.
