//! `tmux` source — `capture-pane` gives real scrollback (commands AND their output)
//! for users working inside tmux.

use std::process::Command;

use super::{indent, last_n_lines, ContextSource};

pub(super) struct TmuxSource;

impl ContextSource for TmuxSource {
    fn collect(&self, lines: usize) -> Option<String> {
        std::env::var_os("TMUX")?;
        let start = format!("-{lines}");
        let out = Command::new("tmux")
            .args(["capture-pane", "-p", "-J", "-S", &start])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            return None;
        }
        Some(format!(
            "Recent terminal output (this tmux pane, most recent last):\n{}",
            indent(&last_n_lines(trimmed, lines))
        ))
    }
}
