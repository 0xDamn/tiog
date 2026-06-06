//! OpenAI and OpenAI-compatible (Ollama, local servers, proxies) Chat Completions backend.
//!
//! Structured output is requested via JSON mode and parsed leniently, so it also works on
//! compatible servers that don't support strict json_schema or tool calling.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use crate::config::{Config, Plugin};
use crate::model::prompt;
use crate::model::RouteDecision;
use crate::output::{Suggestion, TextResponse};

const OPENAI_DEFAULT_URL: &str = "https://api.openai.com/v1";

pub async fn suggest(cfg: &Config, question: &str, context: &str) -> Result<Suggestion> {
    let compatible = cfg.model.provider == "openai-compatible";

    let base = match (&cfg.model.base_url, compatible) {
        (Some(b), _) => b.trim_end_matches('/').to_string(),
        (None, false) => OPENAI_DEFAULT_URL.to_string(),
        (None, true) => anyhow::bail!("openai-compatible requires model.base_url in config"),
    };
    let url = format!("{base}/chat/completions");

    // API key: required for openai, optional for openai-compatible (local servers).
    let key = std::env::var(&cfg.model.api_key_env).ok();
    if key.is_none() && !compatible {
        anyhow::bail!(
            "API key not found in ${} — set it, or change model.api_key_env in config",
            cfg.model.api_key_env
        );
    }

    let system = format!(
        "{}\n\n{}",
        prompt::command_system(&cfg.behavior.output_language),
        prompt::JSON_FORMAT
    );
    let body = json!({
        "model": cfg.model.name,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt::user_message(question, context) }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("building HTTP client")?;

    let mut req = client.post(&url).json(&body);
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let resp = req.send().await.context("calling OpenAI API")?;

    let status = resp.status();
    let text = resp.text().await.context("reading OpenAI response")?;
    if !status.is_success() {
        anyhow::bail!("OpenAI API error {status}: {text}");
    }

    let v: Value = serde_json::from_str(&text).context("parsing OpenAI response")?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .context("OpenAI response missing choices[0].message.content")?;

    parse_suggestion(content)
}

pub async fn route(cfg: &Config, question: &str, catalog: &str) -> Result<RouteDecision> {
    let system = format!(
        "{}\n\n{}",
        prompt::ROUTER_SYSTEM,
        prompt::ROUTER_JSON_FORMAT
    );
    let body = json!({
        "model": cfg.model.name,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": prompt::router_message(question, catalog) }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0
    });

    let content = chat_completion_content(cfg, body).await?;
    parse_json_content::<RouteDecision>(&content)
}

pub async fn text_plugin(
    cfg: &Config,
    plugin_name: &str,
    plugin: &Plugin,
    question: &str,
    context: &str,
) -> Result<TextResponse> {
    let temperature = crate::model::text_plugin_temperature(plugin_name);
    let body = json!({
        "model": cfg.model.name,
        "messages": [
            { "role": "system", "content": prompt::text_plugin_system(&plugin.system_prompt, &cfg.behavior.output_language) },
            { "role": "user", "content": prompt::text_plugin_user_message(plugin_name, question, context) }
        ],
        "response_format": { "type": "json_object" },
        "temperature": temperature
    });

    let mut response =
        parse_json_content::<TextResponse>(&chat_completion_content(cfg, body).await?)?;
    response.plugin = plugin_name.into();
    Ok(response)
}

async fn chat_completion_content(cfg: &Config, body: Value) -> Result<String> {
    let compatible = cfg.model.provider == "openai-compatible";

    let base = match (&cfg.model.base_url, compatible) {
        (Some(b), _) => b.trim_end_matches('/').to_string(),
        (None, false) => OPENAI_DEFAULT_URL.to_string(),
        (None, true) => anyhow::bail!("openai-compatible requires model.base_url in config"),
    };
    let url = format!("{base}/chat/completions");

    // API key: required for openai, optional for openai-compatible (local servers).
    let key = std::env::var(&cfg.model.api_key_env).ok();
    if key.is_none() && !compatible {
        anyhow::bail!(
            "API key not found in ${} — set it, or change model.api_key_env in config",
            cfg.model.api_key_env
        );
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("building HTTP client")?;

    let mut req = client.post(&url).json(&body);
    if let Some(k) = key {
        req = req.bearer_auth(k);
    }
    let resp = req.send().await.context("calling OpenAI API")?;

    let status = resp.status();
    let text = resp.text().await.context("reading OpenAI response")?;
    if !status.is_success() {
        anyhow::bail!("OpenAI API error {status}: {text}");
    }

    let v: Value = serde_json::from_str(&text).context("parsing OpenAI response")?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .context("OpenAI response missing choices[0].message.content")?;
    Ok(content.to_string())
}

fn parse_suggestion(content: &str) -> Result<Suggestion> {
    parse_json_content(content)
}

fn parse_json_content<T>(content: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    if let Ok(s) = serde_json::from_str::<T>(content.trim()) {
        return Ok(s);
    }
    let json = extract_json(content).context("model did not return a JSON object")?;
    serde_json::from_str(&json).context("decoding model JSON")
}

/// Pull the outermost `{ ... }` out of a possibly-chatty response (code fences, prose).
fn extract_json(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end > start).then(|| s[start..=end].to_string())
}
