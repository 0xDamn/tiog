//! Configuration loaded from `~/.config/tiog/config.yaml`.
//!
//! Every field has a default, so a missing or partial file is fine: a present section
//! fills its missing keys from that struct's `Default` (serde container `default`), and
//! unknown keys are ignored.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model: Model,
    pub context: ContextCfg,
    pub behavior: Behavior,
    pub router: Router,
    pub plugins: BTreeMap<String, Plugin>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Model {
    pub provider: String,
    pub name: String,
    pub api_key_env: String,
    pub base_url: Option<String>,
    /// HTTP request timeout in seconds. Local models (e.g. Ollama) with large prompts can
    /// be much slower than hosted APIs, so this is generous by default and overridable.
    pub timeout_secs: u64,
    /// Send `think: false` to the model (Ollama-compatible). Reasoning models (e.g. Qwen3)
    /// otherwise spend most of their tokens thinking before emitting the JSON tiog needs,
    /// which is far too slow for interactive use. Ignored by the anthropic provider.
    pub disable_thinking: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ContextCfg {
    pub source: String,
    pub history_lines: usize,
    pub redact: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Behavior {
    pub auto_run: String,
    pub output_language: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Router {
    pub enabled: bool,
    pub default_plugin: String,
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Plugin {
    pub description: String,
    pub output: String,
    pub include_context: bool,
    pub include_conversation: bool,
    pub system_prompt: String,
    pub builtin: bool,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            provider: "anthropic".into(),
            name: "claude-sonnet-4-6".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            base_url: None,
            timeout_secs: 120,
            disable_thinking: false,
        }
    }
}

impl Default for ContextCfg {
    fn default() -> Self {
        Self {
            source: "auto".into(),
            history_lines: 40,
            redact: true,
        }
    }
}

impl Default for Behavior {
    fn default() -> Self {
        Self {
            auto_run: "off".into(),
            output_language: "auto".into(),
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self {
            enabled: true,
            default_plugin: "command".into(),
            min_confidence: 0.55,
        }
    }
}

impl Default for Plugin {
    fn default() -> Self {
        Self {
            description: String::new(),
            output: "text".into(),
            include_context: false,
            include_conversation: false,
            system_prompt: String::new(),
            builtin: false,
        }
    }
}

impl Config {
    /// `$XDG_CONFIG_HOME/tiog/config.yaml`, else `~/.config/tiog/config.yaml`.
    /// Note: we deliberately use `~/.config` on macOS too, not the platform config dir.
    pub fn default_path() -> Option<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("tiog/config.yaml"));
            }
        }
        dirs::home_dir().map(|h| h.join(".config/tiog/config.yaml"))
    }

    pub fn load(explicit: Option<&Path>) -> Result<Config> {
        let path = explicit.map(PathBuf::from).or_else(Self::default_path);
        match path {
            Some(p) if p.exists() => {
                let text = std::fs::read_to_string(&p)
                    .with_context(|| format!("reading config {}", p.display()))?;
                serde_yaml::from_str(&text)
                    .with_context(|| format!("parsing config {}", p.display()))
            }
            _ => Ok(Config::default()),
        }
    }

    pub fn api_key(&self) -> Result<String> {
        std::env::var(&self.model.api_key_env).map_err(|_| {
            anyhow::anyhow!(
                "API key not found in ${} — set it, or change model.api_key_env in config",
                self.model.api_key_env
            )
        })
    }

    pub fn plugin(&self, name: &str) -> Option<Plugin> {
        self.plugins
            .get(name)
            .cloned()
            .or_else(|| builtin_plugin(name))
    }

    pub fn available_plugins(&self) -> BTreeMap<String, Plugin> {
        let mut plugins = builtin_plugins();
        plugins.extend(self.plugins.clone());
        plugins
    }
}

fn builtin_plugins() -> BTreeMap<String, Plugin> {
    BTreeMap::from([
        (
            "command".into(),
            Plugin {
                description: "Suggest one shell command for the user's terminal task.".into(),
                output: "command".into(),
                include_context: true,
                include_conversation: true,
                system_prompt: String::new(),
                builtin: true,
            },
        ),
        (
            "explainer".into(),
            Plugin {
                description:
                    "Explain concepts, definitions, commands, errors, and terminal output.".into(),
                output: "text".into(),
                include_context: true,
                include_conversation: true,
                system_prompt: "Explain clearly and briefly. If the request mentions an error, \
last command, output, or session state, use the provided terminal context. Prefer practical \
terminal-focused explanations. Define jargon before using it."
                    .into(),
                builtin: true,
            },
        ),
        (
            "translator".into(),
            Plugin {
                description: "Translate text between human languages.".into(),
                output: "text".into(),
                include_context: false,
                include_conversation: true,
                system_prompt: "Translate faithfully. Preserve technical terms when appropriate. \
If the request refers to previous text with words like \"it\", \"that\", \"above\", or \"the \
previous answer\", translate the relevant prior tiog response from the recent conversation."
                    .into(),
                builtin: true,
            },
        ),
        (
            "interesting".into(),
            Plugin {
                description:
                    "Share a short, interesting idea, fact, or story for idle waiting time.".into(),
                output: "text".into(),
                include_context: false,
                include_conversation: true,
                system_prompt: "The user wants something interesting while they wait. Give one \
rich but compact mini-answer: a surprising fact, concept, historical episode, mental model, or \
technical curiosity. Prefer durable knowledge over current events. If the user gives a topic, \
use it; otherwise vary domains across science, history, computing, language, art, and everyday \
life. Write enough to be satisfying: usually two to four short paragraphs, or one paragraph plus \
a few crisp bullets when structure helps. Explain why it is interesting, not just what happened. \
Avoid topics already shown in the recent tiog conversation. Avoid invented specifics, avoid \
shallow trivia lists, and do not mention terminal context, freshness markers, or internal \
selection details unless the user asks for them. Keep it readable during a short wait, but do \
not compress it to a single fact."
                    .into(),
                builtin: true,
            },
        ),
    ])
}

fn builtin_plugin(name: &str) -> Option<Plugin> {
    builtin_plugins().remove(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explainer_builtin_uses_context_and_conversation() {
        let plugin = Config::default().plugin("explainer").unwrap();

        assert_eq!(plugin.output, "text");
        assert!(plugin.include_context);
        assert!(plugin.include_conversation);
        assert!(plugin.description.contains("Explain"));
    }

    #[test]
    fn translator_builtin_uses_conversation_without_terminal_context() {
        let plugin = Config::default().plugin("translator").unwrap();

        assert_eq!(plugin.output, "text");
        assert!(!plugin.include_context);
        assert!(plugin.include_conversation);
        assert!(plugin.system_prompt.contains("previous"));
    }

    #[test]
    fn interesting_builtin_is_context_free_but_remembers_conversation() {
        let plugin = Config::default().plugin("interesting").unwrap();

        assert_eq!(plugin.output, "text");
        assert!(!plugin.include_context);
        assert!(plugin.include_conversation);
        assert!(plugin.description.contains("interesting"));
        assert!(plugin.system_prompt.contains("durable knowledge"));
    }

    #[test]
    fn output_language_defaults_to_auto() {
        let cfg = Config::default();

        assert_eq!(cfg.behavior.output_language, "auto");
    }

    #[test]
    fn output_language_loads_from_yaml() {
        let cfg: Config = serde_yaml::from_str(
            r#"
behavior:
  output_language: Chinese
"#,
        )
        .unwrap();

        assert_eq!(cfg.behavior.output_language, "Chinese");
        assert_eq!(cfg.behavior.auto_run, "off");
    }
}
