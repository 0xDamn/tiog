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
    let body = apply_request_policy(
        cfg,
        json!({
            "model": cfg.model.name,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": prompt::user_message(question, context) }
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0
        }),
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(cfg.model.timeout_secs))
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
    let body = apply_request_policy(
        cfg,
        json!({
            "model": cfg.model.name,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": prompt::router_message(question, catalog) }
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0
        }),
    );

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
    let body = apply_request_policy(
        cfg,
        json!({
            "model": cfg.model.name,
            "messages": [
                { "role": "system", "content": prompt::text_plugin_system(&plugin.system_prompt, &cfg.behavior.output_language) },
                { "role": "user", "content": prompt::text_plugin_user_message(plugin_name, question, context) }
            ],
            "response_format": { "type": "json_object" },
            "temperature": temperature
        }),
    );

    let mut response = parse_text_response(&chat_completion_content(cfg, body).await?);
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
        .timeout(Duration::from_secs(cfg.model.timeout_secs))
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

/// Apply tiog's request policy to an outgoing chat body. Reasoning models (e.g. Qwen3) spend
/// most of their token budget thinking before they emit the JSON tiog asks for, which is far
/// too slow interactively. The OpenAI-standard `reasoning_effort: "none"` turns that off and —
/// unlike the native `think` flag or a `/no_think` prompt — is honored by Ollama's
/// OpenAI-compatible `/v1` endpoint. Only sent when explicitly enabled in config.
fn apply_request_policy(cfg: &Config, mut body: Value) -> Value {
    if cfg.model.disable_thinking {
        body["reasoning_effort"] = Value::String("none".into());
    }
    body
}

fn parse_suggestion(content: &str) -> Result<Suggestion> {
    parse_json_content(content)
}

/// Parse a text-plugin reply tolerantly. Strict-schema models return our keys directly;
/// smaller local/open models (e.g. via Ollama) often ignore the contract — wrong keys, or
/// prose with no JSON at all. Rather than fail the whole request, fall back to the most
/// answer-like field, then to the raw reply.
fn parse_text_response(content: &str) -> TextResponse {
    if let Ok(response) = serde_json::from_str::<TextResponse>(content.trim()) {
        return response;
    }
    if let Some(json) = extract_json(content) {
        if let Ok(response) = serde_json::from_str::<TextResponse>(&json) {
            return response;
        }
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&json) {
            if let Some(text) = pick_text_field(&map) {
                return text_only(text);
            }
        }
    }
    text_only(content.trim().to_string())
}

/// A bare answer with no metadata; `plugin` is overwritten by the caller.
fn text_only(text: String) -> TextResponse {
    TextResponse {
        plugin: String::new(),
        text,
        explanation: String::new(),
        needs: None,
    }
}

/// Pick the field most likely to hold the user-facing answer from an off-schema object,
/// preferring well-known answer keys and falling back to the first non-empty string value.
fn pick_text_field(map: &serde_json::Map<String, Value>) -> Option<String> {
    const KEYS: [&str; 8] = [
        "text", "answer", "response", "reply", "message", "output", "result", "content",
    ];
    let non_empty = |s: &str| (!s.trim().is_empty()).then(|| s.to_string());
    for key in KEYS {
        if let Some(text) = map.get(key).and_then(Value::as_str).and_then(non_empty) {
            return Some(text);
        }
    }
    map.values().filter_map(Value::as_str).find_map(non_empty)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_response_parses_strict_schema() {
        let r =
            parse_text_response(r#"{"text": "an inode is a file index node", "explanation": "x"}"#);
        assert_eq!(r.text, "an inode is a file index node");
        assert_eq!(r.explanation, "x");
    }

    #[test]
    fn text_response_recovers_from_off_schema_keys() {
        // Smaller local models often answer under a different key instead of `text`.
        let r = parse_text_response(r#"{"answer": "use ls -lS"}"#);
        assert_eq!(r.text, "use ls -lS");
    }

    #[test]
    fn text_response_recovers_from_unknown_single_key() {
        // e.g. gemma returning {"thought": "..."} — surface it rather than crash.
        let r = parse_text_response(r#"{"thought": "a friendly greeting"}"#);
        assert_eq!(r.text, "a friendly greeting");
    }

    #[test]
    fn text_response_falls_back_to_raw_prose() {
        let r = parse_text_response("Just some prose, no JSON at all.");
        assert_eq!(r.text, "Just some prose, no JSON at all.");
    }

    #[test]
    fn text_response_skips_empty_fields_for_first_nonempty() {
        let r = parse_text_response(r#"{"thought": "", "note": "real content"}"#);
        assert_eq!(r.text, "real content");
    }
}
