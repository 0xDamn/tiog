# Changelog

All notable changes to this project are documented here. The format loosely follows
[Keep a Changelog](https://keepachangelog.com/), and versions aim to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `timeout_secs` model option: HTTP request timeout is now configurable (default 120s) instead
  of a fixed 60s, so slow local models (e.g. Ollama with large prompts) no longer time out.
- `disable_thinking` model option: when set, the OpenAI-compatible provider sends
  `reasoning_effort: "none"` so reasoning models (e.g. Qwen3 via Ollama) skip thinking and stay
  responsive interactively. Ignored by the Anthropic provider.

### Fixed
- Text-plugin replies from smaller local/open models are parsed tolerantly: off-schema keys or
  bare prose now fall back to the most answer-like field instead of failing the request.

## [0.1.1] - 2026-06-06

### Added
- Session-scoped conversation memory: tiog remembers recent exchanges in a terminal session,
  so follow-ups like "teach me more" or "with examples" build on the previous answer.
- Prompt/config plugin routing with built-in `command` and `translator` plugins.
- Built-in `explainer` plugin for concepts, definitions, command output, and error messages.
- Built-in `interesting` plugin for short reading while waiting on terminal work.
- CLI flags for plugin control: `--list-plugins`, `--plugin <NAME>`, and `--no-context`.
- Prebuilt GitHub release packages, checksums, and the hosted `install.sh` installer.

### Fixed
- tiog no longer parrots its own previous answer: it records its output and strips it from
  future captured terminal context (it was being fed back through the scrollback).
- Local state files are redacted before persistence and created with user-only permissions.
- Repeated identical prompts reuse the cached plugin route instead of paying for another
  routing model call.
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
