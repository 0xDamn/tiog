//! The structured suggestion returned by a model, plus how we render it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Suggestion {
    /// The command line to run. Empty when no command could be produced.
    pub command: String,
    /// One or two short lines explaining the command / the key flags.
    pub explanation: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default = "default_risk")]
    pub risk: String,
    /// Set when `command` is empty: what information is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
}

fn default_risk() -> String {
    "none".into()
}

/// Human output: explanation + warnings on stderr, the command on stdout (so it can be
/// captured, e.g. `set cmd (tiog ...)`). `--json` emits the raw suggestion on stdout.
pub fn render(s: &Suggestion, as_json: bool) {
    if as_json {
        match serde_json::to_string(s) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("tiog: failed to serialize suggestion: {e}"),
        }
        return;
    }

    if s.command.trim().is_empty() {
        eprintln!("{}", s.explanation);
        if let Some(needs) = &s.needs {
            eprintln!("need more info: {needs}");
        }
        return;
    }

    eprintln!("{}", s.explanation);
    if s.risk == "destructive" {
        eprintln!("⚠  destructive — review carefully before running");
    }
    for alt in &s.alternatives {
        eprintln!("alt: {alt}");
    }
    println!("{}", s.command);
}

/// The human-visible lines tiog emits for a suggestion. Recorded (see `crate::selflog`) so
/// they can be stripped from future captured context, preventing tiog from parroting its
/// own previous answers.
pub fn suggestion_lines(s: &Suggestion) -> Vec<String> {
    let mut lines = Vec::new();
    if !s.explanation.trim().is_empty() {
        lines.extend(s.explanation.lines().map(str::to_string));
    }
    for alt in &s.alternatives {
        lines.push(format!("alt: {alt}"));
    }
    if !s.command.trim().is_empty() {
        lines.extend(s.command.lines().map(str::to_string));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestion_lines_cover_explanation_alts_command() {
        let s = Suggestion {
            command: "ls -la".into(),
            explanation: "list everything".into(),
            alternatives: vec!["exa -la".into()],
            risk: "none".into(),
            needs: None,
        };
        let lines = suggestion_lines(&s);
        assert!(lines.contains(&"list everything".to_string()));
        assert!(lines.contains(&"alt: exa -la".to_string()));
        assert!(lines.contains(&"ls -la".to_string()));
    }
}
