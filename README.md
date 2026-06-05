# tiog

**Ask your terminal in plain language — get the command, in your prompt, grounded in what you're actually doing.**

[![CI](https://github.com/0xDamn/tiog/actions/workflows/ci.yml/badge.svg)](https://github.com/0xDamn/tiog/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

You live in the terminal. You hit a wall — the exact `sed` flags, that `ffmpeg` incantation,
why the last command failed — and today that means tabbing over to a chatbot, asking, tabbing
back, retyping. `tiog` closes that loop: ask in place, and the answer lands in your prompt,
ready to review and run.

What makes it more than a chat wrapper: tiog is **bound to your terminal**. It sees your
current directory, recent commands, and (inside tmux) their output — so answers are about
*your* situation, not generic.

```text
~/proj > replace all foo with bar in every .txt        [Ctrl-G]
         macOS sed needs '' after -i for in-place edits.
~/proj > sed -i '' 's/foo/bar/g' *.txt                 [Enter]
```

> ⚠️ **Early days (v0.1).** Works well on macOS + fish. Full output capture currently needs
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

Then set the key and ask:

```sh
set -x ANTHROPIC_API_KEY sk-ant-...     # fish
tiog -- how do I list files by size, largest first
```

The explanation prints to stderr and the command to stdout, so `tiog -- … | pbcopy` copies
just the command.

## The Ctrl-G hotkey (fish)

In-place injection is the whole point. Install the fish integration:

```sh
ln -s (path resolve shell/tiog.fish) ~/.config/fish/conf.d/tiog.fish
exec fish
```

Now **type your request on the command line and press `Ctrl-G`** — tiog replaces the line
with the suggested command (explanation shown dimmed above it). Review, press Enter. Rebind by
editing the `bind \cg` line in [`shell/tiog.fish`](./shell/tiog.fish).

## How it works

tiog gathers context from a pluggable source, scrubs secrets, and sends it to your model,
which returns a structured `{command, explanation, risk}`.

| Source | Sees output? | Needs |
|---|---|---|
| `tmux` | ✅ real scrollback (`capture-pane`) | being inside tmux |
| `hooks` | ❌ commands + exit codes | the fish integration's session log |
| `auto` *(default)* | best available | — |

Inspect exactly what would be sent, with no API call:

```sh
tiog --show-context
```

## Safety & privacy

- **Secrets are redacted** from context before sending (API keys, tokens, `KEY=value`, JWTs…).
- **Destructive commands** (`rm -rf`, `dd`, force-push, …) are flagged even if the model
  doesn't mark them.
- **Paste-only by default** — nothing runs until you press Enter. Opt into
  `behavior.auto_run: safe` to let the hotkey auto-run *only* `risk: none` commands.

## Roadmap

- ✅ Multi-provider queries · fish Ctrl-G injection · tmux/hooks context · redaction · safety.
- ⏸ **PTY session recorder** — full output capture outside tmux (`pty` source is a placeholder).
- ◻ Streaming output · follow-up/refine · zsh & bash integration · `tiog init` installer.

Architecture and milestone detail live in [DESIGN.md](./DESIGN.md).

## Contributing

Issues and PRs welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md). Keep `cargo test` and
`cargo clippy --all-targets -- -D warnings` green; `cargo fmt` keeps style consistent.

## License

Licensed under either of [Apache License 2.0](./LICENSE-APACHE) or [MIT license](./LICENSE-MIT)
at your option. Unless you explicitly state otherwise, any contribution you intentionally
submit for inclusion shall be dual-licensed as above, without additional terms.
