# tiog — Design

> A terminal-native command agent. You stay in your shell, describe what you want in
> plain language, and `tiog` puts the right command into your prompt — using what it can
> see of your current session.

## 1. Motivation

Living in the terminal means constant small interruptions: you forget the exact flag for
`sed`, the `tar` invocation, the `ffmpeg` incantation — so you tab over to a chat/coding
agent, ask, read, tab back, retype, and resume. `tiog` collapses that loop. You ask
in-place; it answers in-place; the command lands in your prompt ready to review and run.

The thing that makes `tiog` more than another LLM wrapper is that it is **bound to the
terminal**: it can see your current directory, recent commands, and (when enabled) their
output — so its answers are about *your* situation, not generic.

## 2. Core idea / UX

Two ways to invoke, both backed by the same engine:

- **Hotkey (primary):** type your request as the command line, press a hotkey
  (default `Ctrl-G`); `tiog` replaces the line with the suggested command. No quoting.
- **`tiog …` command:** `tiog replace foo with bar in all .txt files` (args are joined,
  no quotes needed). Prints a short explanation and stages the command.

The command is **pasted, never auto-run by default** — you review and press Enter.

## 3. Architecture overview

```
┌─ fish integration ("the hands") ───────────────┐
│  • Ctrl-G binding: query → commandline -r       │
│  • `tiog` function                              │
│  • preexec/postexec hooks → OSC 133 + session log│
└───────────────┬─────────────────────────────────┘
                │ invokes
┌───────────────▼── tiog (Rust, "the brain") ─────┐
│  1. gather context (source = auto|tmux|pty|hooks)│
│  2. redact secrets (on send)                     │
│  3. call backend model (config.yaml)             │
│  4. return {command, explanation, risk, …}       │
└──────────────────────────────────────────────────┘
```

A plain child process cannot write into its parent shell's edit buffer (the old `TIOCSTI`
trick is disabled/restricted on modern macOS & Linux). So the "paste into prompt" magic
**must** be done by shell integration, not the binary alone. The binary is the brain; a
thin fish layer is the hands.

## 4. Terminal context — pluggable `ContextSource`

Where command *output* comes from is configurable. The user picks; `auto` does the right
thing per environment.

| Source | Gets output? | Cost / requirement | Best for |
|---|---|---|---|
| `tmux` | ✅ via `capture-pane` | must be inside tmux; no extra process | tmux users (seamless) |
| `pty` (recorder) | ✅ full stream | launch shell under `tiog session` | non-tmux users |
| `hooks` | ❌ commands + exit codes only | nothing — always works | universal fallback |
| `auto` *(default)* | best available | — | most people |

**`auto` resolution:** `TIOG_SESSION` set → `pty`; else `$TMUX` set → `tmux`; else `hooks`.

Nothing is re-exec'd by default. The PTY recorder is opt-in (run `tiog session`, or set
`record.auto_attach: true`). tmux users get output for free.

### PTY recorder (`pty` source) — deferred (placeholder)

`tiog session` spawns the user's `$SHELL` inside a pseudo-terminal, proxies keystrokes and
output transparently (raw passthrough, `SIGWINCH` resize forwarding, exit-code
propagation), and tees the byte stream into a capped per-session transcript with `0600`
perms. It exports `TIOG_SESSION` and `TIOG_SESSION_FILE` into the shell env so the query
process can find the transcript.

### OSC 133 segmentation

Raw bytes aren't enough — we need "this command → this output → exit 1" boundaries. The
fish hooks emit the standard **OSC 133** shell-integration escapes (prompt/command/output/
exit markers, invisible to the user); the recorder slices the stream on them. This is the
same mechanism iTerm2 / WezTerm / VS Code use, so we don't invent a marker format.

### Graceful degradation

Inside a recorded (or tmux) session you get full output. Outside one, the same fish hooks
still log commands + exit codes + cwd, so `tiog` always has *something* — it just loses
real output. No hard dependency on being wrapped.

## 5. Command injection (fish)

- **Hotkey path (reliable) — ✅ confirmed (fish 4.2.1).** A fish key binding reads the
  current `commandline`, runs the engine, and replaces the buffer with `commandline -r`.
  Verified end-to-end with a hermetic PTY test: typed line → Ctrl-G → buffer becomes the
  suggested command, with the explanation shown dimmed above the prompt.
- **`tiog …` command path — resolved.** fish has no `print -z` equivalent; a plain command
  cannot set the *next* prompt's buffer. So the command form does not inject — it prints
  the explanation (stderr) and the command (stdout), which pipes cleanly:
  `tiog … | pbcopy` copies just the command. The Ctrl-G hotkey is the in-place path.

## 6. Model backends

Three provider backends, dispatched by `model::suggest`:

- `anthropic` — Messages API, structured output via tool use. ✅
- `openai` — Chat Completions, JSON mode + lenient parse. ✅
- `openai-compatible` — any compatible endpoint via `base_url` (covers Ollama / local; API
  key optional). ✅

API keys are read from an env var named in config (`api_key_env`); never stored in the file.

## 7. Model I/O contract

The model returns a structured object (tool use for Anthropic, `json_schema` for OpenAI,
lenient JSON parse for local):

```json
{ "command": "sed -i '' 's/foo/bar/g' file.txt",
  "explanation": "macOS sed needs '' after -i for in-place edits.",
  "alternatives": ["perl -pi -e 's/foo/bar/g' file.txt"],
  "risk": "none | caution | destructive",
  "needs": null }
```

- `explanation` → stderr; `command` → stdout / injected.
- `risk: destructive` always warns, even under `auto_run: safe`.
- Empty `command` + `needs` set → the request was too vague; show what's missing.

## 8. Configuration — `~/.config/tiog/config.yaml`

> On macOS this is `~/.config/...` (XDG-style), **not** `~/Library/Application Support`.
> Honors `$XDG_CONFIG_HOME` when set.

```yaml
model:
  provider: anthropic            # anthropic | openai | openai-compatible
  name: claude-sonnet-4-6
  api_key_env: ANTHROPIC_API_KEY # key from env, never in this file
  base_url: null                 # openai-compatible / local (ollama)
context:
  source: auto                   # auto | tmux | hooks  (pty reserved for the M4 recorder)
  history_lines: 40
  redact: true                   # scrub secrets before send
behavior:
  auto_run: "off"                # "off" | "safe" (hotkey auto-runs risk:none commands)
```

Missing and partial files both work — every field falls back to a built-in default, and
unknown keys are ignored. Config reserved for deferred milestones (parsed-but-ignored if
present): the `record:` section (M4 recorder), `behavior.hotkey`/`paste_mode`, and
`context.send_env`.

## 9. Privacy & safety

- **Redaction on send:** the local transcript stays faithful; secrets (tokens, keys,
  passwords, high-entropy strings, known env values) are scrubbed only from the payload
  sent to the model.
- **Transcript at rest:** capped size, `0600` perms, never contains the API key.
- **Execution:** paste-only by default; destructive commands flagged; `auto_run: safe` is
  strictly opt-in and still confirms destructive actions.

## 10. Module layout

```
src/
  main.rs / cli.rs        # clap entry + dispatch
  config.rs               # ~/.config/tiog/config.yaml + defaults
  query.rs                # orchestrate: gather → redact → model → result
  context.rs              # M1: local gather (cwd/git/ls)
   └─ context/            # M3+: ContextSource trait + tmux/recorder/hooks
  record/                 # M4: `tiog session` PTY recorder
    pty.rs  osc133.rs  transcript.rs
  model/                  # Provider trait
    mod.rs  anthropic.rs  openai.rs  compatible.rs  prompt.rs
  redact.rs  output.rs  inject.rs
shell/tiog.fish           # conf.d: OSC133 hooks, `tiog` fn, Ctrl-G binding
```

## 11. Milestones

Each milestone is independently usable.

- **M1 — One-shot query.** ✅ *scaffolded.* clap skeleton + config + Anthropic provider +
  structured result, with local `gather` context (OS/shell/cwd/git/ls). Usable as
  `tiog "…"`.
- **M2 — fish integration + injection.** ✅ *done.* `shell/tiog.fish`: Ctrl-G hotkey
  (`commandline -r` injection, verified by a hermetic PTY test), a per-session command log
  (`fish_postexec` → exit code + cwd + command), and the `tiog … | pbcopy` command form.
  Core loop works.
- **M3 — Pluggable sources.** ✅ *done.* `ContextSource` trait + `tmux` (capture-pane,
  real output), `hooks` (reads `$TIOG_SESSION_LOG`), a `pty` stub, and the `auto` selector.
  `tiog --show-context` dumps the assembled context with no API call. Verified: hooks
  formatting (exit codes + cwd annotation, tiog calls filtered) and a live tmux capture.
- **M4 — PTY recorder.** ⏸ *deferred (placeholder).* Kept as a `PtySource` stub that
  yields nothing, so `auto` falls back to `tmux`/`hooks` — non-tmux users get the command
  list without output until this lands. To implement: `tiog session` spawns `$SHELL` in a
  PTY (raw passthrough, `SIGWINCH` resize, exit-code propagation), parses **OSC 133** to
  segment the stream, tees to a capped `0600` transcript, exports `TIOG_SESSION` /
  `TIOG_SESSION_FILE`, and fills in `PtySource::collect`.
- **M5 — Providers + safety.** ✅ *done.* `openai` + `openai-compatible` (JSON mode, lenient
  parse; key optional for local), secret **redaction on send**, local **destructive-command
  detection** reconciled with the model's risk (worse wins), and `auto_run: safe` via the
  `--shell` exit code (10 = paste+run, 0 = paste) honored by the fish hotkey. Verified: unit
  tests (redact/risk) + mock-server integration (provider, reconciliation, codes, redaction).
- **M6 — Polish.** Streaming explanation, follow-up/refine, zsh/bash integration,
  `tiog init` installer.

## 12. Open questions / spikes

- ~~Fish injection from the command form (M2)~~ — **resolved:** the Ctrl-G hotkey injects
  via `commandline -r` (confirmed by PTY test); a plain command cannot set the next prompt,
  so the command form prints + pipes (`| pbcopy`) instead.
- **OSC 133 emission** (M3/M4) — does the user's fish emit OSC 133 natively, or do we wrap
  `fish_prompt`/`fish_preexec`?
- **macOS PTY** (M4) — raw mode, signal forwarding, and resize corner cases.
- **Streaming** — stream the explanation while preparing the command (latency feel).

## 13. Decisions log

- **Context depth:** full output capture wanted → made the **output source pluggable**
  (`tmux` | `pty` | `hooks` | `auto`) rather than forcing one mechanism.
- **Invocation:** support **both** the Ctrl-G hotkey and the `tiog …` command.
- **Backends:** Anthropic + OpenAI + OpenAI-compatible (latter covers local/Ollama).
- **Safety:** **configurable**; default paste-only, opt-in `auto_run: safe`.
- **Transcript:** **redact on send**, keep a capped local log with tight perms.
- **Default model:** `anthropic / claude-sonnet-4-6`; key from `$ANTHROPIC_API_KEY`.
- **M4 (PTY recorder) deferred:** kept as a `PtySource` stub; `auto` falls back to
  tmux/hooks, so non-tmux users get the command list without output. Revisit when full
  output capture outside tmux is needed.
