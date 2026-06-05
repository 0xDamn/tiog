//! A small record of tiog's own recent output.
//!
//! tiog feeds your terminal context to the model. Without this, tiog's own printed answers
//! sit in the scrollback (or session log) and get fed straight back — so a vague follow-up
//! like "teach me more" makes the model parrot its previous suggestion. We record the lines
//! tiog emits and strip them from future context. Best-effort: IO errors are ignored.

use std::collections::HashSet;
use std::io::Write as _;
use std::path::PathBuf;

const MAX_LINES: usize = 60;

fn path() -> Option<PathBuf> {
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(x) if !x.is_empty() => PathBuf::from(x),
        _ => dirs::home_dir()?.join(".local/state"),
    };
    Some(base.join("tiog/recent_output.log"))
}

/// Append `lines` to the recent-output record, keeping only the last `MAX_LINES`.
pub fn record(lines: &[String]) {
    let Some(p) = path() else {
        return;
    };
    let mut kept: Vec<String> = std::fs::read_to_string(&p)
        .map(|s| s.lines().map(crate::redact::redact).collect())
        .unwrap_or_default();
    kept.extend(
        lines
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(crate::redact::redact),
    );
    let start = kept.len().saturating_sub(MAX_LINES);
    if let Some(dir) = p.parent() {
        let _ = crate::statefile::create_private_dir(dir);
    }
    if let Ok(mut f) = crate::statefile::create_private(&p) {
        let _ = writeln!(f, "{}", kept[start..].join("\n"));
    }
}

/// The set of recently-emitted lines (trimmed), for stripping from captured context.
pub fn recent_set() -> HashSet<String> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}
