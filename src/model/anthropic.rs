//! Anthropic Messages API backend. Structured output via forced tool use.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use crate::config::Config;
use crate::model::prompt;
use crate::output::Suggestion;

const DEFAULT_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 1024;

pub async fn suggest(cfg: &Config, question: &str, context: &str) -> Result<Suggestion> {
    let key = cfg.api_key()?;
    let url = cfg
        .model
        .base_url
        .clone()
        .unwrap_or_else(|| DEFAULT_URL.to_string());

    let body = json!({
        "model": cfg.model.name,
        "max_tokens": MAX_TOKENS,
        "system": prompt::SYSTEM,
        "tools": [tool_schema()],
        "tool_choice": { "type": "tool", "name": "suggest_command" },
        "messages": [
            { "role": "user", "content": prompt::user_message(question, context) }
        ]
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("building HTTP client")?;

    let resp = client
        .post(&url)
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("calling Anthropic API")?;

    let status = resp.status();
    let text = resp.text().await.context("reading Anthropic response")?;
    if !status.is_success() {
        anyhow::bail!("Anthropic API error {status}: {text}");
    }

    let v: Value = serde_json::from_str(&text).context("parsing Anthropic response")?;
    let blocks = v
        .get("content")
        .and_then(Value::as_array)
        .context("Anthropic response missing `content`")?;

    for block in blocks {
        if block.get("type").and_then(Value::as_str) == Some("tool_use") {
            let input = block
                .get("input")
                .context("tool_use block missing `input`")?;
            return serde_json::from_value(input.clone())
                .context("decoding suggestion from tool input");
        }
    }

    anyhow::bail!("model returned no command (no tool_use block in response)")
}

fn tool_schema() -> Value {
    json!({
        "name": "suggest_command",
        "description": "Return the single best shell command for the user's request, with a short explanation.",
        "input_schema": {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command line to run. Empty string if none is possible."
                },
                "explanation": {
                    "type": "string",
                    "description": "One or two short lines explaining the command and the key flags."
                },
                "alternatives": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional alternative commands."
                },
                "risk": {
                    "type": "string",
                    "enum": ["none", "caution", "destructive"]
                },
                "needs": {
                    "type": "string",
                    "description": "If no command is possible, the specific information that is missing."
                }
            },
            "required": ["command", "explanation", "risk"]
        }
    })
}
