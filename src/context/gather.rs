//! Local environment facts — gathered for every request, regardless of context source.

use std::fmt::Write as _;
use std::process::Command;

pub(super) fn local_env() -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "OS: {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    if let Ok(shell) = std::env::var("SHELL") {
        let _ = writeln!(s, "Shell: {shell}");
    }
    if let Ok(cwd) = std::env::current_dir() {
        let _ = writeln!(s, "CWD: {}", cwd.display());
    }
    if let Some(git) = git_status() {
        let _ = writeln!(s, "Git status:\n{}", super::indent(&git));
    }
    if let Some(ls) = dir_listing(40) {
        let _ = writeln!(s, "Files in CWD:\n{}", super::indent(&ls));
    }
    s
}

fn git_status() -> Option<String> {
    let out = Command::new("git").args(["status", "-sb"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let capped = cap_lines(text.trim(), 30);
    (!capped.is_empty()).then_some(capped)
}

fn dir_listing(max: usize) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(std::env::current_dir().ok()?)
        .ok()?
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    names.sort();

    let total = names.len();
    if total == 0 {
        return None;
    }
    names.truncate(max);
    let mut s = names.join("\n");
    if total > max {
        let _ = write!(s, "\n… (+{} more)", total - max);
    }
    Some(s)
}

fn cap_lines(s: &str, max: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max {
        return s.to_string();
    }
    let mut out = lines[..max].join("\n");
    let _ = write!(out, "\n… (+{} more lines)", lines.len() - max);
    out
}
