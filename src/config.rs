//! Configuration loaded from `~/.config/tiog/config.yaml`.
//!
//! Every field has a default, so a missing or partial file is fine: a present section
//! fills its missing keys from that struct's `Default` (serde container `default`), and
//! unknown keys are ignored.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model: Model,
    pub context: ContextCfg,
    pub behavior: Behavior,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Model {
    pub provider: String,
    pub name: String,
    pub api_key_env: String,
    pub base_url: Option<String>,
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
}

impl Default for Model {
    fn default() -> Self {
        Self {
            provider: "anthropic".into(),
            name: "claude-sonnet-4-6".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            base_url: None,
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
}
