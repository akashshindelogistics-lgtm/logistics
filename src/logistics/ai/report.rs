//! An AI-narrated summary of an org's [`OpsReport`] — the same "Explain this
//! report" idea as [`crate::logistics::ai::status::generate_dispatch_summary`]
//! (which narrates one dispatch), pointed at the org-wide operational report
//! instead. Not RAG — straight prompt-over-structured-data: every figure fed
//! to the prompt is already computed by [`OpsReport::for_org`], the same way
//! `generate_dispatch_summary` narrates fields the caller already fetched.
//! See `todo.org`'s "AI opportunities" section for the full design.

use crate::logistics::reports::OpsReport;

/// Build the prompt sent to the Anthropic API for a report narration. Pulled
/// out of [`generate_report_summary`] as a pure function so its content can be
/// unit tested without a network call or an API key.
fn build_report_prompt(report: &OpsReport) -> String {
    let utilization = &report.vehicle_utilization;
    let delivery = &report.delivery_performance;

    let avg_hours = delivery
        .avg_hours_to_deliver
        .map(|h| format!("{h} hours"))
        .unwrap_or_else(|| "no deliveries yet".to_string());
    let on_time = delivery
        .on_time_rate_percent
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| "n/a".to_string());

    let inventory = if report.godown_inventory.is_empty() {
        "no godowns recorded".to_string()
    } else {
        report
            .godown_inventory
            .iter()
            .map(|g| {
                let cap = g
                    .capacity_used_percent
                    .map(|c| format!("{c}% of capacity used"))
                    .unwrap_or_else(|| "no capacity limit set".to_string());
                format!(
                    "{} holds {} units across {} item(s) ({})",
                    g.godown_name, g.units_on_hand, g.distinct_items, cap
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };

    let volume_trend = report
        .dispatch_volume
        .iter()
        .map(|p| format!("{}: {}", p.date, p.count))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "You are a logistics operations analyst. Write a short, plain-English briefing \
        (3-5 sentences) for a dispatcher looking at their operations dashboard. Call out what \
        actually deserves attention today - unusually low utilization, a dip in on-time \
        delivery, godowns running low or near capacity, or a notable shift in dispatch volume. \
        Do not just restate every number; highlight what matters. Do not use bullet points.\n\n\
        Fleet utilization: {util_on}/{util_total} vehicles on an active trip ({util_pct}%).\n\
        Delivered dispatches: {delivered}. Returned: {returned}. Average time to deliver: {avg_hours}. \
        On-time rate: {on_time}.\n\
        Units dispatched in the last 30 days: {recent_units}.\n\
        Godown inventory: {inventory}.\n\
        Dispatch volume over the last 14 days (date: count): {volume_trend}.",
        util_on = utilization.vehicles_on_active_trip,
        util_total = utilization.total_vehicles,
        util_pct = utilization.utilization_percent,
        delivered = delivery.delivered_count,
        returned = delivery.returned_count,
        avg_hours = avg_hours,
        on_time = on_time,
        recent_units = report.units_dispatched_recently,
        inventory = inventory,
        volume_trend = volume_trend,
    )
}

/// Ask Claude to narrate an org's operational report — the numbers a
/// dispatcher would otherwise have to eyeball off a dashboard, turned into a
/// short "what needs your attention" briefing.
pub async fn generate_report_summary(report: &OpsReport) -> Result<String, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let prompt = build_report_prompt(report);

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
            "max_tokens": 320,
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
    use crate::logistics::reports::{
        DeliveryPerformance, DispatchVolumePoint, GodownInventory, VehicleUtilization,
    };
    use uuid::Uuid;

    fn sample_report() -> OpsReport {
        OpsReport {
            vehicle_utilization: VehicleUtilization {
                total_vehicles: 4,
                vehicles_on_active_trip: 1,
                utilization_percent: 25.0,
            },
            delivery_performance: DeliveryPerformance {
                delivered_count: 10,
                returned_count: 2,
                avg_hours_to_deliver: Some(18.5),
                on_time_rate_percent: Some(80.0),
            },
            units_dispatched_recently: 340,
            godown_inventory: vec![GodownInventory {
                godown_id: Uuid::new_v4(),
                godown_name: "Chennai Central".to_string(),
                units_on_hand: 120,
                distinct_items: 3,
                capacity_used_percent: Some(92.0),
            }],
            dispatch_volume: vec![
                DispatchVolumePoint { date: "2026-09-10".to_string(), count: 2 },
                DispatchVolumePoint { date: "2026-09-11".to_string(), count: 5 },
            ],
        }
    }

    #[test]
    fn build_report_prompt_includes_utilization_delivery_and_inventory_figures() {
        let prompt = build_report_prompt(&sample_report());
        assert!(prompt.contains("1/4 vehicles on an active trip (25%)"));
        assert!(prompt.contains("Delivered dispatches: 10"));
        assert!(prompt.contains("Returned: 2"));
        assert!(prompt.contains("18.5 hours"));
        assert!(prompt.contains("80%"));
        assert!(prompt.contains("340"));
        assert!(prompt.contains("Chennai Central holds 120 units across 3 item(s) (92% of capacity used)"));
        assert!(prompt.contains("2026-09-10: 2"));
        assert!(prompt.contains("2026-09-11: 5"));
    }

    #[test]
    fn build_report_prompt_falls_back_when_nothing_has_ever_been_delivered() {
        let mut report = sample_report();
        report.delivery_performance.avg_hours_to_deliver = None;
        report.delivery_performance.on_time_rate_percent = None;
        let prompt = build_report_prompt(&report);
        assert!(prompt.contains("no deliveries yet"));
        assert!(prompt.contains("On-time rate: n/a"));
    }

    #[test]
    fn build_report_prompt_falls_back_when_there_are_no_godowns() {
        let mut report = sample_report();
        report.godown_inventory.clear();
        let prompt = build_report_prompt(&report);
        assert!(prompt.contains("no godowns recorded"));
    }

    #[test]
    fn build_report_prompt_notes_an_uncapped_godown() {
        let mut report = sample_report();
        report.godown_inventory[0].capacity_used_percent = None;
        let prompt = build_report_prompt(&report);
        assert!(prompt.contains("no capacity limit set"));
    }

    #[actix_web::test]
    async fn generate_report_summary_errors_when_the_api_key_is_not_set() {
        // SAFETY: no other test in this crate sets or reads ANTHROPIC_API_KEY
        // concurrently with this one (status.rs's equivalent test carries the
        // same note) — see that module for the full rationale.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let err = generate_report_summary(&sample_report())
            .await
            .expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }
}
