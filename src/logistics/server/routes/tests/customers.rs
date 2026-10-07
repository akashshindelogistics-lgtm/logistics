use super::*;

#[actix_web::test]
async fn test_list_customers_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api/customers").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_create_customer_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Customer API Org").await;

    let customer = create_customer_via_api(&app, org.id, &auth, "API Test Customer").await;
    assert_eq!(customer.name, "API Test Customer");
    assert_eq!(customer.org_id, org.id);
}

#[actix_web::test]
async fn test_create_customer_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _auth) = setup_org(&app, "Owner Cust Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .set_json(&CreateCustomerPayload {
            name: "Poached".to_string(),
            address: "x".to_string(),
            latitude: None,
            longitude: None,
            location_address: None,
            phone: None,
            email: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_create_customer_with_location_locates_it() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Cust Create Loc Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&CreateCustomerPayload {
            name: "Geo Retail".to_string(),
            address: "12 Dockyard Rd, Mumbai".to_string(),
            latitude: Some(19.0760),
            longitude: Some(72.8777),
            location_address: None,
            phone: None,
            email: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let customer = test::read_body_json::<ApiResponse<Customer>, _>(resp)
        .await
        .data
        .unwrap();

    let location = customer.location.expect("customer should be located on create");
    assert_eq!(location.latitude, 19.0760);
    assert_eq!(location.longitude, 72.8777);
    // With no explicit label the customer's address is used for the pin.
    assert_eq!(location.address.as_deref(), Some("12 Dockyard Rd, Mumbai"));

    // The location was persisted, not just echoed back.
    let fetched = Customer::get_by_id(customer.id).unwrap().unwrap();
    assert_eq!(fetched.location.map(|l| l.longitude), Some(72.8777));
}

#[actix_web::test]
async fn test_create_customer_without_coordinates_has_no_location() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Cust Create No Loc Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&CreateCustomerPayload {
            name: "Plain Co".to_string(),
            address: "no pin".to_string(),
            latitude: Some(19.0),
            longitude: None,
            location_address: None,
            phone: None,
            email: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let customer = test::read_body_json::<ApiResponse<Customer>, _>(resp)
        .await
        .data
        .unwrap();
    assert!(customer.location.is_none(), "a lone latitude must not locate the customer");
}

#[actix_web::test]
async fn test_create_customer_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_list_customers_is_org_scoped() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Cust Scope A").await;
    let (org_b, auth_b) = setup_org(&app, "Cust Scope B").await;

    create_customer_via_api(&app, org_a.id, &auth_a, "A One").await;
    create_customer_via_api(&app, org_a.id, &auth_a, "A Two").await;
    create_customer_via_api(&app, org_b.id, &auth_b, "B One").await;

    let req = test::TestRequest::get()
        .uri("/api/customers")
        .insert_header(("Authorization", auth_a))
        .to_request();
    let body: ApiResponse<Vec<Customer>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let customers = body.data.unwrap();
    assert_eq!(customers.len(), 2);
    assert!(customers.iter().all(|c| c.org_id == org_a.id));
}

#[actix_web::test]
async fn test_update_customer_location_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Cust Loc Org").await;
    let customer = create_customer_via_api(&app, org.id, &auth, "Located Co").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/customers/{}/location", customer.id))
        .insert_header(("Authorization", auth))
        .set_json(&LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Bandra West, Mumbai".to_string()),
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Location> = test::read_body_json(resp).await;
    let loc = body.data.unwrap();
    assert_eq!(loc.latitude, 19.0760);
    assert_eq!(loc.longitude, 72.8777);
}

#[actix_web::test]
async fn test_update_customer_location_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Cust Loc Owner").await;
    let customer = create_customer_via_api(&app, org.id, &auth, "Owned Co").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/customers/{}/location", customer.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .set_json(&LocationPayload {
            latitude: 1.0,
            longitude: 1.0,
            address: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_delete_customer_and_cross_org_guard() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Cust Del Org").await;
    let customer = create_customer_via_api(&app, org.id, &auth, "Deletable Co").await;

    // A different org cannot delete it.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/customers/{}", customer.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    // The owner can.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/customers/{}", customer.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri("/api/customers")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Customer>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert!(body.data.unwrap().is_empty());
}

#[actix_web::test]
async fn test_list_customers_with_valid_token_returns_200() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::get()
        .uri("/api/customers")
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vec<Customer>> = test::read_body_json(resp).await;
    assert!(body.success);
    assert!(body.data.is_some());
}

#[actix_web::test]
async fn test_update_customer_location_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let customer_id = Uuid::new_v4();
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::put()
        .uri(&format!("/api/customers/{}/location", customer_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_create_customer_with_contact_details() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Contact Cust Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&CreateCustomerPayload {
            name: "Reachable Retail".to_string(),
            address: "1 Contact Rd".to_string(),
            latitude: None,
            longitude: None,
            location_address: None,
            phone: Some("+91 90000 00000".to_string()),
            email: Some("ops@reachable.example".to_string()),
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let customer = test::read_body_json::<ApiResponse<Customer>, _>(resp).await.data.unwrap();
    assert_eq!(customer.phone.as_deref(), Some("+91 90000 00000"));
    assert_eq!(customer.email.as_deref(), Some("ops@reachable.example"));
}
