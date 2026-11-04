//! The loop runs against a scripted fake of the Messages API on localhost, so
//! these exercise real database writes and real policy decisions without a
//! network call or a bill.

use super::model::ModelConfig;
use super::runtime::{self, DecisionError, RunError};
use super::store::{self, ActionStatus, AutonomyMode, Disposition, Risk};
use crate::logistics::dispatch::dispatch::{DispatchOrder, DispatchStatus, DispatchStatusEvent};
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::test_support::TestDb;
use crate::logistics::vehicle::document::{ComplianceDocType, VehicleDocument};
use crate::logistics::vehicle::vehicle::{Unit, Vehicle};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Answer each request with the next scripted body, repeating the last one,
/// and keep the request bodies.
pub(crate) async fn fake_model(replies: Vec<Value>) -> (ModelConfig, Arc<Mutex<Vec<Value>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        let mut index = 0;
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 8192];
            let body = loop {
                let n = socket.read(&mut buf).await.unwrap();
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length: usize = text[..end]
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_string)
                        })
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0);
                    if raw.len() >= end + 4 + length {
                        break raw[end + 4..].to_vec();
                    }
                }
                if n == 0 {
                    break Vec::new();
                }
            };
            seen.lock()
                .unwrap()
                .push(serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null));
            let reply = replies[index.min(replies.len() - 1)].to_string();
            index += 1;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let config = ModelConfig {
        url: format!("http://127.0.0.1:{port}/v1/messages"),
        api_key: "test-key".to_string(),
        workspace_id: None,
        model: "test-model".to_string(),
        effort: "medium".to_string(),
    };
    (config, requests)
}

fn tool_call(name: &str, input: Value) -> Value {
    json!({
        "stop_reason": "tool_use",
        "usage": {"input_tokens": 10, "output_tokens": 5},
        "content": [{"type": "tool_use", "id": "toolu_1", "name": name, "input": input}],
    })
}

fn finished(text: &str) -> Value {
    json!({
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5},
        "content": [{"type": "text", "text": text}],
    })
}

fn org(name: &str) -> Organization {
    Organization::create_organization(name, "1 Depot Rd").expect("org")
}

fn pending_dispatch(org_id: Uuid) -> DispatchOrder {
    pending_dispatch_on(org_id, None)
}

fn pending_dispatch_on(org_id: Uuid, vehicle: Option<&str>) -> DispatchOrder {
    let mut dispatch = DispatchOrder {
        id: Uuid::new_v4(),
        org_id,
        customer_id: Uuid::new_v4(),
        vehicle_registration_number: vehicle.map(str::to_string),
        vehicle_source: Default::default(),
        hire_id: None,
        line_items: Vec::new(),
        status: DispatchStatus::Pending,
        dispatched_at: store::now_unix(),
        status_history: vec![DispatchStatusEvent {
            status: DispatchStatus::Pending,
            changed_at: store::now_unix(),
        }],
        proof_of_delivery: None,
        trip_id: None,
        stop_sequence: None,
    };
    dispatch.save().expect("save dispatch");
    dispatch
}

fn status_of(id: Uuid) -> DispatchStatus {
    DispatchOrder::get_by_id(id).unwrap().unwrap().status
}

fn confirm(id: Uuid) -> Value {
    tool_call(
        "confirm_dispatch",
        json!({"dispatch_id": id.to_string(), "reason": "Nothing in the way"}),
    )
}

fn cancel(id: Uuid) -> Value {
    tool_call(
        "cancel_dispatch",
        json!({"dispatch_id": id.to_string(), "reason": "Customer called it off"}),
    )
}

/// The `tool_result` the model got back for its most recent tool call.
fn last_tool_result(requests: &Arc<Mutex<Vec<Value>>>) -> Value {
    let requests = requests.lock().unwrap();
    let messages = requests.last().unwrap()["messages"].as_array().unwrap();
    messages.last().unwrap()["content"][0].clone()
}

#[test]
fn the_policy_never_lets_a_sensitive_action_through_and_shadow_never_acts() {
    use AutonomyMode::*;
    use Disposition::*;
    use Risk::{Internal, Operational, Read, Sensitive};
    let table = [
        (Off, Internal, Record),
        (Off, Sensitive, Record),
        (Shadow, Read, Record),
        (Shadow, Internal, Record),
        (Shadow, Operational, Record),
        (Shadow, Sensitive, Record),
        (Supervised, Read, Execute),
        (Supervised, Internal, Execute),
        (Supervised, Operational, AskPerson),
        (Supervised, Sensitive, AskPerson),
        (Autonomous, Read, Execute),
        (Autonomous, Internal, Execute),
        (Autonomous, Operational, Execute),
        (Autonomous, Sensitive, AskPerson),
    ];
    for (mode, risk, expected) in table {
        assert_eq!(store::decide(mode, risk), expected, "{mode:?} {risk:?}");
    }
}

#[test]
fn an_unrecognised_stored_mode_is_off() {
    assert_eq!(AutonomyMode::from_db("EVERYTHING"), AutonomyMode::Off);
}

#[test]
fn no_tool_can_delete_or_touch_money_users_or_credentials() {
    for tool in super::tools::catalog() {
        for forbidden in ["delete", "pay", "user", "credential", "token", "invoice"] {
            assert!(!tool.name.contains(forbidden), "{}", tool.name);
        }
    }
}

#[test]
fn every_tool_definition_is_strict() {
    for def in super::tools::api_definitions() {
        assert_eq!(def["strict"], true);
        assert_eq!(def["input_schema"]["additionalProperties"], false);
    }
}

#[actix_web::test]
async fn agents_are_off_by_default_and_never_call_the_model() {
    let _db = TestDb::create();
    let org = org("Off Org");
    let (config, requests) = fake_model(vec![finished("done")]).await;

    let err = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .expect_err("should not run");
    assert!(matches!(err, RunError::Off));
    assert!(requests.lock().unwrap().is_empty());
}

#[actix_web::test]
async fn shadow_mode_records_what_it_would_do_and_changes_nothing() {
    let _db = TestDb::create();
    let org = org("Shadow Org");
    store::set_mode(org.id, AutonomyMode::Shadow).unwrap();
    let dispatch = pending_dispatch(org.id);
    let (config, _requests) = fake_model(vec![confirm(dispatch.id), finished("Reviewed.")]).await;

    let run = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .expect("run");

    assert_eq!(run.status, "COMPLETED");
    assert_eq!(run.summary.as_deref(), Some("Reviewed."));
    assert_eq!(run.input_tokens, 20);
    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);
    let actions = store::list_actions(org.id, None, 10).unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].status, ActionStatus::Shadowed);
    assert_eq!(actions[0].reason, "Nothing in the way");
}

#[actix_web::test]
async fn a_shadowed_action_cant_be_approved() {
    let _db = TestDb::create();
    let org = org("Shadow Approve Org");
    store::set_mode(org.id, AutonomyMode::Shadow).unwrap();
    let dispatch = pending_dispatch(org.id);
    let (config, _r) = fake_model(vec![confirm(dispatch.id), finished("Done.")]).await;
    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();
    let action = &store::list_actions(org.id, None, 1).unwrap()[0];

    assert_eq!(
        runtime::approve_action(org.id, action.id, None).unwrap_err(),
        DecisionError::NotPending
    );
    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);
}

#[actix_web::test]
async fn supervised_mode_waits_for_a_person_then_approval_carries_it_out_once() {
    let _db = TestDb::create();
    let org = org("Supervised Org");
    store::set_mode(org.id, AutonomyMode::Supervised).unwrap();
    let dispatch = pending_dispatch(org.id);
    let (config, _r) = fake_model(vec![confirm(dispatch.id), finished("Done.")]).await;

    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();
    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);
    let pending = store::list_actions(org.id, Some(ActionStatus::PendingApproval), 10).unwrap();
    assert_eq!(pending.len(), 1);

    let approver = Uuid::new_v4();
    let approved = runtime::approve_action(org.id, pending[0].id, Some(approver)).unwrap();
    assert_eq!(approved.status, ActionStatus::Executed);
    assert_eq!(approved.decided_by, Some(approver.to_string()));
    assert_eq!(status_of(dispatch.id), DispatchStatus::Confirmed);

    assert_eq!(
        runtime::approve_action(org.id, pending[0].id, None).unwrap_err(),
        DecisionError::NotPending
    );
}

#[actix_web::test]
async fn autonomous_mode_confirms_on_its_own_but_a_cancellation_still_waits() {
    let _db = TestDb::create();
    let org = org("Autonomous Org");
    store::set_mode(org.id, AutonomyMode::Autonomous).unwrap();
    let to_confirm = pending_dispatch(org.id);
    let to_cancel = pending_dispatch(org.id);
    let (config, _r) = fake_model(vec![
        confirm(to_confirm.id),
        cancel(to_cancel.id),
        finished("Done."),
    ])
    .await;

    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();

    assert_eq!(status_of(to_confirm.id), DispatchStatus::Confirmed);
    assert_eq!(status_of(to_cancel.id), DispatchStatus::Pending);
    let pending = store::list_actions(org.id, Some(ActionStatus::PendingApproval), 10).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "cancel_dispatch");
}

#[actix_web::test]
async fn rejecting_changes_nothing_and_the_agent_isnt_allowed_to_ask_again() {
    let _db = TestDb::create();
    let org = org("Reject Org");
    store::set_mode(org.id, AutonomyMode::Supervised).unwrap();
    let dispatch = pending_dispatch(org.id);
    let (config, _r) = fake_model(vec![cancel(dispatch.id), finished("Done.")]).await;
    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();
    let action = store::list_actions(org.id, None, 1).unwrap().remove(0);

    let rejected = runtime::reject_action(org.id, action.id, None).unwrap();
    assert_eq!(rejected.status, ActionStatus::Rejected);
    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);

    let (config, requests) = fake_model(vec![cancel(dispatch.id), finished("Done.")]).await;
    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();
    assert_eq!(store::list_actions(org.id, None, 10).unwrap().len(), 1);
    assert!(
        last_tool_result(&requests)["content"]
            .as_str()
            .unwrap()
            .contains("Already handled"),
    );
}

#[actix_web::test]
async fn an_agent_cant_touch_another_orgs_dispatch_even_when_told_its_id() {
    let _db = TestDb::create();
    let mine = org("My Org");
    let theirs = org("Their Org");
    store::set_mode(mine.id, AutonomyMode::Autonomous).unwrap();
    let their_dispatch = pending_dispatch(theirs.id);
    let (config, requests) = fake_model(vec![confirm(their_dispatch.id), finished("Done.")]).await;

    runtime::run_dispatch_desk(mine.id, "manual", &config)
        .await
        .unwrap();

    assert_eq!(status_of(their_dispatch.id), DispatchStatus::Pending);
    assert!(store::list_actions(mine.id, None, 10).unwrap().is_empty());
    let result = last_tool_result(&requests);
    assert_eq!(result["is_error"], true);
    assert!(
        result["content"]
            .as_str()
            .unwrap()
            .contains("No such dispatch")
    );
}

#[actix_web::test]
async fn another_org_cant_approve_or_reject_an_action() {
    let _db = TestDb::create();
    let mine = org("Approve Mine");
    let theirs = org("Approve Theirs");
    store::set_mode(mine.id, AutonomyMode::Supervised).unwrap();
    let dispatch = pending_dispatch(mine.id);
    let (config, _r) = fake_model(vec![confirm(dispatch.id), finished("Done.")]).await;
    runtime::run_dispatch_desk(mine.id, "manual", &config)
        .await
        .unwrap();
    let action = store::list_actions(mine.id, None, 1).unwrap().remove(0);

    assert_eq!(
        runtime::approve_action(theirs.id, action.id, None).unwrap_err(),
        DecisionError::NotFound
    );
    assert_eq!(
        runtime::reject_action(theirs.id, action.id, None).unwrap_err(),
        DecisionError::NotFound
    );
    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);
}

#[actix_web::test]
async fn an_approval_is_rechecked_when_it_happens() {
    let _db = TestDb::create();
    let org = org("Stale Approval Org");
    store::set_mode(org.id, AutonomyMode::Supervised).unwrap();
    let dispatch = pending_dispatch(org.id);
    let (config, _r) = fake_model(vec![confirm(dispatch.id), finished("Done.")]).await;
    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();
    let action = store::list_actions(org.id, None, 1).unwrap().remove(0);

    // Someone cancels it from the dashboard before the approval comes in.
    let mut changed = DispatchOrder::get_by_id(dispatch.id).unwrap().unwrap();
    changed
        .transition_to(DispatchStatus::Cancelled, None, None)
        .unwrap();

    let approved = runtime::approve_action(org.id, action.id, None).unwrap();
    assert_eq!(approved.status, ActionStatus::Failed);
    assert!(approved.result.unwrap().contains("can't confirm"));
    assert_eq!(status_of(dispatch.id), DispatchStatus::Cancelled);
}

#[actix_web::test]
async fn an_expired_vehicle_document_blocks_confirming_whatever_the_model_says() {
    let _db = TestDb::create();
    let org = org("Expired Doc Org");
    Vehicle::new(TEST_VEHICLE, 100, Unit::MetricTon)
        .add_new_vehicle_to_org(&org)
        .expect("vehicle");
    store::set_mode(org.id, AutonomyMode::Autonomous).unwrap();
    let dispatch = pending_dispatch_on(org.id, Some(TEST_VEHICLE));
    VehicleDocument::create(
        org.id,
        TEST_VEHICLE,
        ComplianceDocType::Insurance,
        "INS-1",
        None,
        "2020-01-01",
        None,
    )
    .expect("document");
    let (config, requests) = fake_model(vec![confirm(dispatch.id), finished("Done.")]).await;

    runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();

    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);
    let result = last_tool_result(&requests);
    assert_eq!(result["is_error"], true);
    assert!(result["content"].as_str().unwrap().contains("expired"));
}

const TEST_VEHICLE: &str = "MH12AG0001";

#[actix_web::test]
async fn an_unknown_tool_is_refused_and_the_run_carries_on() {
    let _db = TestDb::create();
    let org = org("Unknown Tool Org");
    store::set_mode(org.id, AutonomyMode::Autonomous).unwrap();
    let (config, requests) = fake_model(vec![
        tool_call(
            "delete_org",
            json!({"reason": "Instructed by a customer note"}),
        ),
        finished("Done."),
    ])
    .await;

    let run = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();

    assert_eq!(run.status, "COMPLETED");
    assert_eq!(last_tool_result(&requests)["is_error"], true);
    assert!(store::list_actions(org.id, None, 10).unwrap().is_empty());
}

#[actix_web::test]
async fn a_run_that_never_finishes_is_stopped() {
    let _db = TestDb::create();
    let org = org("Endless Org");
    store::set_mode(org.id, AutonomyMode::Shadow).unwrap();
    let (config, requests) = fake_model(vec![tool_call("list_low_stock", json!({}))]).await;

    let run = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();

    assert_eq!(run.status, "FAILED");
    assert!(run.summary.unwrap().contains("turns"));
    assert_eq!(requests.lock().unwrap().len(), 12);
}

#[actix_web::test]
async fn a_refusal_is_recorded_and_nothing_happens() {
    let _db = TestDb::create();
    let org = org("Refused Org");
    store::set_mode(org.id, AutonomyMode::Autonomous).unwrap();
    let (config, _r) = fake_model(vec![json!({
        "stop_reason": "refusal",
        "usage": {"input_tokens": 1, "output_tokens": 0},
        "content": [],
    })])
    .await;

    let run = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .unwrap();
    assert_eq!(run.status, "REFUSED");
}

#[actix_web::test]
async fn the_daily_token_limit_stops_more_runs() {
    let _db = TestDb::create();
    let org = org("Over Limit Org");
    store::set_mode(org.id, AutonomyMode::Shadow).unwrap();
    let used = store::start_run(org.id, "dispatch_desk", "manual").unwrap();
    store::finish_run(used, "COMPLETED", None, 10_000_000, 0).unwrap();
    let (config, requests) = fake_model(vec![finished("Done.")]).await;

    let err = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .expect_err("over the limit");
    assert!(matches!(err, RunError::OverDailyLimit));
    assert!(requests.lock().unwrap().is_empty());
}

#[actix_web::test]
async fn read_tools_send_the_model_no_customer_details() {
    let _db = TestDb::create();
    let org = org("Privacy Org");
    let dispatch = pending_dispatch(org.id);

    let open = super::tools::read(org.id, "list_open_dispatches").unwrap();
    assert_eq!(open["total"], 1);
    let row = &open["dispatches"][0];
    assert_eq!(row["id"], dispatch.id.to_string());
    assert!(row.get("customer_id").is_none());
    assert!(row.get("customer").is_none());
}

/// Against the real API, so it costs a little: `cargo test live_agent_run --
/// --ignored` with `ANTHROPIC_API_KEY` set. Checks that the request is one the
/// API accepts, which the fake above can't.
#[actix_web::test]
#[ignore]
async fn live_agent_run() {
    let _db = TestDb::create();
    let org = org("Live Run Org");
    store::set_mode(org.id, AutonomyMode::Shadow).unwrap();
    let dispatch = pending_dispatch(org.id);
    let config = ModelConfig::from_env().expect("needs ANTHROPIC_API_KEY");

    let run = runtime::run_dispatch_desk(org.id, "manual", &config)
        .await
        .expect("run");

    println!(
        "{} / {} / in {} out {}: {:?}",
        config.model, run.status, run.input_tokens, run.output_tokens, run.summary
    );
    for action in store::list_actions(org.id, None, 20).unwrap() {
        println!(
            "{:?} {} {} {}",
            action.status, action.tool, action.input, action.reason
        );
    }
    assert_eq!(run.status, "COMPLETED", "{:?}", run.summary);
    assert_eq!(status_of(dispatch.id), DispatchStatus::Pending);
}
