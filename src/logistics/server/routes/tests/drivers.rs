use super::*;

async fn create_driver_via_api(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    name: &str,
) -> Driver {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{org_id}/drivers"))
        .insert_header(("Authorization", auth.to_string()))
        .set_json(&CreateDriverPayload {
            name: name.to_string(),
            license_number: format!("LIC-{name}"),
            phone: "+91 90000 00000".to_string(),
        })
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    test::read_body_json::<ApiResponse<Driver>, _>(resp)
        .await
        .data
        .unwrap()
}

#[actix_web::test]
async fn test_driver_crud_and_listing() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Fleet Ops").await;

    let driver = create_driver_via_api(&app, org.id, &auth, "Ravi").await;
    assert!(driver.is_active);

    // Update: rename + deactivate.
    let req = test::TestRequest::put()
        .uri(&format!("/api/drivers/{}", driver.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateDriverPayload {
            name: "Ravi Kumar".to_string(),
            license_number: "LIC-Ravi".to_string(),
            phone: "+91 90000 00001".to_string(),
            is_active: false,
        })
        .to_request();
    let body: ApiResponse<Driver> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.as_ref().unwrap().name, "Ravi Kumar");
    assert!(!body.data.unwrap().is_active);

    // List.
    let req = test::TestRequest::get()
        .uri("/api/drivers")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let body: ApiResponse<Vec<Driver>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().len(), 1);

    // Delete.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/drivers/{}", driver.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri("/api/drivers")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Driver>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert!(body.data.unwrap().is_empty());
}

#[actix_web::test]
async fn test_update_driver_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Owner Org").await;
    let driver = create_driver_via_api(&app, org.id, &auth, "Owned").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/drivers/{}", driver.id))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
        .set_json(&UpdateDriverPayload {
            name: "Hijacked".to_string(),
            license_number: "X".to_string(),
            phone: "0".to_string(),
            is_active: true,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_assign_vehicle_driver_rejects_foreign_driver() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Org A").await;
    let (org_b, auth_b) = setup_org(&app, "Org B").await;

    // Org A gets a vehicle.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org_a.id))
        .insert_header(("Authorization", auth_a.clone()))
        .set_json(&CreateVehiclePayload {
            registration_number: "A-VH-1".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

    // Org B's driver can't be assigned to org A's vehicle.
    let foreign = create_driver_via_api(&app, org_b.id, &auth_b, "Bee").await;
    let req = test::TestRequest::put()
        .uri("/api/vehicles/A-VH-1/driver")
        .insert_header(("Authorization", auth_a.clone()))
        .set_json(&AssignDriverPayload { driver_id: Some(foreign.id) })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);

    // Org A's own driver assigns fine, and clearing works.
    let own = create_driver_via_api(&app, org_a.id, &auth_a, "Ay").await;
    let req = test::TestRequest::put()
        .uri("/api/vehicles/A-VH-1/driver")
        .insert_header(("Authorization", auth_a.clone()))
        .set_json(&AssignDriverPayload { driver_id: Some(own.id) })
        .to_request();
    let body: ApiResponse<Vehicle> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().assigned_driver_id, Some(own.id));

    let req = test::TestRequest::put()
        .uri("/api/vehicles/A-VH-1/driver")
        .insert_header(("Authorization", auth_a))
        .set_json(&AssignDriverPayload { driver_id: None })
        .to_request();
    let body: ApiResponse<Vehicle> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().assigned_driver_id, None);
}
