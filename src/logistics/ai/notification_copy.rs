//! AI-personalized notification copy: replaces the fixed per-event templates
//! in [`crate::logistics::notification::notification`] with a Claude-
//! generated message when `ANTHROPIC_API_KEY` is configured. Not RAG —
//! straight prompt-over-structured-data, same shape as the other AI features
//! in this codebase (`ai::status`, `ai::report`, `ai::digest`, `ai::reorder`).
//!
//! Every function here only *attempts* generation — on any error (no API
//! key, a network problem, an unexpected response shape) it returns `Err`
//! and the caller falls back to the exact same fixed template this feature
//! used before AI copy existed. A misconfigured or rate-limited environment
//! must never block sending a notification outright.

/// Ask Claude to write one notification message. Shared by the three
/// `generate_*` functions below — they differ only in the prompt they build.
async fn call_claude(prompt: String) -> Result<String, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let client = reqwest::Client::new();
    let mut request = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01");

    // Identity-linked API keys (issued against a user rather than a single
    // workspace) must name the target workspace explicitly, or the API rejects
    // the call with `workspace_id_required`. Workspace-scoped keys don't need
    // this, so the header is only sent when `ANTHROPIC_WORKSPACE_ID` is set.
    if let Ok(workspace_id) = std::env::var("ANTHROPIC_WORKSPACE_ID") {
        if !workspace_id.trim().is_empty() {
            request = request.header("anthropic-workspace-id", workspace_id);
        }
    }

    let resp = request
        .json(&serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 128,
            "messages": [{ "role": "user", "content": prompt }]
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to reach Anthropic API: {}", e))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error: {}", err));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse API response: {}", e))?;

    json["content"][0]["text"]
        .as_str()
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "Unexpected response format from Anthropic API".to_string())
}

/// Shared instruction suffix: keep the model from wrapping the message in
/// extras the fixed templates never had (a subject line, a greeting
/// placeholder, quote marks around the whole thing).
const STYLE_NOTE: &str = "Write ONLY the message text itself, as 1-2 short, friendly sentences. \
    No subject line, no signature, no quotation marks around it, and use the name given directly \
    rather than a placeholder like [Name].";

fn build_dispatch_created_customer_prompt(customer_name: &str, dispatch_short_id: &str) -> String {
    format!(
        "You are a logistics company texting a customer to let them know their order has just been \
        dispatched and is on its way. Customer name: {customer_name}. Order reference: {dispatch_short_id}. \
        {STYLE_NOTE}"
    )
}

fn build_dispatch_created_driver_prompt(customer_name: &str, dispatch_short_id: &str) -> String {
    format!(
        "You are a logistics company texting a driver to let them know they've been assigned a new \
        delivery trip. Customer they're delivering to: {customer_name}. Order reference: {dispatch_short_id}. \
        {STYLE_NOTE}"
    )
}

fn build_dispatch_delivered_customer_prompt(customer_name: &str, dispatch_short_id: &str) -> String {
    format!(
        "You are a logistics company texting a customer to confirm their order has just been \
        delivered, and thanking them. Customer name: {customer_name}. Order reference: {dispatch_short_id}. \
        {STYLE_NOTE}"
    )
}

fn build_dispatch_running_late_customer_prompt(customer_name: &str, dispatch_short_id: &str, hours_late: i64) -> String {
    format!(
        "You are a logistics company texting a customer to apologetically let them know their order \
        is running behind schedule and is still on its way. Customer name: {customer_name}. Order \
        reference: {dispatch_short_id}. It is running about {hours_late} hours behind its expected \
        delivery time. {STYLE_NOTE}"
    )
}

/// "Your order is on its way" — for the customer, when a dispatch is created.
pub async fn generate_dispatch_created_customer_body(customer_name: &str, dispatch_short_id: &str) -> Result<String, String> {
    call_claude(build_dispatch_created_customer_prompt(customer_name, dispatch_short_id)).await
}

/// "New trip assigned" — for the driver, when a dispatch is created.
pub async fn generate_dispatch_created_driver_body(customer_name: &str, dispatch_short_id: &str) -> Result<String, String> {
    call_claude(build_dispatch_created_driver_prompt(customer_name, dispatch_short_id)).await
}

/// "Your order has been delivered" — for the customer, when a dispatch is delivered.
pub async fn generate_dispatch_delivered_customer_body(customer_name: &str, dispatch_short_id: &str) -> Result<String, String> {
    call_claude(build_dispatch_delivered_customer_prompt(customer_name, dispatch_short_id)).await
}

/// "Your order is running behind schedule" — for the customer, when a
/// dispatch's delay alert fires.
pub async fn generate_dispatch_running_late_customer_body(customer_name: &str, dispatch_short_id: &str, hours_late: i64) -> Result<String, String> {
    call_claude(build_dispatch_running_late_customer_prompt(customer_name, dispatch_short_id, hours_late)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dispatch_created_customer_prompt_includes_name_id_and_style_note() {
        let prompt = build_dispatch_created_customer_prompt("Priya Sharma", "a1b2c3d4");
        assert!(prompt.contains("Priya Sharma"));
        assert!(prompt.contains("a1b2c3d4"));
        assert!(prompt.contains("dispatched"));
        assert!(prompt.contains("no signature"));
    }

    #[test]
    fn build_dispatch_created_driver_prompt_mentions_the_customer_being_delivered_to() {
        let prompt = build_dispatch_created_driver_prompt("Priya Sharma", "a1b2c3d4");
        assert!(prompt.contains("Priya Sharma"));
        assert!(prompt.contains("a1b2c3d4"));
        assert!(prompt.contains("driver"));
        assert!(prompt.contains("new delivery trip") || prompt.contains("assigned"));
    }

    #[test]
    fn build_dispatch_delivered_customer_prompt_mentions_delivery_and_thanks() {
        let prompt = build_dispatch_delivered_customer_prompt("Priya Sharma", "a1b2c3d4");
        assert!(prompt.contains("Priya Sharma"));
        assert!(prompt.contains("delivered"));
        assert!(prompt.contains("thanking"));
    }

    #[test]
    fn build_dispatch_running_late_customer_prompt_mentions_the_delay_and_hours() {
        let prompt = build_dispatch_running_late_customer_prompt("Priya Sharma", "a1b2c3d4", 14);
        assert!(prompt.contains("Priya Sharma"));
        assert!(prompt.contains("a1b2c3d4"));
        assert!(prompt.contains("behind schedule"));
        assert!(prompt.contains("14 hours"));
    }

    // The three tests below only exercise the missing-API-key error path,
    // matching this codebase's convention of never hitting a live
    // third-party API in tests (see ai/status.rs's equivalent note).

    #[actix_web::test]
    async fn generate_dispatch_created_customer_body_errors_when_the_api_key_is_not_set() {
        // SAFETY: no other test in this crate sets or reads ANTHROPIC_API_KEY
        // concurrently with this one — see ai/status.rs's equivalent test for
        // the full rationale.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let err = generate_dispatch_created_customer_body("Priya Sharma", "a1b2c3d4")
            .await
            .expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }

    #[actix_web::test]
    async fn generate_dispatch_created_driver_body_errors_when_the_api_key_is_not_set() {
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let err = generate_dispatch_created_driver_body("Priya Sharma", "a1b2c3d4")
            .await
            .expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }

    #[actix_web::test]
    async fn generate_dispatch_delivered_customer_body_errors_when_the_api_key_is_not_set() {
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let err = generate_dispatch_delivered_customer_body("Priya Sharma", "a1b2c3d4")
            .await
            .expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }

    #[actix_web::test]
    async fn generate_dispatch_running_late_customer_body_errors_when_the_api_key_is_not_set() {
        // SAFETY: see above.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let err = generate_dispatch_running_late_customer_body("Priya Sharma", "a1b2c3d4", 14)
            .await
            .expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }
}
