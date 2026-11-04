use super::*;
use crate::logistics::agent::store::{self, ActionStatus, AgentAction, AutonomyMode, Risk};

fn pending_action(org_id: Uuid, dispatch_id: Uuid) -> Uuid {
    let run_id = store::start_run(org_id, "dispatch_desk", "manual").unwrap();
    store::record_action(store::NewAction {
        run_id,
        org_id,
        agent: "dispatch_desk",
        tool: "confirm_dispatch",
        input: &serde_json::json!({"dispatch_id": dispatch_id, "reason": "Ready"}).to_string(),
        reason: "Ready",
        risk: Risk::Operational,
        status: ActionStatus::PendingApproval,
        result: None,
        dedupe_key: &format!("confirm:{dispatch_id}"),
    })
    .unwrap()
}

#[actix_web::test]
async fn test_agents_start_off_and_only_an_admin_can_change_the_mode() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Agent Settings Org").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/agents/settings", org.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let settings =
        test::read_body_json::<ApiResponse<AgentSettings>, _>(test::call_service(&app, req).await)
            .await
            .data
            .unwrap();
    assert_eq!(settings.mode, AutonomyMode::Off);

    let dispatcher = add_user_and_login(
        &app,
        org.id,
        &auth,
        "agent-dispatcher@example.com",
        OrgRole::Dispatcher,
    )
    .await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}/agents/settings", org.id))
        .insert_header(("Authorization", dispatcher))
        .set_json(AgentSettings {
            mode: AutonomyMode::Autonomous,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}/agents/settings", org.id))
        .insert_header(("Authorization", auth))
        .set_json(AgentSettings {
            mode: AutonomyMode::Shadow,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);
    assert_eq!(store::get_mode(org.id).unwrap(), AutonomyMode::Shadow);
}

#[actix_web::test]
async fn test_agent_routes_are_scoped_to_the_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _) = setup_org(&app, "Agent Scope Org").await;
    let (_other, intruder) = setup_org(&app, "Agent Scope Intruder").await;

    for uri in ["settings", "runs", "actions"] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/agents/{uri}", org.id))
            .insert_header(("Authorization", intruder.clone()))
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status().as_u16(),
            403,
            "{uri}"
        );
    }
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/agents/run", org.id))
        .insert_header(("Authorization", intruder))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_approving_through_the_api_carries_the_action_out_once() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, dispatch, auth) = setup_dispatch(&app, "Agent Approve Org").await;
    let (_other, intruder) = setup_org(&app, "Agent Approve Intruder").await;
    let action_id = pending_action(org.id, dispatch.id);

    let approve = |who: String| {
        test::TestRequest::post()
            .uri(&format!("/api/agent-actions/{action_id}/approve"))
            .insert_header(("Authorization", who))
            .to_request()
    };

    assert_eq!(
        test::call_service(&app, approve(intruder))
            .await
            .status()
            .as_u16(),
        404
    );

    let list = test::TestRequest::get()
        .uri(&format!(
            "/api/orgs/{}/agents/actions?status=PENDING_APPROVAL",
            org.id
        ))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let pending = test::read_body_json::<ApiResponse<Vec<AgentAction>>, _>(
        test::call_service(&app, list).await,
    )
    .await
    .data
    .unwrap();
    assert_eq!(pending.len(), 1);

    let done = test::read_body_json::<ApiResponse<AgentAction>, _>(
        test::call_service(&app, approve(auth.clone())).await,
    )
    .await
    .data
    .unwrap();
    assert_eq!(done.status, ActionStatus::Executed);
    assert_eq!(
        DispatchOrder::get_by_id(dispatch.id)
            .unwrap()
            .unwrap()
            .status,
        DispatchStatus::Confirmed
    );

    assert_eq!(
        test::call_service(&app, approve(auth))
            .await
            .status()
            .as_u16(),
        409
    );
}

#[actix_web::test]
async fn test_warehouse_staff_cant_decide_agent_actions() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, dispatch, auth) = setup_dispatch(&app, "Agent Role Org").await;
    let staff = add_user_and_login(
        &app,
        org.id,
        &auth,
        "agent-staff@example.com",
        OrgRole::WarehouseStaff,
    )
    .await;
    let action_id = pending_action(org.id, dispatch.id);

    for verb in ["approve", "reject"] {
        let req = test::TestRequest::post()
            .uri(&format!("/api/agent-actions/{action_id}/{verb}"))
            .insert_header(("Authorization", staff.clone()))
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status().as_u16(),
            403,
            "{verb}"
        );
    }
    assert_eq!(
        store::get_action(action_id).unwrap().unwrap().status,
        ActionStatus::PendingApproval
    );
}

#[actix_web::test]
async fn test_rejecting_through_the_api_changes_nothing() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, dispatch, auth) = setup_dispatch(&app, "Agent Reject Org").await;
    let before = dispatch.status;
    let action_id = pending_action(org.id, dispatch.id);

    let req = test::TestRequest::post()
        .uri(&format!("/api/agent-actions/{action_id}/reject"))
        .insert_header(("Authorization", auth))
        .to_request();
    let rejected =
        test::read_body_json::<ApiResponse<AgentAction>, _>(test::call_service(&app, req).await)
            .await
            .data
            .unwrap();
    assert_eq!(rejected.status, ActionStatus::Rejected);
    assert_eq!(
        DispatchOrder::get_by_id(dispatch.id)
            .unwrap()
            .unwrap()
            .status,
        before
    );
}
