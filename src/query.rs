//! Orchestration: gather context (terminal + recent conversation), redact, call the model,
//! reconcile risk, and remember the exchange.

use anyhow::Result;
use std::fmt::Write as _;

use crate::config::{Config, Plugin};
use crate::model::RouteDecision;
use crate::output::{AssistantResponse, Suggestion};

#[derive(Debug, Default)]
pub struct RunOptions {
    pub include_context: bool,
    pub plugin: Option<String>,
}

pub fn gather_context_with(
    cfg: &Config,
    include_context: bool,
    include_conversation: bool,
) -> String {
    gather_context_for(cfg, include_context, include_conversation)
}

fn gather_context_for(cfg: &Config, include_context: bool, include_conversation: bool) -> String {
    let mut ctx = if include_context {
        crate::context::build(cfg).text
    } else {
        String::new()
    };
    if include_conversation {
        if let Some(conversation) = crate::conversation::recent() {
            if ctx.is_empty() {
                ctx = conversation;
            } else {
                ctx = format!("{conversation}\n{ctx}");
            }
        }
    }
    if cfg.context.redact {
        crate::redact::redact(&ctx)
    } else {
        ctx
    }
}

pub async fn run_with_options(
    cfg: &Config,
    question: &str,
    options: RunOptions,
) -> Result<AssistantResponse> {
    let selected = select_plugin(cfg, question, options.plugin.as_deref()).await?;
    let plugin = cfg.plugin(&selected.name).unwrap_or_else(command_plugin);
    if plugin.output == "command" || selected.name == "command" {
        let mut suggestion =
            run_command(cfg, &selected.request, &plugin, options.include_context).await?;
        suggestion.plugin = selected.name;
        let response = AssistantResponse::Command(suggestion);
        if plugin.include_conversation {
            crate::conversation::record_response(question, &response);
        }
        return Ok(response);
    }

    let context = gather_context_for(
        cfg,
        plugin.include_context && options.include_context,
        plugin.include_conversation,
    );
    let response =
        crate::model::text_plugin(cfg, &selected.name, &plugin, &selected.request, &context)
            .await?;
    let response = AssistantResponse::Text(response);
    if plugin.include_conversation {
        crate::conversation::record_response(question, &response);
    }
    Ok(response)
}

async fn run_command(
    cfg: &Config,
    question: &str,
    plugin: &Plugin,
    include_context: bool,
) -> Result<Suggestion> {
    let context = gather_context_for(
        cfg,
        plugin.include_context && include_context,
        plugin.include_conversation,
    );
    let mut suggestion = crate::model::suggest(cfg, question, &context).await?;
    suggestion.risk = crate::risk::reconcile(&suggestion.risk, &suggestion.command);
    Ok(suggestion)
}

#[derive(Debug)]
struct SelectedPlugin {
    name: String,
    request: String,
}

async fn select_plugin(
    cfg: &Config,
    question: &str,
    explicit_plugin: Option<&str>,
) -> Result<SelectedPlugin> {
    let default = default_plugin_name(cfg);
    if let Some(plugin) = explicit_plugin {
        if cfg.plugin(plugin).is_none() {
            anyhow::bail!("unknown plugin '{plugin}'");
        }
        return Ok(SelectedPlugin {
            name: plugin.to_string(),
            request: question.into(),
        });
    }

    if !cfg.router.enabled {
        return Ok(SelectedPlugin {
            name: default,
            request: question.into(),
        });
    }

    let catalog = plugin_catalog(cfg);
    let decision = crate::model::route(cfg, question, &catalog)
        .await
        .unwrap_or_else(|_| RouteDecision {
            plugin: default.clone(),
            confidence: 0.0,
            reason: String::new(),
            normalized_request: String::new(),
        });

    Ok(resolve_decision(cfg, decision, &default, question))
}

fn resolve_decision(
    cfg: &Config,
    decision: RouteDecision,
    default: &str,
    original_request: &str,
) -> SelectedPlugin {
    if decision.confidence < cfg.router.min_confidence || cfg.plugin(&decision.plugin).is_none() {
        return SelectedPlugin {
            name: default.to_string(),
            request: original_request.to_string(),
        };
    }
    let uses_conversation = cfg
        .plugin(&decision.plugin)
        .is_some_and(|plugin| plugin.include_conversation);
    let normalized_request = decision.normalized_request.trim();
    let request = if !uses_conversation && !normalized_request.is_empty() {
        decision.normalized_request
    } else {
        original_request.to_string()
    };
    SelectedPlugin {
        name: decision.plugin,
        request,
    }
}

fn default_plugin_name(cfg: &Config) -> String {
    if cfg.plugin(&cfg.router.default_plugin).is_some() {
        cfg.router.default_plugin.clone()
    } else {
        "command".into()
    }
}

fn command_plugin() -> Plugin {
    cfg_plugin("command")
}

fn cfg_plugin(name: &str) -> Plugin {
    Config::default()
        .plugin(name)
        .expect("built-in plugin must exist")
}

fn plugin_catalog(cfg: &Config) -> String {
    let mut out = String::new();
    for (name, plugin) in cfg.available_plugins() {
        let _ = writeln!(
            out,
            "- {name}: {} (output: {})",
            plugin.description, plugin.output
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_confidence_routes_to_default_command() {
        let cfg = Config::default();
        let decision = RouteDecision {
            plugin: "translator".into(),
            confidence: 0.1,
            reason: String::new(),
            normalized_request: String::new(),
        };

        let selected = resolve_decision(&cfg, decision, "command", "list files");

        assert_eq!(selected.name, "command");
        assert_eq!(selected.request, "list files");
    }

    #[test]
    fn conversation_plugin_keeps_original_request() {
        let cfg = Config::default();
        let decision = RouteDecision {
            plugin: "translator".into(),
            confidence: 0.9,
            reason: "translation follow-up".into(),
            normalized_request: "Translate the request into Chinese".into(),
        };

        let selected = resolve_decision(&cfg, decision, "command", "pls translate it into Chinese");

        assert_eq!(selected.name, "translator");
        assert_eq!(selected.request, "pls translate it into Chinese");
    }

    #[test]
    fn context_free_plugin_uses_normalized_request() {
        let mut cfg = Config::default();
        cfg.plugins.insert(
            "formatter".into(),
            Plugin {
                description: "Format context-free text.".into(),
                output: "text".into(),
                include_context: false,
                include_conversation: false,
                system_prompt: "Format text.".into(),
                builtin: false,
            },
        );
        let decision = RouteDecision {
            plugin: "formatter".into(),
            confidence: 0.9,
            reason: String::new(),
            normalized_request: "Format hello as title case".into(),
        };

        let selected = resolve_decision(&cfg, decision, "command", "hello title case");

        assert_eq!(selected.name, "formatter");
        assert_eq!(selected.request, "Format hello as title case");
    }

    #[test]
    fn unknown_plugin_routes_to_default_command() {
        let cfg = Config::default();
        let decision = RouteDecision {
            plugin: "missing".into(),
            confidence: 1.0,
            reason: String::new(),
            normalized_request: String::new(),
        };

        let selected = resolve_decision(&cfg, decision, "command", "list files");

        assert_eq!(selected.name, "command");
        assert_eq!(selected.request, "list files");
    }

    #[tokio::test]
    async fn explicit_plugin_bypasses_router() {
        let cfg = Config::default();
        let selected = select_plugin(&cfg, "hello into Chinese", Some("translator"))
            .await
            .unwrap();

        assert_eq!(selected.name, "translator");
        assert_eq!(selected.request, "hello into Chinese");
    }

    #[tokio::test]
    async fn explicit_unknown_plugin_errors() {
        let cfg = Config::default();
        let err = select_plugin(&cfg, "hello", Some("missing"))
            .await
            .unwrap_err();

        assert!(err.to_string().contains("unknown plugin 'missing'"));
    }
}
