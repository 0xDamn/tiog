//! Lightweight, session-scoped memory of recent tiog exchanges, so follow-ups like
//! "teach me more" build on the previous answer instead of starting cold.
//!
//! Scoped to the current shell session by reusing the fish integration's session-log path
//! (`.../session-<id>.log` -> `.../conversation-<id>.log`); otherwise keyed by the current
//! terminal session or tty. Best-effort: IO errors are ignored.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::output::Suggestion;

const KEEP: usize = 12; // exchanges retained on disk
const SHOW: usize = 3; // most-recent exchanges shown to the model

#[derive(Serialize, Deserialize)]
struct Exchange {
    q: String,
    command: String,
    explanation: String,
}

fn path() -> Option<PathBuf> {
    if let Ok(log) = std::env::var("TIOG_SESSION_LOG") {
        if !log.is_empty() {
            let p = PathBuf::from(&log);
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                return Some(p.with_file_name(name.replacen("session-", "conversation-", 1)));
            }
        }
    }
    let base = state_dir()?;
    let key = session_key()?;
    Some(base.join(format!("tiog/conversation-{key}.log")))
}

fn state_dir() -> Option<PathBuf> {
    Some(match std::env::var("XDG_STATE_HOME") {
        Ok(x) if !x.is_empty() => PathBuf::from(x),
        _ => dirs::home_dir()?.join(".local/state"),
    })
}

fn session_key() -> Option<String> {
    for name in [
        "TIOG_SESSION",
        "TMUX_PANE",
        "TERM_SESSION_ID",
        "ITERM_SESSION_ID",
        "WT_SESSION",
        "KONSOLE_DBUS_SESSION",
    ] {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                return sanitize_key(&format!("{name}-{value}"));
            }
        }
    }

    tty_name().and_then(|tty| sanitize_key(&format!("tty-{tty}")))
}

fn tty_name() -> Option<String> {
    let out = Command::new("tty").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let tty = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!tty.is_empty() && tty != "not a tty").then_some(tty)
}

fn sanitize_key(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else if matches!(ch, '.' | '_' | '-') {
            ch
        } else {
            '-'
        };
        if mapped == '-' {
            if out.is_empty() || last_sep {
                continue;
            }
            last_sep = true;
        } else {
            last_sep = false;
        }
        out.push(mapped);
        if out.len() >= 96 {
            break;
        }
    }
    let trimmed = out
        .trim_matches(|ch| matches!(ch, '-' | '.' | '_'))
        .to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn read_all() -> Vec<Exchange> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<Exchange>(l).ok())
                .map(redact_exchange)
                .collect()
        })
        .unwrap_or_default()
}

fn redact_exchange(ex: Exchange) -> Exchange {
    Exchange {
        q: crate::redact::redact(&ex.q),
        command: crate::redact::redact(&ex.command),
        explanation: crate::redact::redact(&ex.explanation),
    }
}

/// Append an exchange. No-op when no command was produced (a clarification isn't worth
/// remembering as a turn).
pub fn record(question: &str, suggestion: &Suggestion) {
    if suggestion.command.trim().is_empty() {
        return;
    }
    let Some(p) = path() else {
        return;
    };
    let mut all = read_all();
    all.push(redact_exchange(Exchange {
        q: question.to_string(),
        command: suggestion.command.clone(),
        explanation: suggestion.explanation.clone(),
    }));
    let start = all.len().saturating_sub(KEEP);
    if let Some(dir) = p.parent() {
        let _ = crate::statefile::create_private_dir(dir);
    }
    if let Ok(mut f) = crate::statefile::create_private(&p) {
        for ex in &all[start..] {
            if let Ok(line) = serde_json::to_string(ex) {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

/// A labeled block of the most-recent exchanges for the model, or `None` if there are none.
pub fn recent() -> Option<String> {
    let all = read_all();
    if all.is_empty() {
        return None;
    }
    let start = all.len().saturating_sub(SHOW);
    let mut out = String::from("Recent tiog conversation (this session, oldest first):\n");
    for (i, ex) in all[start..].iter().enumerate() {
        out.push_str(&format!(
            "{}. you asked: \"{}\"\n   tiog suggested: {} — {}\n",
            i + 1,
            ex.q,
            ex.command,
            ex.explanation
        ));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_key_makes_safe_filename_piece() {
        assert_eq!(
            sanitize_key("tty-/dev/ttys001").as_deref(),
            Some("tty-dev-ttys001")
        );
        assert_eq!(
            sanitize_key("TMUX_PANE-%12").as_deref(),
            Some("tmux_pane-12")
        );
    }

    #[test]
    fn sanitize_key_rejects_empty_values() {
        assert_eq!(sanitize_key("////"), None);
        assert_eq!(sanitize_key(""), None);
    }

    #[test]
    fn persisted_exchange_is_redacted() {
        let ex = redact_exchange(Exchange {
            q: "use API_KEY=sk-abcdef0123456789xyz".into(),
            command: "curl -H 'Bearer abcdef0123456789' https://example.com".into(),
            explanation: "uses token=ghp_0123456789abcdefghij0123".into(),
        });

        assert!(!ex.q.contains("sk-"));
        assert!(!ex.command.contains("Bearer abc"));
        assert!(!ex.explanation.contains("ghp_"));
    }
}
