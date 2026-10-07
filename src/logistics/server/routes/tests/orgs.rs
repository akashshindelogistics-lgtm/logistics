use super::*;

#[actix_web::test]
async fn test_list_orgs_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api/orgs").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_get_org_own_org_returns_200() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Own Org Test".to_string(),
        address: "5 Auth Road".to_string(),
        password: "own_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}", org.id))
        .insert_header(("Authorization", make_auth_header(org.id, &org.name)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_get_org_different_org_returns_403() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Org A Forbidden".to_string(),
        address: "6 Auth Road".to_string(),
        password: "pass_a".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org_a = body.data.unwrap();

    // Use a different org_id in the token
    let different_org_id = Uuid::new_v4();
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}", org_a.id))
        .insert_header(("Authorization", make_auth_header(different_org_id, "Org B")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_create_org_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let payload = CreateOrgPayload {
        name: "API Test Express Org".to_string(),
        address: "100 Server Hub, Cyber City".to_string(),
        password: "test_password_123".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    assert!(body.success);
    let org = body.data.unwrap();
    assert_eq!(org.name, "API Test Express Org");
}

#[actix_web::test]
async fn test_create_org_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .insert_header(("Content-Type", "application/json"))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_update_org_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Update Org Test".to_string(),
        address: "Initial Address".to_string(),
        password: "update_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();
    let auth_header = make_auth_header(org.id, &org.name);

    let update_payload = UpdateOrgPayload {
        name: "Updated Org Name".to_string(),
        address: "456 Updated Ave, New City".to_string(),
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}", org.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&update_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.data.unwrap().name, "Updated Org Name");
}

#[actix_web::test]
async fn test_update_org_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}", org_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test Org")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_update_org_location_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Location Org".to_string(),
        address: "Loc Address".to_string(),
        password: "loc_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();
    let auth_header = make_auth_header(org.id, &org.name);

    let payload = LocationPayload {
        latitude: 28.6139,
        longitude: 77.2090,
        address: Some("New Delhi, India".to_string()),
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}/location", org.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Location> = test::read_body_json(resp).await;
    assert!(body.success);
    let loc = body.data.unwrap();
    assert_eq!(loc.latitude, 28.6139);
    assert_eq!(loc.longitude, 77.2090);
}

#[actix_web::test]
async fn test_delete_org_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Org To Delete".to_string(),
        address: "Delete Address".to_string(),
        password: "del_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();
    let auth_header = make_auth_header(org.id, &org.name);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/orgs/{}", org.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.message, "Organization deleted successfully");
}

#[actix_web::test]
async fn test_list_orgs_with_valid_token_returns_own_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "List Orgs Success Org".to_string(),
        address: "10 Success Rd".to_string(),
        password: "list_orgs_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();
    let auth_header = make_auth_header(org.id, &org.name);

    let req = test::TestRequest::get()
        .uri("/api/orgs")
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vec<Organization>> = test::read_body_json(resp).await;
    assert!(body.success);
    let orgs = body.data.unwrap();
    assert_eq!(orgs.len(), 1);
    assert_eq!(orgs[0].id, org.id);
}

#[actix_web::test]
async fn test_update_org_location_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}/location", org_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_update_org_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let target_org_id = Uuid::new_v4();
    let attacker_org_id = Uuid::new_v4();
    let payload = UpdateOrgPayload {
        name: "Hacked Name".to_string(),
        address: "Hacked Address".to_string(),
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}", target_org_id))
        .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_update_org_location_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let target_org_id = Uuid::new_v4();
    let attacker_org_id = Uuid::new_v4();
    let payload = LocationPayload {
        latitude: 0.0,
        longitude: 0.0,
        address: None,
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/orgs/{}/location", target_org_id))
        .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_delete_org_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let target_org_id = Uuid::new_v4();
    let attacker_org_id = Uuid::new_v4();
    let req = test::TestRequest::delete()
        .uri(&format!("/api/orgs/{}", target_org_id))
        .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_delete_org_requires_admin() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, admin) = setup_org(&app, "Delete Guard Org").await;
    let dispatcher = add_user_and_login(&app, org.id, &admin, "dd@example.com", OrgRole::Dispatcher).await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/orgs/{}", org.id))
        .insert_header(("Authorization", dispatcher))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}
