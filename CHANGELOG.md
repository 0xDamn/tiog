# Changelog

All notable changes to this project are documented here. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/), and versions aim to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-06-05

Initial release.

### Added
- Plain-language → shell-command suggestions with a structured `{command, explanation,
  alternatives, risk}` contract.
- Model backends: Anthropic (tool use) and OpenAI / OpenAI-compatible (JSON mode). The
  compatible provider covers DeepSeek, Ollama, and other OpenAI-shaped endpoints via
  `base_url` (API key optional for local servers).
- fish integration: a `Ctrl-G` hotkey that injects the suggested command into your prompt,
  plus a per-session command log.
- Pluggable terminal context: `tmux` (real scrollback via `capture-pane`), `hooks`
  (commands + exit codes), and an `auto` selector. `tiog --show-context` to inspect it.
- Secret redaction of context before it is sent to a model.
- Safety model: local destructive-command detection reconciled with the model's risk, and an
  opt-in `auto_run: safe` (only `risk: none` commands auto-run via the hotkey).

### Deferred
- PTY session recorder for full output capture outside tmux — the `pty` context source is a
  placeholder for now.
