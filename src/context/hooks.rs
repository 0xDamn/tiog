//! `hooks` source — reads the per-session command log the fish integration writes to
//! `$TIOG_SESSION_LOG`: one `exitcode<TAB>cwd<TAB>command` line per command. No output,
//! but it always works and needs no tmux/recorder.

use std::fmt::Write as _;

use super::ContextSource;

pub(super) struct HooksSource;

impl ContextSource for HooksSource {
    fn collect(&self, lines: usize) -> Option<String> {
        let path = std::env::var_os("TIOG_SESSION_LOG")?;
        let text = std::fs::read_to_string(path).ok()?;

        let cwd_now = std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string());

        let entries: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        let start = entries.len().saturating_sub(lines);

        let mut out = String::from("Recent commands (this session, most recent last):\n");
        let mut shown = 0usize;
        for line in &entries[start..] {
            let mut parts = line.splitn(3, '\t');
            let code = parts.next().unwrap_or("");
            let cwd = parts.next().unwrap_or("");
            let cmd = parts.next().unwrap_or("").trim();
            if cmd.is_empty() || is_tiog(cmd) {
                continue;
            }
            let exit = if code == "0" {
                String::new()
            } else {
                format!("   [exit {code}]")
            };
            let location = match &cwd_now {
                Some(c) if !cwd.is_empty() && c != cwd => format!("   (in {cwd})"),
                _ => String::new(),
            };
            let _ = writeln!(out, "  {cmd}{exit}{location}");
            shown += 1;
        }
        (shown > 0).then_some(out)
    }
}

fn is_tiog(cmd: &str) -> bool {
    cmd == "tiog" || cmd.starts_with("tiog ")
}
