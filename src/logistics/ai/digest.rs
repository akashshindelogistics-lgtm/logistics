//! A daily ops digest: an on-demand (not scheduled) AI summary of "what
//! needs attention today" across dispatches, compliance and stock. Also
//! prompt-over-structured-data, one level broader than
//! [`crate::logistics::ai::report::generate_report_summary`]: it pulls
//! together dispatches stuck in a status too long, vehicles with expiring or
//! expired compliance documents, and godown stock below its reorder
//! threshold — all already-computed fields, no new queries of consequence —
//! and asks Claude to prioritize them into a short actionable list. Folded
//! into the "ask your data" assistant widget as a canned starter question
//! rather than a page of its own, since the widget already gives it a place
//! to live. See `todo.org`'s "AI opportunities" section for the full design.

use crate::logistics::dispatch::dispatch::DispatchOrder;
use crate::logistics::godown::godown::Godown;
use crate::logistics::vehicle::document::{ComplianceStatus, VehicleDocument};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A dispatch counts as "stuck" once it's been sitting in the same
/// non-terminal status for this long without moving.
const STALE_DISPATCH_HOURS: i64 = 48;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The most recent status change on a dispatch — its last `status_history`
/// entry, or `dispatched_at` if it has never changed since creation.
fn last_status_change_at(dispatch: &DispatchOrder) -> i64 {
    dispatch
        .status_history
        .last()
        .map(|e| e.changed_at)
        .unwrap_or(dispatch.dispatched_at)
}

/// A godown's stock row that's fallen below its reorder threshold — plain
/// data rather than borrowed [`Godown`]/[`Stock`] references, so
/// [`build_digest_prompt`] stays simple to construct in a unit test.
#[derive(Debug, Clone)]
pub struct LowStockItem {
    pub godown_name: String,
    pub description: String,
    pub quantity: i64,
}

/// Build the prompt sent to the Anthropic API for the daily digest. Pulled
/// out of [`generate_daily_digest`] as a pure function so its content can be
/// unit tested without a network call, an API key, or a database.
fn build_digest_prompt(
    stale_dispatches: &[DispatchOrder],
    expiring_documents: &[VehicleDocument],
    low_stock: &[LowStockItem],
) -> String {
    let stale_lines = if stale_dispatches.is_empty() {
        "none".to_string()
    } else {
        stale_dispatches
            .iter()
            .map(|d| {
                format!(
                    "Dispatch {} has been {:?} for over {STALE_DISPATCH_HOURS} hours (vehicle {})",
                    &d.id.to_string()[..8],
                    d.status,
                    d.vehicle_registration_number
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };

    let expiring_lines = if expiring_documents.is_empty() {
        "none".to_string()
    } else {
        expiring_documents
            .iter()
            .map(|d| {
                format!(
                    "Vehicle {}'s {} (#{}) is {:?}, expires {}",
                    d.vehicle_registration,
                    d.doc_type.as_str(),
                    d.document_number,
                    d.status,
                    d.expires_on
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    };

    let low_stock_lines = if low_stock.is_empty() {
        "none".to_string()
    } else {
        low_stock
            .iter()
            .map(|i| format!("{} at {} ({} units left)", i.description, i.godown_name, i.quantity))
            .collect::<Vec<_>>()
            .join("; ")
    };

    format!(
        "You are a logistics operations assistant. Based only on the facts below, write a \
        short, prioritized briefing (as plain sentences, not bullet points) of what a \
        dispatcher should look at today. If a category says \"none\", don't mention it at all. \
        Do not invent anything not listed below.\n\n\
        Dispatches stuck in the same status for over {STALE_DISPATCH_HOURS} hours: {stale_lines}\n\
        Vehicle compliance documents expiring soon or already expired: {expiring_lines}\n\
        Godown stock below its reorder threshold: {low_stock_lines}"
    )
}

/// Ask Claude to prioritize an org's outstanding operational issues into a
/// short "what needs your attention today" briefing. Returns a canned
/// "nothing needs attention" answer without calling Claude when every
/// category is empty — the same short-circuit
/// [`crate::logistics::ai::assistant::answer_question`] uses when retrieval
/// finds nothing.
pub async fn generate_daily_digest(org_id: Uuid) -> Result<String, String> {
    let dispatches = DispatchOrder::list_by_org(org_id).map_err(|e| e.to_string())?;
    let now = now_unix();
    let stale_dispatches: Vec<DispatchOrder> = dispatches
        .into_iter()
        .filter(|d| {
            !d.status.is_terminal()
                && now - last_status_change_at(d) >= STALE_DISPATCH_HOURS * 3600
        })
        .collect();

    let expiring_documents: Vec<VehicleDocument> = VehicleDocument::list_by_org(org_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|d| matches!(d.status, ComplianceStatus::ExpiringSoon | ComplianceStatus::Expired))
        .collect();

    let low_stock: Vec<LowStockItem> = Godown::list_by_org(org_id)
        .map_err(|e| e.to_string())?
        .iter()
        .flat_map(|g| {
            g.stock.iter().filter(|s| s.below_threshold).map(|s| LowStockItem {
                godown_name: g.name.clone(),
                description: s.description.clone(),
                quantity: s.quantity,
            })
        })
        .collect();

    if stale_dispatches.is_empty() && expiring_documents.is_empty() && low_stock.is_empty() {
        return Ok(
            "Nothing needs your attention right now - no dispatches stuck, no compliance \
             documents expiring soon, and no godowns below their reorder threshold."
                .to_string(),
        );
    }

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let prompt = build_digest_prompt(&stale_dispatches, &expiring_documents, &low_stock);

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
    use crate::logistics::dispatch::dispatch::{DispatchStatus, DispatchStatusEvent};
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::TestDb;
    use crate::logistics::vehicle::document::ComplianceDocType;

    fn sample_dispatch(status: DispatchStatus, changed_at: i64) -> DispatchOrder {
        DispatchOrder {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            vehicle_registration_number: "MH12AB1234".to_string(),
            line_items: Vec::new(),
            status,
            dispatched_at: changed_at,
            status_history: vec![DispatchStatusEvent { status, changed_at }],
            proof_of_delivery: None,
            trip_id: None,
            stop_sequence: None,
        }
    }

    fn sample_document(status: ComplianceStatus) -> VehicleDocument {
        VehicleDocument {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            vehicle_registration: "MH12AB1234".to_string(),
            doc_type: ComplianceDocType::Insurance,
            document_number: "INS-9911".to_string(),
            issued_on: None,
            expires_on: "2026-10-02".to_string(),
            notes: None,
            days_until_expiry: 2,
            status,
        }
    }

    #[test]
    fn build_digest_prompt_includes_stale_dispatches_expiring_documents_and_low_stock() {
        let prompt = build_digest_prompt(
            &[sample_dispatch(DispatchStatus::InTransit, 0)],
            &[sample_document(ComplianceStatus::ExpiringSoon)],
            &[LowStockItem {
                godown_name: "Chennai Central".to_string(),
                description: "Cement".to_string(),
                quantity: 4,
            }],
        );
        assert!(prompt.contains("InTransit"));
        assert!(prompt.contains("MH12AB1234"));
        assert!(prompt.contains("Insurance"));
        assert!(prompt.contains("ExpiringSoon"));
        assert!(prompt.contains("Cement at Chennai Central (4 units left)"));
    }

    #[test]
    fn build_digest_prompt_says_none_for_every_empty_category() {
        let prompt = build_digest_prompt(&[], &[], &[]);
        assert!(prompt.contains("hours: none"));
        assert!(prompt.contains("expired: none"));
        assert!(prompt.contains("threshold: none"));
    }

    #[actix_web::test]
    async fn generate_daily_digest_returns_a_canned_answer_when_nothing_needs_attention() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Quiet Ops Co", "1 Depot Rd").expect("org");

        let digest = generate_daily_digest(org.id).await.expect("digest");
        assert!(digest.contains("Nothing needs your attention"));
    }

    #[actix_web::test]
    async fn generate_daily_digest_errors_when_the_api_key_is_not_set_but_something_needs_attention() {
        // SAFETY: no other test in this crate sets or reads ANTHROPIC_API_KEY
        // concurrently with this one — see status.rs's equivalent test for the
        // full rationale.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
        let _db = TestDb::create();
        let org = Organization::create_organization("Busy Ops Co", "1 Depot Rd").expect("org");

        let mut dispatch = sample_dispatch(DispatchStatus::InTransit, now_unix() - STALE_DISPATCH_HOURS * 3600 - 1);
        dispatch.org_id = org.id;
        dispatch.save().expect("save dispatch");

        let err = generate_daily_digest(org.id).await.expect_err("should fail without an API key");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }
}
