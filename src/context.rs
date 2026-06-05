//! Terminal context: local environment (always gathered) plus recent session activity
//! from a pluggable source — `tmux` | `hooks` | `pty` | `auto` (see `config.context.source`).

mod gather;
mod hooks;
mod tmux;

use std::fmt::Write as _;

use crate::config::Config;

pub struct TermContext {
    pub text: String,
}

/// A source of recent session activity (commands, and — for tmux/pty — their output).
trait ContextSource {
    /// Collect up to roughly `lines` of recent activity, or `None` if unavailable.
    fn collect(&self, lines: usize) -> Option<String>;
}

/// Assemble the context sent to the model: local environment, plus recent session
/// activity from the configured source when it is available.
pub fn build(cfg: &Config) -> TermContext {
    let mut text = gather::local_env();
    if let Some(activity) = resolve(&cfg.context.source).collect(cfg.context.history_lines) {
        let _ = write!(text, "\n{activity}");
    }
    // Drop tiog's own recent output so it doesn't see — and parrot — its previous answers
    // (e.g. on vague follow-ups like "teach me more").
    let own = crate::selflog::recent_set();
    if !own.is_empty() {
        text = text
            .lines()
            .filter(|line| !own.contains(line.trim()))
            .collect::<Vec<_>>()
            .join("\n");
    }
    TermContext { text }
}

fn resolve(source: &str) -> Box<dyn ContextSource> {
    match source {
        "tmux" => Box::new(tmux::TmuxSource),
        "hooks" => Box::new(hooks::HooksSource),
        "pty" => Box::new(PtySource),
        _ => auto(), // "auto" and any unrecognized value
    }
}

/// `auto`: a recorder session (`TIOG_SESSION`) → pty; else inside tmux (`$TMUX`) → tmux;
/// else the fish hooks log.
fn auto() -> Box<dyn ContextSource> {
    if std::env::var_os("TIOG_SESSION").is_some() {
        Box::new(PtySource)
    } else if std::env::var_os("TMUX").is_some() {
        Box::new(tmux::TmuxSource)
    } else {
        Box::new(hooks::HooksSource)
    }
}

/// PTY recorder source — **deferred (M4): placeholder for further implementation.**
///
/// Yields nothing for now, so `auto` falls back to tmux/hooks when no recorder is attached.
/// To implement: have `tiog session` record the shell stream to `$TIOG_SESSION_FILE`
/// (OSC 133-segmented; see DESIGN.md §4), then read and format it here.
struct PtySource;

impl ContextSource for PtySource {
    fn collect(&self, _lines: usize) -> Option<String> {
        None
    }
}

/// Indent each line by two spaces (shared by the gather/tmux formatters).
fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Keep only the last `n` lines of `s`.
fn last_n_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}
