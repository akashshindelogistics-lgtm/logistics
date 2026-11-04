//! The chat routes over HTTP: privacy and setup. The conversation itself is
//! tested against a scripted model in `chat/tests.rs`.

use super::*;
use crate::logistics::auth::auth::generate_user_token;
use crate::logistics::chat::catalog;
use crate::logistics::chat::store::{self, NewProposal, Owner};
use serde_json::{Value, json};

fn owner_of(org_id: Uuid) -> Owner {
    Owner {
        org_id,
        user_id: None,
    }
}

fn member_auth(org: &Organization) -> (Uuid, String) {
    let user_id = Uuid::new_v4();
    let token = generate_user_token(org.id, &org.name, Some(user_id), "Dispatcher").unwrap();
    (user_id, format!("Bearer {token}"))
}

fn pending_card(conversation_id: Uuid) -> Uuid {
    store::create_proposal(NewProposal {
        conversation_id,
        tool_use_id: "toolu_1",
        operation_id: "add_customer",
        title: "Add customer",
        dangerous: false,
        method: "POST",
        uri: "/api/health",
        body: None,
        fields: &[],
        reason: "x",
    })
    .unwrap()
}

macro_rules! status_of {
    ($app:expr, $req:expr, $auth:expr, $uri:expr) => {{
        let req = $req
            .uri($uri)
            .insert_header(("Authorization", $auth.clone()))
            .to_request();
        test::call_service(&$app, req).await.status().as_u16()
    }};
}

#[actix_web::test]
async fn test_a_conversation_is_private_to_the_person_who_started_it() {
    let _db = TestDb::create();
    let org = Organization::create_organization("Chat Co", "1 Road").unwrap();
    let other = Organization::create_organization("Other Chat Co", "2 Road").unwrap();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let owner_auth = make_auth_header(org.id, &org.name);
    let (_, colleague_auth) = member_auth(&org);
    let outsider_auth = make_auth_header(other.id, &other.name);

    let req = test::TestRequest::post()
        .uri("/api/chat/conversations")
        .insert_header(("Authorization", owner_auth.clone()))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status().as_u16(), 201);
    let body: Value = test::read_body_json(response).await;
    let id = body["data"]["id"].as_str().unwrap().to_string();
    let uri = format!("/api/chat/conversations/{id}");
    let card = pending_card(Uuid::parse_str(&id).unwrap());
    let confirm = format!("/api/chat/proposals/{card}/confirm");
    let cancel = format!("/api/chat/proposals/{card}/cancel");

    assert_eq!(
        status_of!(app, test::TestRequest::get(), owner_auth, &uri),
        200
    );
    for auth in [&colleague_auth, &outsider_auth] {
        assert_eq!(status_of!(app, test::TestRequest::get(), auth, &uri), 404);
        assert_eq!(
            status_of!(app, test::TestRequest::delete(), auth, &uri),
            404
        );
        assert_eq!(
            status_of!(app, test::TestRequest::post(), auth, &confirm),
            404
        );
        assert_eq!(
            status_of!(app, test::TestRequest::post(), auth, &cancel),
            404
        );

        let req = test::TestRequest::get()
            .uri("/api/chat/conversations")
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(body["data"], json!([]));
    }
    assert_eq!(
        store::pending_count(Uuid::parse_str(&id).unwrap()).unwrap(),
        1
    );
}

#[actix_web::test]
async fn test_chat_needs_a_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get()
        .uri("/api/chat/conversations")
        .to_request();
    let response = test::try_call_service(&app, req).await;
    assert_eq!(response.map(|r| r.status().as_u16()).unwrap_or(401), 401);
}

#[actix_web::test]
async fn test_cancelling_a_card_answers_it_once() {
    let _db = TestDb::create();
    let org = Organization::create_organization("Cancel Route Co", "1 Road").unwrap();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let auth = make_auth_header(org.id, &org.name);
    let conversation = store::create_conversation(&owner_of(org.id)).unwrap();
    let card = pending_card(conversation.id);
    let uri = format!("/api/chat/proposals/{card}/cancel");

    assert_eq!(status_of!(app, test::TestRequest::post(), auth, &uri), 200);
    assert_eq!(status_of!(app, test::TestRequest::post(), auth, &uri), 409);
}

#[actix_web::test]
async fn test_deleting_a_conversation_removes_it_and_its_cards() {
    let _db = TestDb::create();
    let org = Organization::create_organization("Delete Route Co", "1 Road").unwrap();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let auth = make_auth_header(org.id, &org.name);
    let conversation = store::create_conversation(&owner_of(org.id)).unwrap();
    let card = pending_card(conversation.id);
    let uri = format!("/api/chat/conversations/{}", conversation.id);

    assert_eq!(
        status_of!(app, test::TestRequest::delete(), auth, &uri),
        200
    );
    assert_eq!(status_of!(app, test::TestRequest::get(), auth, &uri), 404);
    assert!(store::get_proposal(card).unwrap().is_none());
}

#[actix_web::test]
async fn test_invoke_runs_the_real_handlers_as_the_token_holder() {
    let _db = TestDb::create();
    let org = Organization::create_organization("Invoke Co", "1 Road").unwrap();
    let auth = make_auth_header(org.id, &org.name);

    let (status, _) = catalog::invoke("GET", "/api/customers", &auth, None)
        .await
        .unwrap();
    assert_eq!(status, 200);

    let (status, _) = catalog::invoke("GET", "/api/customers", "Bearer nonsense", None)
        .await
        .unwrap();
    assert_eq!(status, 401);
}
