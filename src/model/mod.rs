//! Model backends. M1 ships `anthropic`; `openai` and `openai-compatible` arrive in M5.

pub mod anthropic;
pub mod openai;
pub mod prompt;

use anyhow::Result;

use crate::config::Config;
use crate::output::Suggestion;

pub async fn suggest(cfg: &Config, question: &str, context: &str) -> Result<Suggestion> {
    match cfg.model.provider.as_str() {
        "anthropic" => anthropic::suggest(cfg, question, context).await,
        "openai" | "openai-compatible" => openai::suggest(cfg, question, context).await,
        other => anyhow::bail!(
            "unknown provider '{other}' (supported: anthropic, openai, openai-compatible)"
        ),
    }
}
