//! The generation half of the org-scoped "ask your data" assistant: given a
//! question, retrieve the org's most relevant chunks
//! (`crate::logistics::ai::chunk::search_by_org`) and ask Claude to answer
//! using only those facts. Mirrors `ai::status::generate_dispatch_summary`'s
//! call shape exactly — same model, same reqwest pattern, same header
//! handling — this is the second and last AI integration in the app.
//!
//! Phase 4 also injects a small always-current [`OpsReport`] snapshot into
//! the prompt alongside retrieved chunks, so aggregate-shaped questions
//! ("what's my fleet utilization") are answerable even though nothing in
//! the FULLTEXT index could ever match them — those numbers change on every
//! dispatch, so they're computed fresh on every call rather than kept as
//! chunks that would go stale immediately.

use crate::logistics::ai::chunk::{self, AiChunk};
use crate::logistics::reports::OpsReport;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How many chunks to hand to Claude as grounding for one answer.
const TOP_K: u32 = 8;

/// One fact the answer was grounded in, for a "based on N facts" UI footer.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AssistantSource {
    pub kind: String,
    pub source_id: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AssistantAnswer {
    pub answer: String,
    pub sources: Vec<AssistantSource>,
}

/// A compact, always-current summary of an org's [`OpsReport`] — fleet
/// utilization, delivery performance, recent volume and godown inventory —
/// suitable for dropping straight into a prompt. Pure, so it's unit-testable
/// without a database.
fn build_ops_snapshot_text(report: &OpsReport) -> String {
    let utilization = &report.vehicle_utilization;
    let delivery = &report.delivery_performance;

    let inventory = if report.godown_inventory.is_empty() {
        "no godowns recorded".to_string()
    } else {
        report
            .godown_inventory
            .iter()
            .map(|g| format!("{} holds {} units across {} item(s)", g.godown_name, g.units_on_hand, g.distinct_items))
            .collect::<Vec<_>>()
            .join("; ")
    };

    format!(
        "Fleet utilization: {}/{} vehicles on an active trip ({}%). Delivered dispatches: {}. \
        Returned: {}. Units dispatched in the last 30 days: {}. Godown inventory: {}.",
        utilization.vehicles_on_active_trip,
        utilization.total_vehicles,
        utilization.utilization_percent,
        delivery.delivered_count,
        delivery.returned_count,
        report.units_dispatched_recently,
        inventory
    )
}

/// Whether an org's [`OpsReport`] carries no real activity at all — no
/// vehicles, no dispatches, no godowns. Used to decide whether the canned
/// "nothing indexed yet" answer still applies when retrieval also found
/// nothing: an org with real operational data always has *something* to
/// ground an answer in, even without a single matching chunk.
fn ops_report_is_empty(report: &OpsReport) -> bool {
    report.vehicle_utilization.total_vehicles == 0
        && report.delivery_performance.delivered_count == 0
        && report.delivery_performance.returned_count == 0
        && report.units_dispatched_recently == 0
        && report.godown_inventory.is_empty()
}

/// Build the prompt sent to Claude: the current operational snapshot, the
/// retrieved facts, then the question, with an explicit instruction not to
/// answer beyond what's given. Pulled out as a pure function (mirrors
/// `ai::status::build_prompt`) so it's unit-testable without a network call.
fn build_answer_prompt(question: &str, chunks: &[AiChunk], ops_snapshot: &str) -> String {
    let facts = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}. [{}] {}", i + 1, c.kind.as_str(), c.text))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a logistics assistant answering questions about one organization's own data.\n\
        Answer only using the snapshot and facts below. If they don't contain the answer, say so \
        plainly \u{2014} do not guess or use outside knowledge.\n\n\
        Current operational snapshot: {ops_snapshot}\n\n\
        Facts:\n{facts}\n\n\
        Question: {question}"
    )
}

/// Answer `question` for `org_id`, grounded in that org's indexed data plus
/// a fresh [`OpsReport`] snapshot. Returns a canned "nothing indexed yet"
/// answer without calling Claude only when retrieval finds nothing *and*
/// the org has no operational data at all to fall back on.
pub async fn answer_question(org_id: Uuid, question: &str) -> Result<AssistantAnswer, String> {
    let chunks = chunk::search_by_org(org_id, question, TOP_K).map_err(|e| e.to_string())?;
    let report = OpsReport::for_org(org_id).map_err(|e| e.to_string())?;

    if chunks.is_empty() && ops_report_is_empty(&report) {
        return Ok(AssistantAnswer {
            answer: "I don't have anything indexed yet that matches that question — try again \
                     once there's some activity (notifications, vehicle documents) to draw on."
                .to_string(),
            sources: Vec::new(),
        });
    }

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY environment variable not set".to_string())?;

    let ops_snapshot = build_ops_snapshot_text(&report);
    let prompt = build_answer_prompt(question, &chunks, &ops_snapshot);

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
            "max_tokens": 512,
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

    let answer = json["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Unexpected response format from Anthropic API".to_string())?;

    let sources = chunks
        .iter()
        .map(|c| AssistantSource {
            kind: c.kind.as_str().to_string(),
            source_id: c.source_id.clone(),
            excerpt: c.text.clone(),
        })
        .collect();

    Ok(AssistantAnswer { answer, sources })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::ai::chunk::ChunkKind;
    use crate::logistics::test_support::TestDb;

    fn sample_chunk(kind: ChunkKind, text: &str) -> AiChunk {
        AiChunk {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            kind,
            source_id: "src-1".to_string(),
            text: text.to_string(),
            updated_at: 1_700_000_000,
        }
    }

    #[test]
    fn build_answer_prompt_includes_the_question_every_fact_and_the_snapshot() {
        let chunks = vec![
            sample_chunk(ChunkKind::Notification, "Order abcd1234 was dispatched"),
            sample_chunk(ChunkKind::VehicleDocument, "MH12AB1234 insurance expires 2026-10-02"),
        ];
        let prompt = build_answer_prompt(
            "What happened to order abcd1234?",
            &chunks,
            "Fleet utilization: 1/4 vehicles on an active trip (25%).",
        );

        assert!(prompt.contains("What happened to order abcd1234?"));
        assert!(prompt.contains("Order abcd1234 was dispatched"));
        assert!(prompt.contains("MH12AB1234 insurance expires 2026-10-02"));
        assert!(prompt.contains("notification"));
        assert!(prompt.contains("vehicle_document"));
        assert!(prompt.contains("do not guess"));
        assert!(prompt.contains("Fleet utilization: 1/4 vehicles on an active trip (25%)."));
    }

    #[test]
    fn build_answer_prompt_handles_no_facts() {
        let prompt = build_answer_prompt("anything?", &[], "no snapshot data");
        assert!(prompt.contains("anything?"));
        assert!(prompt.contains("Facts:"));
        assert!(prompt.contains("no snapshot data"));
    }

    fn sample_report() -> OpsReport {
        use crate::logistics::reports::{DeliveryPerformance, VehicleUtilization};
        OpsReport {
            vehicle_utilization: VehicleUtilization { total_vehicles: 4, vehicles_on_active_trip: 1, utilization_percent: 25.0 },
            delivery_performance: DeliveryPerformance {
                delivered_count: 10, returned_count: 2, avg_hours_to_deliver: Some(18.5), on_time_rate_percent: Some(80.0),
            },
            units_dispatched_recently: 340,
            godown_inventory: Vec::new(),
            dispatch_volume: Vec::new(),
            hired_transport: Default::default(),
        }
    }

    #[test]
    fn build_ops_snapshot_text_includes_utilization_delivery_and_volume_figures() {
        let text = build_ops_snapshot_text(&sample_report());
        assert!(text.contains("1/4 vehicles on an active trip (25%)"));
        assert!(text.contains("Delivered dispatches: 10"));
        assert!(text.contains("Returned: 2"));
        assert!(text.contains("340"));
        assert!(text.contains("no godowns recorded"));
    }

    #[test]
    fn ops_report_is_empty_is_true_for_an_org_with_no_activity_at_all() {
        let mut report = sample_report();
        report.vehicle_utilization.total_vehicles = 0;
        report.delivery_performance.delivered_count = 0;
        report.delivery_performance.returned_count = 0;
        report.units_dispatched_recently = 0;
        assert!(ops_report_is_empty(&report));
    }

    #[test]
    fn ops_report_is_empty_is_false_when_any_field_has_data() {
        assert!(!ops_report_is_empty(&sample_report()));
    }

    #[actix_web::test]
    async fn answer_question_returns_a_canned_answer_when_nothing_is_indexed() {
        let _db = TestDb::create();
        let org_id = Uuid::new_v4();
        let answer = answer_question(org_id, "anything at all?").await.unwrap();
        assert!(answer.sources.is_empty());
        assert!(answer.answer.to_lowercase().contains("indexed yet"));
    }

    #[actix_web::test]
    async fn answer_question_errors_when_the_api_key_is_not_set_but_chunks_exist() {
        let _db = TestDb::create();
        let org = crate::logistics::orgs::orgs::Organization::create_organization(
            "Assistant Test Org",
            "1 Depot Rd",
        )
        .expect("org");
        let org_id = org.id;
        chunk::upsert(
            org_id,
            ChunkKind::Notification,
            "n1",
            "a matching fact about widgets".to_string(),
        )
        .unwrap();

        // SAFETY: this module is the only code that reads ANTHROPIC_API_KEY,
        // and no other test sets it, so this doesn't race with anything else.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        let err = answer_question(org_id, "widgets")
            .await
            .expect_err("should fail without an API key once there are chunks to answer from");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }

    #[actix_web::test]
    async fn answer_question_does_not_short_circuit_when_the_org_has_operational_data_even_without_a_matching_chunk() {
        use crate::logistics::godown::godown::Godown;
        use crate::logistics::orgs::orgs::Organization;

        let _db = TestDb::create();
        let org = Organization::create_organization("Ops Snapshot Org", "1 Depot Rd").expect("org");
        // A godown with no matching chunk content, but enough to make the
        // OpsReport non-empty (a real "what's my fleet utilization"-shaped
        // question has nothing in the FULLTEXT index to match, ever).
        Godown::create(org.id, "Chennai Central", "12 Anna Salai", None).expect("godown");

        // SAFETY: this module is the only code that reads ANTHROPIC_API_KEY,
        // and no other test sets it, so this doesn't race with anything else.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        let err = answer_question(org.id, "what's my fleet utilization?")
            .await
            .expect_err("an org with real operational data should attempt generation, not short-circuit");
        assert!(err.contains("ANTHROPIC_API_KEY"), "unexpected: {err}");
    }
}
