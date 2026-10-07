use super::*;

/// Create an org (with credentials) and one godown under it, returning
/// `(org, godown, auth_header)`. Shared by the godown/stock route tests below.
async fn setup_org_with_godown(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_name: &str,
    godown_name: &str,
) -> (Organization, Godown, String) {
    let create_payload = CreateOrgPayload {
        name: org_name.to_string(),
        address: format!("1 {} Road", org_name),
        password: "godown_test_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let body: ApiResponse<Organization> =
        test::read_body_json(test::call_service(app, req).await).await;
    let org = body.data.unwrap();
    let auth_header = make_auth_header(org.id, &org.name);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&CreateGodownPayload {
            name: godown_name.to_string(),
            address: format!("Plot 1, {}", godown_name),
            max_capacity: None,
        })
        .to_request();
    let body: ApiResponse<Godown> =
        test::read_body_json(test::call_service(app, req).await).await;
    let godown = body.data.unwrap();

    (org, godown, auth_header)
}

/// Add a second godown to an org that already has `auth_header`, returning
/// the new godown. Used by the stock-transfer route tests.
async fn add_godown(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth_header: &str,
    name: &str,
    max_capacity: Option<i64>,
) -> Godown {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org_id))
        .insert_header(("Authorization", auth_header.to_string()))
        .set_json(&CreateGodownPayload {
            name: name.to_string(),
            address: format!("Plot 2, {}", name),
            max_capacity,
        })
        .to_request();
    let body: ApiResponse<Godown> =
        test::read_body_json(test::call_service(app, req).await).await;
    body.data.unwrap()
}

async fn seed_stock(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    godown_id: Uuid,
    auth_header: &str,
    description: &str,
    volume_in_size: i64,
    quantity: i64,
) {
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown_id))
        .insert_header(("Authorization", auth_header.to_string()))
        .set_json(&CreateStockPayload {
            volume_in_size,
            quantity,
            description: description.to_string(),
            category: "General".to_string(),
            reorder_threshold: None,
        })
        .to_request();
    assert_eq!(test::call_service(app, req).await.status().as_u16(), 201);
}

#[actix_web::test]
async fn test_create_godown_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_create_godown_for_nonexistent_org_returns_error() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let payload = CreateGodownPayload {
        name: "Ghost Godown".to_string(),
        address: "Nowhere".to_string(),
        max_capacity: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org_id))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 500);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
    assert!(body.message.contains("Failed to create godown"));
}

#[actix_web::test]
async fn test_list_godowns_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, godown, auth_header) =
        setup_org_with_godown(&app, "List Godowns Org", "List Godown").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vec<Godown>> = test::read_body_json(resp).await;
    assert!(body.success);
    let godowns = body.data.unwrap();
    assert_eq!(godowns.len(), 1);
    assert_eq!(godowns[0].id, godown.id);
}

#[actix_web::test]
async fn test_get_godown_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Get Godown Org", "Get Godown").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Godown> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.data.unwrap().id, godown.id);
}

#[actix_web::test]
async fn test_get_godown_not_found_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/godowns/{}", Uuid::new_v4()))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_get_godown_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, _auth_header) =
        setup_org_with_godown(&app, "Owner Org", "Owner Godown").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_update_godown_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Update Godown Org", "Old Godown Name").await;

    let payload = UpdateGodownPayload {
        name: "New Godown Name".to_string(),
        address: "New Godown Address".to_string(),
        max_capacity: None,
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Godown> = test::read_body_json(resp).await;
    assert!(body.success);
    let updated = body.data.unwrap();
    assert_eq!(updated.name, "New Godown Name");
    assert_eq!(updated.address, "New Godown Address");
}

#[actix_web::test]
async fn test_update_godown_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}", Uuid::new_v4()))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_update_godown_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, _auth_header) =
        setup_org_with_godown(&app, "Update Owner Org", "Update Owner Godown").await;

    let payload = UpdateGodownPayload {
        name: "Hacked Godown".to_string(),
        address: "Hacked Address".to_string(),
        max_capacity: None,
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_update_godown_location_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Godown Location Org", "Godown Location Warehouse").await;

    let payload = LocationPayload {
        latitude: 19.0760,
        longitude: 72.8777,
        address: Some("Mumbai, Maharashtra".to_string()),
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}/location", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Location> = test::read_body_json(resp).await;
    assert!(body.success);
    let loc = body.data.unwrap();
    assert_eq!(loc.latitude, 19.0760);
    assert_eq!(loc.longitude, 72.8777);
}

#[actix_web::test]
async fn test_update_godown_location_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}/location", Uuid::new_v4()))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_delete_godown_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, godown, auth_header) =
        setup_org_with_godown(&app, "Delete Godown Org", "Doomed Godown").await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", auth_header.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.message, "Godown deleted successfully");

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Vec<Godown>> = test::read_body_json(resp).await;
    assert!(body.data.unwrap().is_empty());
}

#[actix_web::test]
async fn test_delete_godown_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, _auth_header) =
        setup_org_with_godown(&app, "Delete Owner Org", "Delete Owner Godown").await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_add_godown_stock_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", Uuid::new_v4()))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_add_godown_stock_nonexistent_godown_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let payload = CreateStockPayload {
        volume_in_size: 50,
        quantity: 100,
        description: "Ghost Stock".to_string(),
        category: "General".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", Uuid::new_v4()))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_add_godown_stock_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, _auth_header) =
        setup_org_with_godown(&app, "Stock Owner Org", "Stock Owner Godown").await;

    let payload = CreateStockPayload {
        volume_in_size: 50,
        quantity: 100,
        description: "Stolen Goods".to_string(),
        category: "General".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_update_godown_stock_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Update Stock Org", "Update Stock Godown").await;

    let payload = UpdateStockPayload {
        volume_in_size: 200,
        quantity: 75,
        description: "Nonexistent Stock Description".to_string(),
        category: "General".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Stock> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.message, "Stock updated successfully");
}

#[actix_web::test]
async fn test_update_godown_stock_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}/stock", Uuid::new_v4()))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_update_godown_stock_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, _auth_header) =
        setup_org_with_godown(&app, "Update Stock Owner Org", "Update Stock Owner Godown").await;

    let payload = UpdateStockPayload {
        volume_in_size: 999,
        quantity: 999,
        description: "Tampered Stock".to_string(),
        category: "General".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_delete_godown_stock_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Delete Stock Org", "Delete Stock Godown").await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/godowns/{}/stock/nonexistent-description", godown.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.message, "Stock removed successfully");
}

#[actix_web::test]
async fn test_delete_godown_stock_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, _auth_header) =
        setup_org_with_godown(&app, "Delete Stock Owner Org", "Delete Stock Owner Godown").await;

    let req = test::TestRequest::delete()
        .uri(&format!("/api/godowns/{}/stock/some-item", godown.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_transfer_godown_stock_moves_units_and_records_the_move() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, from, auth_header) =
        setup_org_with_godown(&app, "Transfer Org", "Source Godown").await;
    let to = add_godown(&app, org.id, &auth_header, "Dest Godown", None).await;
    seed_stock(&app, from.id, &auth_header, "Cement", 5, 100).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/transfer", from.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&TransferStockPayload {
            to_godown_id: to.id,
            description: "Cement".to_string(),
            quantity: 40,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<StockTransfer> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.data.unwrap().quantity, 40);

    // The audit trail now lists the one transfer.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/stock-transfers", org.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let body: ApiResponse<Vec<StockTransfer>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let transfers = body.data.unwrap();
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers[0].from_godown_id, from.id);
    assert_eq!(transfers[0].to_godown_id, to.id);
    assert_eq!(transfers[0].description, "Cement");
}

#[actix_web::test]
async fn test_transfer_godown_stock_carries_the_sources_category_to_a_new_destination_row() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, from, auth_header) =
        setup_org_with_godown(&app, "Transfer Category Org", "Source Godown").await;
    let to = add_godown(&app, org.id, &auth_header, "Dest Godown", None).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", from.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&CreateStockPayload {
            volume_in_size: 5,
            quantity: 100,
            description: "Cement".to_string(),
            category: "Building Materials".to_string(),
            reorder_threshold: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/transfer", from.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&TransferStockPayload {
            to_godown_id: to.id,
            description: "Cement".to_string(),
            quantity: 40,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<StockTransfer> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().category, "Building Materials");

    let req = test::TestRequest::get()
        .uri(&format!("/api/godowns/{}", to.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let body: ApiResponse<Godown> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let dest_cement = body.data.unwrap().stock.into_iter().find(|s| s.description == "Cement").unwrap();
    assert_eq!(dest_cement.category, "Building Materials");
}

#[actix_web::test]
async fn test_transfer_godown_stock_rejects_insufficient_quantity_with_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, from, auth_header) =
        setup_org_with_godown(&app, "Short Transfer Org", "Source").await;
    let to = add_godown(&app, org.id, &auth_header, "Dest", None).await;
    seed_stock(&app, from.id, &auth_header, "Bricks", 2, 10).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/transfer", from.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&TransferStockPayload {
            to_godown_id: to.id,
            description: "Bricks".to_string(),
            quantity: 50,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_transfer_godown_stock_over_destination_capacity_returns_409() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, from, auth_header) =
        setup_org_with_godown(&app, "Cap Transfer Org", "Source").await;
    let to = add_godown(&app, org.id, &auth_header, "Tiny Dest", Some(100)).await;
    seed_stock(&app, from.id, &auth_header, "Tiles", 10, 50).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/transfer", from.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&TransferStockPayload {
            to_godown_id: to.id,
            description: "Tiles".to_string(),
            quantity: 11, // 11 * 10 = 110 > 100
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 409);
}

#[actix_web::test]
async fn test_transfer_godown_stock_returns_403_for_a_foreign_destination() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org_a, from, auth_a) =
        setup_org_with_godown(&app, "Owner Org", "Owner Source").await;
    let (_org_b, foreign, _auth_b) =
        setup_org_with_godown(&app, "Other Org", "Other Godown").await;
    seed_stock(&app, from.id, &auth_a, "Sacks", 1, 20).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/transfer", from.id))
        .insert_header(("Authorization", auth_a))
        .set_json(&TransferStockPayload {
            to_godown_id: foreign.id,
            description: "Sacks".to_string(),
            quantity: 5,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_list_stock_transfers_returns_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _from, _auth) =
        setup_org_with_godown(&app, "Transfers Owner Org", "G").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/stock-transfers", org.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_add_stock_to_own_godown_returns_201() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Stock Test Org", "Stock Test Godown").await;

    let stock_payload = CreateStockPayload {
        volume_in_size: 100,
        quantity: 500,
        description: "Test Widget".to_string(),
        category: "General".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&stock_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Stock> = test::read_body_json(resp).await;
    assert!(body.success);
    let stock = body.data.unwrap();
    assert_eq!(stock.description, "Test Widget");
    assert_eq!(stock.quantity, 500);
}

#[actix_web::test]
async fn test_add_godown_stock_persists_an_explicit_category() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Category Test Org", "Category Test Godown").await;

    let stock_payload = CreateStockPayload {
        volume_in_size: 10,
        quantity: 20,
        description: "Server Rack".to_string(),
        category: "Electronics".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&stock_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Stock> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().category, "Electronics");
}

#[actix_web::test]
async fn test_add_godown_stock_defaults_category_when_omitted_from_the_json_body() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Default Category Org", "Default Category Godown").await;

    // Send raw JSON without a `category` field at all, exercising the
    // `#[serde(default = ...)]` fallback (not just a struct literal that
    // happens to set it to "General").
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&serde_json::json!({
            "volume_in_size": 5,
            "quantity": 10,
            "description": "Unlabeled Widget"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Stock> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().category, "General");
}

#[actix_web::test]
async fn test_update_godown_stock_changes_category() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Update Category Org", "Update Category Godown").await;

    Stock::new(5, 40, "Bolts").add_to_godown(godown.id).expect("seed stock");

    let payload = UpdateStockPayload {
        volume_in_size: 5,
        quantity: 40,
        description: "Bolts".to_string(),
        category: "Hardware".to_string(),
        reorder_threshold: None,
    };
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Stock> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().category, "Hardware");
}

#[actix_web::test]
async fn test_create_godown_persists_max_capacity() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Capacity Org".to_string(),
        address: "1 Capacity Road".to_string(),
        password: "pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let org: ApiResponse<Organization> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let org = org.data.unwrap();
    let auth = make_auth_header(org.id, &org.name);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&CreateGodownPayload {
            name: "Capped".to_string(),
            address: "Bay 1".to_string(),
            max_capacity: Some(2_000),
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Godown> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().max_capacity, Some(2_000));
}

#[actix_web::test]
async fn test_add_godown_stock_over_capacity_returns_409() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Overfill Org", "Small Godown").await;

    // Give the godown a tight cap.
    let req = test::TestRequest::put()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&UpdateGodownPayload {
            name: godown.name.clone(),
            address: godown.address.clone(),
            max_capacity: Some(1_000),
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    // 20 * 60 = 1200 > 1000 -> rejected.
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&CreateStockPayload {
            volume_in_size: 20,
            quantity: 60,
            description: "Bulky Crates".to_string(),
            category: "General".to_string(),
            reorder_threshold: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 409);

    // 20 * 40 = 800 <= 1000 -> accepted.
    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header))
        .set_json(&CreateStockPayload {
            volume_in_size: 20,
            quantity: 40,
            description: "Bulky Crates".to_string(),
            category: "General".to_string(),
            reorder_threshold: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);
}

#[actix_web::test]
async fn test_add_godown_stock_reports_below_threshold() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, godown, auth_header) =
        setup_org_with_godown(&app, "Reorder Org", "Reorder Godown").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&CreateStockPayload {
            volume_in_size: 1,
            quantity: 8,
            description: "Label Rolls".to_string(),
            category: "General".to_string(),
            reorder_threshold: Some(25),
        })
        .to_request();
    let body: ApiResponse<Stock> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let stock = body.data.unwrap();
    assert_eq!(stock.reorder_threshold, Some(25));
    assert!(stock.below_threshold);

    // And it survives a reload of the godown.
    let req = test::TestRequest::get()
        .uri(&format!("/api/godowns/{}", godown.id))
        .insert_header(("Authorization", auth_header))
        .to_request();
    let body: ApiResponse<Godown> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let reloaded = body.data.unwrap();
    let item = reloaded
        .stock
        .iter()
        .find(|s| s.description == "Label Rolls")
        .unwrap();
    assert!(item.below_threshold);
}

#[actix_web::test]
async fn test_create_godown_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let target_org_id = Uuid::new_v4();
    let attacker_org_id = Uuid::new_v4();
    let payload = CreateGodownPayload {
        name: "Stolen Godown".to_string(),
        address: "Stolen Address".to_string(),
        max_capacity: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", target_org_id))
        .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_warehouse_staff_can_add_stock_but_dispatcher_cannot() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, godown, admin) = setup_org_with_godown(&app, "Role Stock Org", "Shed").await;

    let dispatcher = add_user_and_login(&app, org.id, &admin, "d2@example.com", OrgRole::Dispatcher).await;
    let warehouse = add_user_and_login(&app, org.id, &admin, "w2@example.com", OrgRole::WarehouseStaff).await;

    let stock = CreateStockPayload {
        description: "Pallets".to_string(),
        category: "General".to_string(),
        quantity: 10,
        volume_in_size: 1,
        reorder_threshold: None,
    };

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", dispatcher))
        .set_json(&stock)
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", warehouse))
        .set_json(&stock)
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);
}
