# tiog

**Ask your terminal in plain language — get commands or text answers grounded in your working directory.**

[![CI](https://github.com/0xDamn/tiog/actions/workflows/ci.yml/badge.svg)](https://github.com/0xDamn/tiog/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

You live in the terminal. You hit a wall — the exact `sed` flags, that `ffmpeg` incantation,
why the last command failed — and today that means tabbing over to a chatbot, asking, tabbing
back, retyping. `tiog` closes that loop: ask from any shell, get a focused answer on stdout,
and decide what to run.

What makes it more than a chat wrapper: tiog is **bound to your terminal**. It sees your
current directory, recent commands, and (inside tmux) their output — so answers are about
*your* situation, not generic.

tiog can also route requests to prompt/config plugins. The built-in `command` plugin keeps
the original command-assistant behavior, while the built-in `translator` plugin handles
context-free translation requests:

```sh
tiog local state files are written unredacted into Chinese
```

> ⚠️ **Early days (v0.1).** Works well as a shell-agnostic CLI. Full output capture currently needs
> tmux; the standalone PTY recorder is deferred (see [Roadmap](#roadmap)). Feedback and PRs
> welcome.

## Install

```sh
# from source (needs a Rust toolchain)
cargo install --git https://github.com/0xDamn/tiog

# …or clone + build
git clone https://github.com/0xDamn/tiog
cd tiog && cargo install --path .
```

This puts `tiog` on your PATH (in `~/.cargo/bin`).

## Configure a model

tiog reads your API key from an environment variable — nothing secret is stored in its
config. The default is Anthropic, but any OpenAI-compatible endpoint works (OpenAI, DeepSeek,
a local Ollama, …).

Create `~/.config/tiog/config.yaml` (full sample: [`config.example.yaml`](./config.example.yaml)):

```yaml
model:
  provider: anthropic            # anthropic | openai | openai-compatible
  name: claude-sonnet-4-6
  api_key_env: ANTHROPIC_API_KEY
```

<details><summary><b>OpenAI, DeepSeek, or local Ollama</b></summary>

```yaml
# OpenAI
model: { provider: openai, name: gpt-4o, api_key_env: OPENAI_API_KEY }

# DeepSeek (OpenAI-compatible)
model:
  provider: openai-compatible
  name: deepseek-chat
  base_url: https://api.deepseek.com
  api_key_env: DEEPSEEK_API_KEY

# Local Ollama (no key needed)
model:
  provider: openai-compatible
  name: llama3.1
  base_url: http://localhost:11434/v1
  api_key_env: UNUSED
```
</details>

## Plugins and routing

By default, tiog asks the configured model to route each request to a plugin, then falls back
to `command` when the route is uncertain. Prompt/config plugins are safe by design: they are
just descriptions and prompts, not executable code.

```yaml
router:
  enabled: true
  default_plugin: command
  min_confidence: 0.55

plugins:
  translator:
    description: "Translate text between human languages."
    output: text
    include_context: false
    include_conversation: false
    system_prompt: |
      Translate faithfully. Preserve technical terms when appropriate.
```

Text plugins print the short explanation to stderr and the main result to stdout.

Useful plugin flags:

```sh
tiog --list-plugins
tiog --plugin translator local state files are written unredacted into Chinese
tiog --no-context how do I list files by size
```

Then set the key and ask:

```sh
export ANTHROPIC_API_KEY=sk-ant-...
tiog -- how do I list files by size, largest first
```

The explanation prints to stderr and the command or text result prints to stdout, so
`tiog -- … | pbcopy` copies just the main result.

## How it works

tiog gathers context from a pluggable source, scrubs secrets, and sends it to your model,
which returns a structured `{command, explanation, risk}`.

| Source | Sees output? | Needs |
|---|---|---|
| `tmux` | ✅ real scrollback (`capture-pane`) | being inside tmux |
| `hooks` | ❌ commands + exit codes | `$TIOG_SESSION_LOG` in `exit<TAB>cwd<TAB>command` format |
| `auto` *(default)* | best available | — |

Inspect the command plugin's terminal context, with no API call:

```sh
tiog --show-context
```

## Safety & privacy

- **Secrets are redacted** from context before sending (API keys, tokens, `KEY=value`, JWTs…).
- **Destructive commands** (`rm -rf`, `dd`, force-push, …) are flagged even if the model
  doesn't mark them.
- **No automatic execution** — tiog prints suggestions; your shell decides what runs.

## Roadmap

- ✅ Multi-provider queries · routed prompt plugins · tmux/hooks context · redaction · safety.
- ⏸ **PTY session recorder** — full output capture outside tmux (`pty` source is a placeholder).
- ◻ Streaming output · follow-up/refine · optional shell integrations · `tiog init` installer.

Architecture and milestone detail live in [DESIGN.md](./DESIGN.md).

## Contributing

Issues and PRs welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md). Keep `cargo test` and
`cargo clippy --all-targets -- -D warnings` green; `cargo fmt` keeps style consistent.

## License

Licensed under either of [Apache License 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT)
at your option. Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion shall be dual-licensed as above, without additional terms.
