//! Model backends. M1 ships `anthropic`; `openai` and `openai-compatible` arrive in M5.

pub mod anthropic;
pub mod openai;
pub mod prompt;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{Config, Plugin};
use crate::output::{Suggestion, TextResponse};

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteDecision {
    pub plugin: String,
    pub confidence: f32,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub normalized_request: String,
}

pub async fn suggest(cfg: &Config, question: &str, context: &str) -> Result<Suggestion> {
    match cfg.model.provider.as_str() {
        "anthropic" => anthropic::suggest(cfg, question, context).await,
        "openai" | "openai-compatible" => openai::suggest(cfg, question, context).await,
        other => anyhow::bail!(
            "unknown provider '{other}' (supported: anthropic, openai, openai-compatible)"
        ),
    }
}

pub async fn route(cfg: &Config, question: &str, catalog: &str) -> Result<RouteDecision> {
    match cfg.model.provider.as_str() {
        "anthropic" => anthropic::route(cfg, question, catalog).await,
        "openai" | "openai-compatible" => openai::route(cfg, question, catalog).await,
        other => anyhow::bail!(
            "unknown provider '{other}' (supported: anthropic, openai, openai-compatible)"
        ),
    }
}

pub async fn text_plugin(
    cfg: &Config,
    plugin_name: &str,
    plugin: &Plugin,
    question: &str,
    context: &str,
) -> Result<TextResponse> {
    match cfg.model.provider.as_str() {
        "anthropic" => anthropic::text_plugin(cfg, plugin_name, plugin, question, context).await,
        "openai" | "openai-compatible" => {
            openai::text_plugin(cfg, plugin_name, plugin, question, context).await
        }
        other => anyhow::bail!(
            "unknown provider '{other}' (supported: anthropic, openai, openai-compatible)"
        ),
    }
}

pub fn text_plugin_temperature(plugin_name: &str) -> f32 {
    if plugin_name == "interesting" {
        0.9
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interesting_plugin_uses_sampling_temperature() {
        assert_eq!(text_plugin_temperature("interesting"), 0.9);
        assert_eq!(text_plugin_temperature("translator"), 0.0);
    }
}
