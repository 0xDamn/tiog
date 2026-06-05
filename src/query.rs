//! Orchestration: gather context, redact secrets, call the model, reconcile risk.

use anyhow::Result;

use crate::config::Config;
use crate::output::Suggestion;

/// Build the terminal context and, unless disabled, scrub secrets from it.
pub fn gather_context(cfg: &Config) -> String {
    let ctx = crate::context::build(cfg);
    if cfg.context.redact {
        crate::redact::redact(&ctx.text)
    } else {
        ctx.text
    }
}

pub async fn run(cfg: &Config, question: &str) -> Result<Suggestion> {
    let context = gather_context(cfg);
    let mut suggestion = crate::model::suggest(cfg, question, &context).await?;
    suggestion.risk = crate::risk::reconcile(&suggestion.risk, &suggestion.command);
    Ok(suggestion)
}
