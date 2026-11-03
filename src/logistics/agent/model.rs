//! One Claude Messages API request for an agent loop. There's no official Rust
//! SDK, so this is the same reqwest call shape as the rest of `ai/`.

use serde_json::{Value, json};
use std::time::Duration;

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-opus-5-5";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_TOKENS: u32 = 16_000;
/// Lets the API retry a classifier refusal on a suitable fallback model.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub url: String,
    pub api_key: String,
    pub workspace_id: Option<String>,
    pub model: String,
    pub effort: String,
}

impl ModelConfig {
    /// `None` without `ANTHROPIC_API_KEY`. `AGENT_MODEL` and `AGENT_EFFORT`
    /// override the model and effort.
    pub fn from_env() -> Option<Self> {
        let non_blank = |key: &str| std::env::var(key).ok().filter(|s| !s.trim().is_empty());
        Some(Self {
            url: MESSAGES_URL.to_string(),
            api_key: non_blank("ANTHROPIC_API_KEY")?,
            workspace_id: non_blank("ANTHROPIC_WORKSPACE_ID"),
            model: non_blank("AGENT_MODEL").unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            effort: non_blank("AGENT_EFFORT").unwrap_or_else(|| "medium".to_string()),
        })
    }
}

/// Send one turn and return the whole response body.
pub async fn send(
    config: &ModelConfig,
    system: &str,
    tools: &[Value],
    messages: &[Value],
) -> Result<Value, String> {
    let mut request = reqwest::Client::new()
        .post(&config.url)
        .timeout(REQUEST_TIMEOUT)
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", FALLBACK_BETA)
        .json(&json!({
            "model": config.model,
            "max_tokens": MAX_TOKENS,
            "system": system,
            "tools": tools,
            "messages": messages,
            "tool_choice": {"type": "auto"},
            "output_config": {"effort": config.effort},
            "fallbacks": "default",
            "cache_control": {"type": "ephemeral"},
        }));
    if let Some(workspace_id) = &config.workspace_id {
        request = request.header("anthropic-workspace-id", workspace_id);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to reach the Anthropic API: {e}"))?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Anthropic API error ({status}): {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("Unreadable Anthropic API response: {e}"))
}
