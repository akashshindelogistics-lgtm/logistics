use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::DispatchOrder;
use std::time::{SystemTime, UNIX_EPOCH};

/// Build the prompt sent to the Anthropic API for a dispatch's status
/// summary. Pulled out of [`generate_dispatch_summary`] as a pure function so
/// the prompt's content (vehicle, stock lines, customer details, the
/// "none recorded" / "location not set" fallbacks) can be unit tested without
/// a network call or an API key.
fn build_prompt(dispatch: &DispatchOrder, customer: &Customer) -> String {
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

    let stock_lines = if dispatch.line_items.is_empty() {
        "none recorded".to_string()
    } else {
        dispatch
            .line_items
            .iter()
            .map(|li| format!("{} ({} units)", li.stock_description, li.quantity))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "You are a logistics status assistant. Write a clear, friendly 2–3 sentence status \
        update for the following dispatch order. Be specific and informative — mention the \
        vehicle, the stock items and their quantities, and the customer. Do not use bullet points.\n\n\
        Order ID: {id}\n\
        Vehicle: {vehicle}\n\
        Stock items: {stock}\n\
        Status: {status}\n\
        Dispatched: {when}\n\
        Customer: {customer_name}\n\
        Delivery address: {customer_addr}\n\
        Customer GPS: {customer_loc}",
        id = &dispatch.id.to_string()[..8],
        vehicle = dispatch.vehicle_registration_number,
        stock = stock_lines,
        status = dispatch.status,
        when = dispatched_when,
        customer_name = customer.name,
        customer_addr = customer.address,
        customer_loc = customer_location,
    )
}

pub async fn generate_dispatch_summary(
    dispatch: &DispatchOrder,
    customer: &Customer,
) -> Result<String, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let prompt = build_prompt(dispatch, customer);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::dispatch::dispatch::{DispatchLineItem, DispatchStatus};
    use crate::logistics::vehicle::vehicle::Location;
    use uuid::Uuid;

    fn sample_dispatch() -> DispatchOrder {
        DispatchOrder {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            vehicle_registration_number: "MH12AB1234".to_string(),
            line_items: vec![DispatchLineItem {
                stock_description: "Cement".to_string(),
                quantity: 10,
                volume_in_size: 1,
            }],
            status: DispatchStatus::InTransit,
            dispatched_at: 1_700_000_000,
            status_history: Vec::new(),
            proof_of_delivery: None,
            trip_id: None,
            stop_sequence: None,
        }
    }

    fn sample_customer(location: Option<Location>) -> Customer {
        Customer {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "Priya Sharma".to_string(),
            address: "5 Market Road, Bengaluru".to_string(),
            location,
            phone: None,
            email: None,
        }
    }

    #[test]
    fn build_prompt_includes_order_vehicle_stock_and_customer_details() {
        let dispatch = sample_dispatch();
        let customer = sample_customer(Some(Location {
            latitude: 12.9716,
            longitude: 77.5946,
            timestamp: 0,
            address: Some("Koramangala".to_string()),
        }));

        let prompt = build_prompt(&dispatch, &customer);

        assert!(prompt.contains(&dispatch.vehicle_registration_number));
        assert!(prompt.contains("Cement (10 units)"));
        assert!(prompt.contains("IN_TRANSIT"));
        assert!(prompt.contains("Priya Sharma"));
        assert!(prompt.contains("5 Market Road, Bengaluru"));
        assert!(prompt.contains("12.9716"));
        assert!(prompt.contains("77.5946"));
        assert!(prompt.contains("Koramangala"));
        // Only the first 8 characters of the id are surfaced, not the whole UUID.
        assert!(prompt.contains(&dispatch.id.to_string()[..8]));
    }

    #[test]
    fn build_prompt_falls_back_when_customer_location_is_unset() {
        let dispatch = sample_dispatch();
        let customer = sample_customer(None);

        let prompt = build_prompt(&dispatch, &customer);

        assert!(prompt.contains("location not set"));
    }

    #[test]
    fn build_prompt_falls_back_when_there_are_no_line_items() {
        let mut dispatch = sample_dispatch();
        dispatch.line_items.clear();
        let customer = sample_customer(None);

        let prompt = build_prompt(&dispatch, &customer);

        assert!(prompt.contains("none recorded"));
    }

    #[test]
    fn build_prompt_joins_multiple_line_items() {
        let mut dispatch = sample_dispatch();
        dispatch.line_items.push(DispatchLineItem {
            stock_description: "Sand".to_string(),
            quantity: 25,
            volume_in_size: 1,
        });
        let customer = sample_customer(None);

        let prompt = build_prompt(&dispatch, &customer);

        assert!(prompt.contains("Cement (10 units), Sand (25 units)"));
    }

    #[actix_web::test]
    async fn generate_dispatch_summary_errors_when_the_api_key_is_not_set() {
        // SAFETY: this module is the only code that reads ANTHROPIC_API_KEY,
        // and no other test sets it, so this doesn't race with anything else.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let dispatch = sample_dispatch();
        let customer = sample_customer(None);

        let err = generate_dispatch_summary(&dispatch, &customer)
            .await
            .expect_err("should fail without an API key");

        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }
}
