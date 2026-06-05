# Changelog

All notable changes to this project are documented here. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/), and versions aim to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Session-scoped conversation memory: tiog remembers recent exchanges in a terminal session,
  so follow-ups like "teach me more" or "with examples" build on the previous answer.
- Prompt/config plugin routing with built-in `command` and `translator` plugins.
- Built-in `explainer` plugin for concepts, definitions, command output, and error messages.
- CLI flags for plugin control: `--list-plugins`, `--plugin <NAME>`, and `--no-context`.

### Fixed
- tiog no longer parrots its own previous answer: it records its output and strips it from
  future captured terminal context (it was being fed back through the scrollback).
- Local state files are redacted before persistence and created with user-only permissions.
- Long-form destructive `rm` flags are detected by the local risk classifier.

## [0.1.0] - 2026-06-05

Initial release.

### Added
- Plain-language → shell-command suggestions with a structured `{command, explanation,
  alternatives, risk}` contract.
- Model backends: Anthropic (tool use) and OpenAI / OpenAI-compatible (JSON mode). The
  compatible provider covers DeepSeek, Ollama, and other OpenAI-shaped endpoints via
  `base_url` (API key optional for local servers).
- Pluggable terminal context: `tmux` (real scrollback via `capture-pane`), `hooks`
  (commands + exit codes), and an `auto` selector. `tiog --show-context` to inspect it.
- Secret redaction of context before it is sent to a model.
- Safety model: local destructive-command detection reconciled with the model's risk.

### Deferred
- PTY session recorder for full output capture outside tmux — the `pty` context source is a
  placeholder for now.
