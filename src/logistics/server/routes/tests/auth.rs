use super::*;

#[actix_web::test]
async fn test_auth_login_with_valid_credentials_returns_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Login Test Org".to_string(),
        address: "1 Auth Road".to_string(),
        password: "login_pass_123".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org_id = body.data.unwrap().id;

    let login_payload = LoginPayload {
        org_id,
        password: "login_pass_123".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(&login_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<LoginData> = test::read_body_json(resp).await;
    assert!(body.success);
    assert!(!body.data.unwrap().token.is_empty());
}

#[actix_web::test]
async fn test_auth_login_with_wrong_password_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Wrong Pass Org".to_string(),
        address: "2 Auth Road".to_string(),
        password: "correct_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org_id = body.data.unwrap().id;

    let login_payload = LoginPayload {
        org_id,
        password: "wrong_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(&login_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_auth_login_with_nonexistent_org_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let login_payload = LoginPayload {
        org_id: Uuid::new_v4(),
        password: "any_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(&login_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_auth_login_invalid_payload_returns_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .insert_header(("Content-Type", "application/json"))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_auth_orgs_returns_list_without_auth() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    // First register an org so the list is not empty
    let create_payload = CreateOrgPayload {
        name: "List Orgs Org".to_string(),
        address: "3 Auth Road".to_string(),
        password: "list_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get().uri("/api/auth/orgs").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vec<OrgSummary>> = test::read_body_json(resp).await;
    assert!(body.success);
    assert!(body.data.unwrap().len() >= 1);
}

#[actix_web::test]
async fn test_auth_me_with_valid_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Me Endpoint Org".to_string(),
        address: "4 Auth Road".to_string(),
        password: "me_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();

    let req = test::TestRequest::get()
        .uri("/api/auth/me")
        .insert_header(("Authorization", make_auth_header(org.id, &org.name)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.data.unwrap().id, org.id);
}

#[actix_web::test]
async fn test_auth_me_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api/auth/me").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_user_login_rejects_wrong_password_and_inactive_account() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, admin) = setup_org(&app, "User Login Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/users", org.id))
        .insert_header(("Authorization", admin.clone()))
        .set_json(&CreateUserPayload {
            name: "Dana".to_string(),
            email: "dana@example.com".to_string(),
            password: "dana-password".to_string(),
            role: OrgRole::Dispatcher,
        })
        .to_request();
    let user = test::read_body_json::<ApiResponse<OrgUser>, _>(test::call_service(&app, req).await)
        .await
        .data
        .unwrap();

    let bad = test::TestRequest::post()
        .uri("/api/auth/user-login")
        .set_json(&UserLoginPayload { email: "dana@example.com".into(), password: "nope".into() })
        .to_request();
    assert_eq!(test::call_service(&app, bad).await.status().as_u16(), 401);

    // Deactivate, then a correct password still fails.
    let req = test::TestRequest::put()
        .uri(&format!("/api/users/{}", user.id))
        .insert_header(("Authorization", admin))
        .set_json(&UpdateUserPayload { name: "Dana".into(), role: OrgRole::Dispatcher, is_active: false })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let after = test::TestRequest::post()
        .uri("/api/auth/user-login")
        .set_json(&UserLoginPayload { email: "dana@example.com".into(), password: "dana-password".into() })
        .to_request();
    assert_eq!(test::call_service(&app, after).await.status().as_u16(), 401);
}
