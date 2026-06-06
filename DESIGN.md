# tiog — Design

> A shell-agnostic terminal assistant. Ask from any shell, and `tiog` returns either a
> command suggestion or a text result using model-routed prompt/config plugins.

## 1. Core UX

`tiog` is a normal CLI binary:

```sh
tiog how do I list files by size, largest first
tiog --plugin explainer what is inode
tiog --plugin translator local state files are written unredacted into Chinese
tiog --no-context explain tar -xzf
```

For command responses, the explanation is printed to stderr and the command is printed to
stdout. For text responses, only the text result is printed to stdout in normal output.
`--json` keeps all structured fields. This keeps piping predictable:

```sh
tiog how do I list files by size | pbcopy
```

`behavior.output_language` can request a preferred language for user-visible prose. `auto`
keeps the model's default behavior, and explicit user language requests override the
preference. Shell commands, flags, paths, code, and other literals remain unchanged.

## 2. Architecture

```
┌────────────── tiog CLI ──────────────┐
│  1. parse config + CLI overrides      │
│  2. route request to a plugin         │
│  3. gather configured context         │
│  4. redact context/local memory       │
│  5. call provider backend             │
│  6. render command/text response      │
└───────────────────────────────────────┘
```

The binary does not depend on any shell-specific integration. Optional shell integrations
can be added later, but they should remain thin wrappers around the same CLI behavior.

## 3. Routing and Plugins

By default, the configured model routes each request to a plugin. If routing is disabled,
the selected plugin is unknown, or confidence is below `router.min_confidence`, tiog falls
back to `router.default_plugin` (`command` by default).

Built-in plugins:

| Plugin | Output | Context | Conversation |
|---|---|---|---|
| `command` | command | yes | yes |
| `explainer` | text | yes | yes |
| `translator` | text | no | yes |
| `interesting` | text | no | yes |

Prompt/config plugins are not executable code. A plugin defines description, output type,
context policy, conversation policy, and a system prompt:

```yaml
plugins:
  translator:
    description: "Translate text between human languages."
    output: text
    include_context: false
    include_conversation: true
    system_prompt: |
      Translate faithfully. Preserve technical terms when appropriate. If the request refers
      to previous text with words like "it", "that", "above", or "the previous answer",
      translate the relevant prior tiog response from the recent conversation.
```

CLI overrides:

| Flag | Behavior |
|---|---|
| `--list-plugins` | Print available built-in/config plugins and exit. |
| `--plugin NAME` | Bypass model routing and run a plugin explicitly. |
| `--no-context` | Suppress terminal context for this request. |

## 4. Context Sources

The command plugin can include local terminal context. Context gathering is pluggable:

| Source | Gets output? | Requirement |
|---|---|---|
| `tmux` | yes | `$TMUX` and `tmux capture-pane` |
| `hooks` | no, commands only | `$TIOG_SESSION_LOG` |
| `pty` | placeholder | deferred recorder |
| `auto` | best available | `pty` → `tmux` → `hooks` |

Local context is always shell-agnostic:

- OS and architecture
- `$SHELL`
- current directory
- `git status -sb`
- files in the current directory

`hooks` reads `$TIOG_SESSION_LOG` lines in this format:

```text
exit_code<TAB>cwd<TAB>command
```

No shell-specific hook producer is bundled right now.

## 5. Provider Backends

Provider dispatch lives in `model::mod`:

- `anthropic`: Messages API with forced tool use.
- `openai`: Chat Completions with JSON mode.
- `openai-compatible`: same OpenAI-shaped path for DeepSeek, Ollama, local servers, and
  compatible proxies.

The provider layer supports three operations:

- `route`: choose a plugin.
- `suggest`: return a command response.
- `text_plugin`: return a text response.

## 6. Response Contracts

Command response:

```json
{
  "kind": "command",
  "plugin": "command",
  "command": "ls -lhS",
  "explanation": "Sorts by size and prints human-readable sizes.",
  "alternatives": [],
  "risk": "none",
  "needs": null
}
```

Text response:

```json
{
  "kind": "text",
  "plugin": "translator",
  "text": "本地状态文件以未脱敏形式写入。",
  "explanation": "Translated the technical phrase directly.",
  "needs": null
}
```

## 7. Safety and Privacy

- Context is redacted before model calls unless `context.redact: false`.
- Local conversation memory and self-log entries are redacted before persistence.
- Local state files are created with user-only permissions on Unix.
- Command risk is reconciled with local heuristics; destructive commands are flagged even if
  the model under-reports risk.
- The CLI never executes commands automatically.

## 8. Deferred Work

- PTY recorder for full output capture outside tmux.
- Streaming provider output.
- Follow-up/refine UX.
- Optional shell integration files.
- `tiog init` installer.
