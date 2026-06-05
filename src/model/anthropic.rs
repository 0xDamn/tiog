//! Anthropic Messages API backend. Structured output via forced tool use.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use crate::config::{Config, Plugin};
use crate::model::prompt;
use crate::model::RouteDecision;
use crate::output::{Suggestion, TextResponse};

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

pub async fn route(cfg: &Config, question: &str, catalog: &str) -> Result<RouteDecision> {
    let body = json!({
        "model": cfg.model.name,
        "max_tokens": MAX_TOKENS,
        "system": prompt::ROUTER_SYSTEM,
        "tools": [route_tool_schema()],
        "tool_choice": { "type": "tool", "name": "route_request" },
        "messages": [
            { "role": "user", "content": prompt::router_message(question, catalog) }
        ]
    });

    call_tool(cfg, body, "route_request").await
}

pub async fn text_plugin(
    cfg: &Config,
    plugin_name: &str,
    plugin: &Plugin,
    question: &str,
    context: &str,
) -> Result<TextResponse> {
    let body = json!({
        "model": cfg.model.name,
        "max_tokens": MAX_TOKENS,
        "system": prompt::text_plugin_system(&plugin.system_prompt),
        "tools": [text_tool_schema()],
        "tool_choice": { "type": "tool", "name": "return_text" },
        "messages": [
            { "role": "user", "content": prompt::text_plugin_user_message(question, context) }
        ]
    });

    let mut response: TextResponse = call_tool(cfg, body, "return_text").await?;
    response.plugin = plugin_name.into();
    Ok(response)
}

async fn call_tool<T>(cfg: &Config, body: Value, tool_name: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let key = cfg.api_key()?;
    let url = cfg
        .model
        .base_url
        .clone()
        .unwrap_or_else(|| DEFAULT_URL.to_string());

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
        if block.get("type").and_then(Value::as_str) == Some("tool_use")
            && block.get("name").and_then(Value::as_str) == Some(tool_name)
        {
            let input = block
                .get("input")
                .context("tool_use block missing `input`")?;
            return serde_json::from_value(input.clone()).context("decoding tool input");
        }
    }

    anyhow::bail!("model returned no `{tool_name}` tool_use block in response")
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

fn route_tool_schema() -> Value {
    json!({
        "name": "route_request",
        "description": "Choose the best tiog plugin for the user's request.",
        "input_schema": {
            "type": "object",
            "properties": {
                "plugin": {
                    "type": "string",
                    "description": "The selected plugin name from the provided catalog."
                },
                "confidence": {
                    "type": "number",
                    "description": "Confidence from 0.0 to 1.0."
                },
                "reason": {
                    "type": "string",
                    "description": "Brief reason for the routing decision."
                },
                "normalized_request": {
                    "type": "string",
                    "description": "The request rewritten for the selected plugin."
                }
            },
            "required": ["plugin", "confidence", "reason", "normalized_request"]
        }
    })
}

fn text_tool_schema() -> Value {
    json!({
        "name": "return_text",
        "description": "Return a text result and a short explanation.",
        "input_schema": {
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The main text answer to print on stdout."
                },
                "explanation": {
                    "type": "string",
                    "description": "One short line explaining the result."
                },
                "needs": {
                    "type": "string",
                    "description": "If no answer is possible, the specific information missing."
                }
            },
            "required": ["text", "explanation"]
        }
    })
}
