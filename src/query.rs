//! Orchestration: gather context (terminal + recent conversation), redact, call the model,
//! reconcile risk, and remember the exchange.

use anyhow::Result;

use crate::config::Config;
use crate::output::Suggestion;

/// Build the context sent to the model: recent tiog conversation (for follow-ups) plus the
/// terminal context, with secrets scrubbed unless disabled.
pub fn gather_context(cfg: &Config) -> String {
    let mut ctx = crate::context::build(cfg).text;
    if let Some(conversation) = crate::conversation::recent() {
        ctx = format!("{conversation}\n{ctx}");
    }
    if cfg.context.redact {
        crate::redact::redact(&ctx)
    } else {
        ctx
    }
}

pub async fn run(cfg: &Config, question: &str) -> Result<Suggestion> {
    let context = gather_context(cfg);
    let mut suggestion = crate::model::suggest(cfg, question, &context).await?;
    suggestion.risk = crate::risk::reconcile(&suggestion.risk, &suggestion.command);
    crate::conversation::record(question, &suggestion);
    Ok(suggestion)
}
