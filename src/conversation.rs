//! Lightweight, session-scoped memory of recent tiog exchanges, so follow-ups like
//! "teach me more" build on the previous answer instead of starting cold.
//!
//! Scoped to the current shell session by reusing the fish integration's session-log path
//! (`.../session-<id>.log` -> `.../conversation-<id>.log`); falls back to a shared file when
//! the integration isn't installed. Best-effort: IO errors are ignored.

use std::io::Write as _;
use std::path::PathBuf;

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
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(x) if !x.is_empty() => PathBuf::from(x),
        _ => dirs::home_dir()?.join(".local/state"),
    };
    Some(base.join("tiog/conversation.log"))
}

fn read_all() -> Vec<Exchange> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<Exchange>(l).ok())
                .collect()
        })
        .unwrap_or_default()
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
    all.push(Exchange {
        q: question.to_string(),
        command: suggestion.command.clone(),
        explanation: suggestion.explanation.clone(),
    });
    let start = all.len().saturating_sub(KEEP);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::File::create(&p) {
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
