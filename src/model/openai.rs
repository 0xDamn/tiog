//! OpenAI and OpenAI-compatible (Ollama, local servers, proxies) Chat Completions backend.
//!
//! Structured output is requested via JSON mode and parsed leniently, so it also works on
//! compatible servers that don't support strict json_schema or tool calling.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use crate::config::Config;
use crate::model::prompt;
use crate::output::Suggestion;

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

    let system = format!("{}\n\n{}", prompt::SYSTEM, prompt::JSON_FORMAT);
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

fn parse_suggestion(content: &str) -> Result<Suggestion> {
    if let Ok(s) = serde_json::from_str::<Suggestion>(content.trim()) {
        return Ok(s);
    }
    let json = extract_json(content).context("model did not return a JSON object")?;
    serde_json::from_str(&json).context("decoding suggestion JSON")
}

/// Pull the outermost `{ ... }` out of a possibly-chatty response (code fences, prose).
fn extract_json(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end > start).then(|| s[start..=end].to_string())
}
