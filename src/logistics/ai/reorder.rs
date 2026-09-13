//! Smart reorder suggestions: narrate low-stock godowns with a suggested
//! reorder quantity based on recent dispatch velocity, not just the current
//! `below_threshold` flag. Also prompt-over-structured-data, gated on one
//! new piece of structured computation this system didn't have before -
//! per-item dispatch velocity (units of a given stock description
//! dispatched per week, derived from existing `DispatchLineItem`s +
//! `dispatched_at`, no new table). Folded into the assistant widget as a
//! canned starter question, the same way
//! [`crate::logistics::ai::digest::generate_daily_digest`] is. See
//! `todo.org`'s "AI opportunities" section for the full design.

use crate::logistics::dispatch::dispatch::DispatchOrder;
use crate::logistics::godown::godown::Godown;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// How far back to look when estimating recent demand.
const VELOCITY_LOOKBACK_DAYS: i64 = 30;
/// How many weeks of cover a suggested reorder quantity aims to restock to.
const TARGET_WEEKS_OF_COVER: f64 = 2.0;

const SECONDS_PER_DAY: i64 = 86_400;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A godown stock item currently below its reorder threshold, with a
/// suggested quantity to bring back in based on recent demand.
#[derive(Debug, Clone)]
pub struct ReorderSuggestion {
    pub godown_name: String,
    pub description: String,
    pub quantity_on_hand: i64,
    /// Units of this description dispatched per week, averaged over the last
    /// [`VELOCITY_LOOKBACK_DAYS`] days across the whole org (dispatches
    /// aren't attributed to a source godown, so this is an org-wide rate —
    /// the same simplification `OpsReport::units_dispatched_recently` makes).
    pub weekly_velocity: f64,
    /// `0` when there's no recent dispatch activity to estimate demand from.
    pub suggested_reorder_quantity: i64,
}

/// Units of `description` dispatched per week, averaged over the last
/// `lookback_days` days as of `now`. Pure — takes an already-fetched
/// dispatch list rather than querying, so it's unit-testable without a
/// database.
fn weekly_velocity(dispatches: &[DispatchOrder], description: &str, now: i64, lookback_days: i64) -> f64 {
    let since = now - lookback_days * SECONDS_PER_DAY;
    let total: i64 = dispatches
        .iter()
        .filter(|d| d.dispatched_at >= since)
        .flat_map(|d| d.line_items.iter())
        .filter(|li| li.stock_description == description)
        .map(|li| li.quantity)
        .sum();

    total as f64 / (lookback_days as f64 / 7.0)
}

/// How much of `description` to reorder to cover [`TARGET_WEEKS_OF_COVER`]
/// weeks of demand at `velocity` units/week, given `quantity_on_hand`
/// already in stock. `0` when there's no recent velocity to estimate from.
fn suggested_quantity(velocity: f64, quantity_on_hand: i64) -> i64 {
    if velocity <= 0.0 {
        return 0;
    }
    let target_stock = (velocity * TARGET_WEEKS_OF_COVER).ceil() as i64;
    (target_stock - quantity_on_hand).max(0)
}

/// Every godown stock item currently below its reorder threshold, each with
/// a suggested reorder quantity based on recent dispatch velocity.
pub fn compute_reorder_suggestions(org_id: Uuid) -> Result<Vec<ReorderSuggestion>, Box<dyn std::error::Error>> {
    let dispatches = DispatchOrder::list_by_org(org_id)?;
    let now = now_unix();

    let suggestions = Godown::list_by_org(org_id)?
        .iter()
        .flat_map(|g| {
            g.stock
                .iter()
                .filter(|s| s.below_threshold)
                .map(|s| {
                    let velocity = weekly_velocity(&dispatches, &s.description, now, VELOCITY_LOOKBACK_DAYS);
                    ReorderSuggestion {
                        godown_name: g.name.clone(),
                        description: s.description.clone(),
                        quantity_on_hand: s.quantity,
                        weekly_velocity: velocity,
                        suggested_reorder_quantity: suggested_quantity(velocity, s.quantity),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(suggestions)
}

/// Build the prompt sent to the Anthropic API for the reorder narration.
/// Pulled out of [`generate_reorder_narration`] as a pure function so its
/// content can be unit tested without a network call, an API key, or a
/// database.
fn build_reorder_prompt(suggestions: &[ReorderSuggestion]) -> String {
    let lines = suggestions
        .iter()
        .map(|s| {
            if s.weekly_velocity > 0.0 {
                format!(
                    "{} at {}: {} units on hand, recent demand ~{:.1} units/week, suggest reordering about {} units",
                    s.description, s.godown_name, s.quantity_on_hand, s.weekly_velocity, s.suggested_reorder_quantity
                )
            } else {
                format!(
                    "{} at {}: {} units on hand, no recent dispatch activity to estimate demand from",
                    s.description, s.godown_name, s.quantity_on_hand
                )
            }
        })
        .collect::<Vec<_>>()
        .join("; ");

    format!(
        "You are a logistics operations assistant. Based only on the facts below, write a \
        short, plain-English briefing (as plain sentences, not bullet points) telling a \
        dispatcher what to reorder and roughly how much, and note when a suggestion couldn't \
        be estimated. Do not invent numbers not given below.\n\n\
        Godown stock below its reorder threshold: {lines}"
    )
}

/// Ask Claude to narrate an org's low-stock reorder suggestions. Returns a
/// canned "nothing is low" answer without calling Claude when no godown is
/// below its reorder threshold — the same short-circuit
/// [`crate::logistics::ai::digest::generate_daily_digest`] uses when nothing
/// needs attention.
pub async fn generate_reorder_narration(org_id: Uuid) -> Result<String, String> {
    let suggestions = compute_reorder_suggestions(org_id).map_err(|e| e.to_string())?;

    if suggestions.is_empty() {
        return Ok("No godowns are below their reorder threshold right now.".to_string());
    }

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let prompt = build_reorder_prompt(&suggestions);

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
    use crate::logistics::dispatch::dispatch::{DispatchLineItem, DispatchStatus};
    use crate::logistics::godown::godown::Godown;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::stock::stock::Stock;
    use crate::logistics::test_support::TestDb;

    fn dispatch_with_line(description: &str, quantity: i64, dispatched_at: i64) -> DispatchOrder {
        DispatchOrder {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            vehicle_registration_number: "MH12AB1234".to_string(),
            line_items: vec![DispatchLineItem {
                stock_description: description.to_string(),
                quantity,
                volume_in_size: 1,
            }],
            status: DispatchStatus::Delivered,
            dispatched_at,
            status_history: Vec::new(),
            proof_of_delivery: None,
            trip_id: None,
            stop_sequence: None,
        }
    }

    #[test]
    fn weekly_velocity_averages_recent_dispatches_over_the_lookback_window() {
        let now = 30 * SECONDS_PER_DAY;
        let dispatches = vec![
            dispatch_with_line("Cement", 30, now - 5 * SECONDS_PER_DAY),
            dispatch_with_line("Cement", 30, now - 10 * SECONDS_PER_DAY),
            dispatch_with_line("Bolts", 100, now - 5 * SECONDS_PER_DAY),
        ];
        // 60 units of Cement over a 30-day (~4.286 week) window -> 14 units/week.
        let v = weekly_velocity(&dispatches, "Cement", now, 30);
        assert!((v - 14.0).abs() < 0.1, "expected ~14.0, got {v}");
    }

    #[test]
    fn weekly_velocity_ignores_dispatches_outside_the_lookback_window() {
        let now = 30 * SECONDS_PER_DAY;
        let dispatches = vec![dispatch_with_line("Cement", 300, now - 60 * SECONDS_PER_DAY)];
        assert_eq!(weekly_velocity(&dispatches, "Cement", now, 30), 0.0);
    }

    #[test]
    fn suggested_quantity_targets_two_weeks_of_cover_above_what_is_on_hand() {
        // 14 units/week * 2 weeks = 28 target; 5 on hand -> suggest 23.
        assert_eq!(suggested_quantity(14.0, 5), 23);
    }

    #[test]
    fn suggested_quantity_never_goes_negative_when_on_hand_already_exceeds_the_target() {
        assert_eq!(suggested_quantity(2.0, 100), 0);
    }

    #[test]
    fn suggested_quantity_is_zero_without_any_recent_velocity() {
        assert_eq!(suggested_quantity(0.0, 5), 0);
    }

    #[test]
    fn build_reorder_prompt_includes_the_suggested_quantity_when_velocity_is_known() {
        let prompt = build_reorder_prompt(&[ReorderSuggestion {
            godown_name: "Chennai Central".to_string(),
            description: "Cement".to_string(),
            quantity_on_hand: 5,
            weekly_velocity: 14.0,
            suggested_reorder_quantity: 23,
        }]);
        assert!(prompt.contains("Cement at Chennai Central"));
        assert!(prompt.contains("5 units on hand"));
        assert!(prompt.contains("~14.0 units/week"));
        assert!(prompt.contains("suggest reordering about 23 units"));
    }

    #[test]
    fn build_reorder_prompt_notes_when_demand_cannot_be_estimated() {
        let prompt = build_reorder_prompt(&[ReorderSuggestion {
            godown_name: "Chennai Central".to_string(),
            description: "Cement".to_string(),
            quantity_on_hand: 5,
            weekly_velocity: 0.0,
            suggested_reorder_quantity: 0,
        }]);
        assert!(prompt.contains("no recent dispatch activity to estimate demand from"));
    }

    #[actix_web::test]
    async fn generate_reorder_narration_returns_a_canned_answer_when_nothing_is_low() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Well Stocked Co", "1 Depot Rd").expect("org");

        let narration = generate_reorder_narration(org.id).await.expect("narration");
        assert!(narration.contains("No godowns are below their reorder threshold"));
    }

    #[actix_web::test]
    async fn generate_reorder_narration_errors_when_the_api_key_is_not_set_but_something_is_low() {
        // SAFETY: no other test in this crate sets or reads ANTHROPIC_API_KEY
        // concurrently with this one — see status.rs's equivalent test for the
        // full rationale.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let _db = TestDb::create();
        let org = Organization::create_organization("Low Stock Co", "1 Depot Rd").expect("org");
        let godown = Godown::create(org.id, "Chennai Central", "12 Anna Salai", None).expect("godown");
        Stock::new(1, 5, "Cement")
            .with_reorder_threshold(Some(50))
            .add_to_godown(godown.id)
            .expect("seed low stock");

        let err = generate_reorder_narration(org.id).await.expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }

    #[actix_web::test]
    async fn compute_reorder_suggestions_only_returns_stock_below_its_threshold() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Mixed Stock Co", "1 Depot Rd").expect("org");
        let godown = Godown::create(org.id, "Chennai Central", "12 Anna Salai", None).expect("godown");
        Stock::new(1, 5, "Cement").with_reorder_threshold(Some(50)).add_to_godown(godown.id).expect("low stock");
        Stock::new(1, 200, "Sand").with_reorder_threshold(Some(50)).add_to_godown(godown.id).expect("healthy stock");

        let suggestions = compute_reorder_suggestions(org.id).expect("compute");
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].description, "Cement");
    }
}
