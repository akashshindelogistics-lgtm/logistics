use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::DispatchOrder;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn generate_dispatch_summary(
    dispatch: &DispatchOrder,
    customer: &Customer,
) -> Result<String, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let dispatched_when = {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let secs = dispatch.dispatched_at as u64;
        let diff = now.saturating_sub(secs);
        if diff < 120 {
            "just now".to_string()
        } else if diff < 3600 {
            format!("{} minutes ago", diff / 60)
        } else if diff < 86400 {
            format!("{} hours ago", diff / 3600)
        } else {
            format!("{} days ago", diff / 86400)
        }
    };

    let customer_location = match &customer.location {
        Some(loc) => format!(
            "{:.4}°N, {:.4}°E{}",
            loc.latitude,
            loc.longitude,
            loc.address
                .as_deref()
                .map(|a| format!(" ({})", a))
                .unwrap_or_default()
        ),
        None => "location not set".to_string(),
    };

    let prompt = format!(
        "You are a logistics status assistant. Write a clear, friendly 2–3 sentence status \
        update for the following dispatch order. Be specific and informative — mention the \
        vehicle, stock item, quantity, and customer. Do not use bullet points.\n\n\
        Order ID: {id}\n\
        Vehicle: {vehicle}\n\
        Stock item: {stock} ({qty} units)\n\
        Status: {status}\n\
        Dispatched: {when}\n\
        Customer: {customer_name}\n\
        Delivery address: {customer_addr}\n\
        Customer GPS: {customer_loc}",
        id = &dispatch.id.to_string()[..8],
        vehicle = dispatch.vehicle_registration_number,
        stock = dispatch.stock_description,
        qty = dispatch.quantity,
        status = dispatch.status,
        when = dispatched_when,
        customer_name = customer.name,
        customer_addr = customer.address,
        customer_loc = customer_location,
    );

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-haiku-4-5-20251001",
            "max_tokens": 256,
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
        .map(|s| s.to_string())
        .ok_or_else(|| "Unexpected response format from Anthropic API".to_string())
}
