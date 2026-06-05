//! Structured command/text responses returned by model handlers, plus how we render them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AssistantResponse {
    #[serde(rename = "command")]
    Command(CommandResponse),
    #[serde(rename = "text")]
    Text(TextResponse),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandResponse {
    #[serde(default = "default_plugin")]
    pub plugin: String,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TextResponse {
    #[serde(default = "default_plugin")]
    pub plugin: String,
    pub text: String,
    #[serde(default)]
    pub explanation: String,
    /// Set when `text` is empty: what information is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
}

pub type Suggestion = CommandResponse;

fn default_plugin() -> String {
    "command".into()
}

fn default_risk() -> String {
    "none".into()
}

/// Human output: explanation + warnings on stderr, the command on stdout (so it can be
/// captured, e.g. `set cmd (tiog ...)`). `--json` emits the raw suggestion on stdout.
pub fn render(response: &AssistantResponse, as_json: bool) {
    if as_json {
        match serde_json::to_string(response) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("tiog: failed to serialize suggestion: {e}"),
        }
        return;
    }

    match response {
        AssistantResponse::Command(s) => render_command(s),
        AssistantResponse::Text(t) => render_text(t),
    }
}

fn render_command(s: &CommandResponse) {
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

fn render_text(t: &TextResponse) {
    if t.text.trim().is_empty() {
        eprintln!("{}", t.explanation);
        if let Some(needs) = &t.needs {
            eprintln!("need more info: {needs}");
        }
        return;
    }

    if !t.explanation.trim().is_empty() {
        eprintln!("{}", t.explanation);
    }
    println!("{}", t.text);
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

pub fn response_lines(response: &AssistantResponse) -> Vec<String> {
    match response {
        AssistantResponse::Command(s) => suggestion_lines(s),
        AssistantResponse::Text(t) => {
            let mut lines = Vec::new();
            if !t.explanation.trim().is_empty() {
                lines.extend(t.explanation.lines().map(str::to_string));
            }
            if !t.text.trim().is_empty() {
                lines.extend(t.text.lines().map(str::to_string));
            }
            lines
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestion_lines_cover_explanation_alts_command() {
        let s = Suggestion {
            plugin: "command".into(),
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
