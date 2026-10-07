use super::*;

#[actix_web::test]
async fn test_vendor_crud_and_listing() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Vendor Ops").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&vendor_payload("Sharma Roadlines"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<VehicleVendor> = test::read_body_json(resp).await;
    let vendor = body.data.expect("vendor");
    assert!(vendor.is_active);

    let req = test::TestRequest::put()
        .uri(&format!("/api/vendors/{}", vendor.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateVendorPayload {
            name: "Sharma Roadlines Pvt".to_string(),
            contact_person: None,
            phone: "+91 98200 11111".to_string(),
            gstin: None,
            notes: Some("Mumbai-Pune lane".to_string()),
            is_active: false,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<VehicleVendor> = test::read_body_json(resp).await;
    let updated = body.data.expect("vendor");
    assert_eq!(updated.name, "Sharma Roadlines Pvt");
    assert_eq!(updated.gstin, None);
    assert!(!updated.is_active);

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let body: ApiResponse<Vec<VehicleVendor>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().len(), 1);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/vendors/{}", vendor.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<VehicleVendor>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert!(body.data.unwrap().is_empty());
}

#[actix_web::test]
async fn test_add_vendor_rejects_invalid_input_with_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Vendor Validation").await;

    for payload in [
        CreateVendorPayload { name: "  ".to_string(), ..vendor_payload("x") },
        CreateVendorPayload { phone: "".to_string(), ..vendor_payload("No Phone") },
        CreateVendorPayload { gstin: Some("123".to_string()), ..vendor_payload("Bad GSTIN") },
    ] {
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vendors", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&payload)
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
    }
}

#[actix_web::test]
async fn test_vendor_routes_reject_other_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Vendor Owner").await;
    let other = make_auth_header(Uuid::new_v4(), "Attacker");

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&vendor_payload("Owned Vendor"))
        .to_request();
    let body: ApiResponse<VehicleVendor> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let vendor = body.data.expect("vendor");

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", other.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let req = test::TestRequest::delete()
        .uri(&format!("/api/vendors/{}", vendor.id))
        .insert_header(("Authorization", other))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_warehouse_staff_cannot_manage_vendors_but_dispatcher_can() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, admin) = setup_org(&app, "Vendor Roles").await;
    let dispatcher =
        add_user_and_login(&app, org.id, &admin, "vd@example.com", OrgRole::Dispatcher).await;
    let warehouse =
        add_user_and_login(&app, org.id, &admin, "vw@example.com", OrgRole::WarehouseStaff).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", warehouse.clone()))
        .set_json(&vendor_payload("Not Allowed"))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", dispatcher))
        .set_json(&vendor_payload("Dispatcher Vendor"))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

    // Every role can still read the vendor list.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", warehouse))
        .to_request();
    let body: ApiResponse<Vec<VehicleVendor>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().len(), 1);
}
