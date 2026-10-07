use super::*;

#[actix_web::test]
async fn test_get_dispatch_summary_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/dispatches/{}/summary", Uuid::new_v4()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_get_dispatch_summary_nonexistent_dispatch_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::get()
        .uri(&format!("/api/dispatches/{}/summary", Uuid::new_v4()))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
    assert!(body.message.contains("not found"));
}

#[actix_web::test]
async fn test_get_dispatch_summary_different_org_returns_403() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, _auth) = setup_dispatch(&app, "Summary Source Org").await;

    // Request summary with a different org's token — must be 403
    let req = test::TestRequest::get()
        .uri(&format!("/api/dispatches/{}/summary", dispatch.id))
        .insert_header((
            "Authorization",
            make_auth_header(Uuid::new_v4(), "Attacker"),
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_ask_assistant_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/ask", Uuid::new_v4()))
        .set_json(&AssistantAskPayload { question: "anything?".to_string() })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_ask_assistant_returns_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _auth) = setup_org(&app, "Assistant Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Assistant Intruder Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/ask", org.id))
        .insert_header(("Authorization", other_auth))
        .set_json(&AssistantAskPayload { question: "anything?".to_string() })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_ask_assistant_rejects_an_empty_question_with_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Assistant Empty Question Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/ask", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&AssistantAskPayload { question: "   ".to_string() })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_ask_assistant_returns_a_canned_answer_when_nothing_is_indexed() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Assistant Empty Index Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/ask", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&AssistantAskPayload { question: "what happened to my orders?".to_string() })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<AssistantAnswer> = test::read_body_json(resp).await;
    let answer = body.data.unwrap();
    assert!(answer.sources.is_empty());
    assert!(answer.answer.to_lowercase().contains("indexed yet"));
}

#[actix_web::test]
async fn test_reindex_assistant_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/reindex", Uuid::new_v4()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_reindex_assistant_returns_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _auth) = setup_org(&app, "Reindex Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Reindex Intruder Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/reindex", org.id))
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_reindex_assistant_rebuilds_the_index_and_reports_the_count() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Reindex Backfill Org").await;
    Customer::create_customer(org.id, "Legacy Customer", "1 Old Rd").expect("create customer");

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/assistant/reindex", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<AssistantReindexResult> = test::read_body_json(resp).await;
    assert!(body.data.unwrap().chunks_indexed >= 1);
}

#[actix_web::test]
async fn test_get_daily_digest_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/assistant/digest", Uuid::new_v4()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_get_daily_digest_returns_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _auth) = setup_org(&app, "Digest Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Digest Intruder Org").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/assistant/digest", org.id))
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_get_daily_digest_returns_a_canned_answer_when_nothing_needs_attention() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Digest Quiet Org").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/assistant/digest", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.data.unwrap().contains("Nothing needs your attention"));
}

#[actix_web::test]
async fn test_get_reorder_suggestions_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/assistant/reorder-suggestions", Uuid::new_v4()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_get_reorder_suggestions_returns_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _auth) = setup_org(&app, "Reorder Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Reorder Intruder Org").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/assistant/reorder-suggestions", org.id))
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_get_reorder_suggestions_returns_a_canned_answer_when_nothing_is_low() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Reorder Quiet Org").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/assistant/reorder-suggestions", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.data.unwrap().contains("No godowns are below their reorder threshold"));
}
