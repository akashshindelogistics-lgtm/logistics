use super::*;
use super::vehicles::register_vehicle;
use crate::logistics::auth::auth::generate_user_token;

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

// ── Driver phone tracking ─────────────────────────────────────────────────

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn fix(latitude: f64, longitude: f64, recorded_at: i64) -> DriverLocationFix {
    DriverLocationFix { latitude, longitude, recorded_at, accuracy_m: Some(8.0), speed_mps: Some(5.0) }
}

/// Create a driver, optionally assign it to a fresh vehicle, and pair a
/// phone. Returns `(driver_id, device_token)`.
async fn pair_driver_phone(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    vehicle_reg: Option<&str>,
) -> (Uuid, Uuid) {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/drivers", org_id))
        .insert_header(("Authorization", auth.to_string()))
        .set_json(&CreateDriverPayload {
            name: "Phone Driver".to_string(),
            license_number: "DL-PH-1".to_string(),
            phone: "+91 90000 00001".to_string(),
        })
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let driver_id = test::read_body_json::<ApiResponse<Driver>, _>(resp).await.data.unwrap().id;

    if let Some(reg) = vehicle_reg {
        register_vehicle(app, org_id, auth, reg).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/vehicles/{}/driver", reg))
            .insert_header(("Authorization", auth.to_string()))
            .set_json(&AssignDriverPayload { driver_id: Some(driver_id) })
            .to_request();
        assert_eq!(test::call_service(app, req).await.status().as_u16(), 200);
    }

    let req = test::TestRequest::post()
        .uri(&format!("/api/drivers/{}/device-token/rotate", driver_id))
        .insert_header(("Authorization", auth.to_string()))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let token = test::read_body_json::<ApiResponse<DriverDeviceToken>, _>(resp)
        .await
        .data
        .unwrap()
        .device_token;
    (driver_id, token)
}

async fn post_driver_fixes(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    token: Option<&str>,
    fixes: Vec<DriverLocationFix>,
) -> actix_web::dev::ServiceResponse {
    let mut req = test::TestRequest::post()
        .uri("/api/driver/location")
        .set_json(&DriverLocationPayload { fixes });
    if let Some(t) = token {
        req = req.insert_header(("Authorization", format!("Bearer {}", t)));
    }
    test::call_service(app, req.to_request()).await
}

#[actix_web::test]
async fn test_driver_location_moves_the_assigned_vehicle_using_only_the_device_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Track Org").await;
    let (_driver, token) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-1")).await;

    let t = unix_now() - 30;
    let resp = post_driver_fixes(&app, Some(&token.to_string()), vec![fix(18.5204, 73.8567, t)]).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<DriverLocationResult> = test::read_body_json(resp).await;
    let result = body.data.unwrap();
    assert_eq!(result.accepted, 1);
    assert!(result.location_updated);
    assert_eq!(result.vehicle_registration_number, "PH-VH-1");

    // Visible on the org's fleet list, stamped with the phone's capture time.
    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Vehicle>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let loc = body.data.unwrap().into_iter().find(|v| v.registration_number == "PH-VH-1").unwrap().location.unwrap();
    assert_eq!(loc.latitude, 18.5204);
    assert_eq!(loc.timestamp, t);
}

#[actix_web::test]
async fn test_driver_location_batch_applies_only_the_newest_fix() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Batch Org").await;
    let (_d, token) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-2")).await;

    let now = unix_now();
    // Deliberately out of order inside the batch.
    let resp = post_driver_fixes(
        &app,
        Some(&token.to_string()),
        vec![fix(3.0, 3.0, now - 10), fix(1.0, 1.0, now - 50), fix(2.0, 2.0, now - 30)],
    )
    .await;
    assert_eq!(resp.status().as_u16(), 200);
    let result = test::read_body_json::<ApiResponse<DriverLocationResult>, _>(resp).await.data.unwrap();
    assert_eq!(result.accepted, 3);
    let loc = result.location.unwrap();
    assert_eq!((loc.latitude, loc.timestamp), (3.0, now - 10));
}

#[actix_web::test]
async fn test_driver_location_late_upload_cannot_move_the_vehicle_backwards() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Late Org").await;
    let (_d, token) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-3")).await;
    let token = token.to_string();

    let now = unix_now();
    let resp = post_driver_fixes(&app, Some(&token), vec![fix(10.0, 10.0, now - 10)]).await;
    assert_eq!(resp.status().as_u16(), 200);

    // A queued, older batch arrives afterwards.
    let resp = post_driver_fixes(&app, Some(&token), vec![fix(49.0, 20.0, now - 600)]).await;
    assert_eq!(resp.status().as_u16(), 200);
    let result = test::read_body_json::<ApiResponse<DriverLocationResult>, _>(resp).await.data.unwrap();
    assert!(!result.location_updated);
    let loc = result.location.unwrap();
    assert_eq!((loc.latitude, loc.longitude), (10.0, 10.0));
}

#[actix_web::test]
async fn test_driver_location_without_a_token_is_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let resp = post_driver_fixes(&app, None, vec![fix(1.0, 1.0, unix_now())]).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_driver_location_with_unknown_or_malformed_token_is_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let unknown = Uuid::new_v4().to_string();
    assert_eq!(post_driver_fixes(&app, Some(&unknown), vec![fix(1.0, 1.0, unix_now())]).await.status().as_u16(), 401);
    assert_eq!(post_driver_fixes(&app, Some("not-a-uuid"), vec![fix(1.0, 1.0, unix_now())]).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_driver_location_rejects_an_org_bearer_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Org Token Org").await;
    pair_driver_phone(&app, org.id, &auth, Some("PH-VH-4")).await;

    // An org JWT is not a device token.
    let jwt = auth.trim_start_matches("Bearer ").to_string();
    assert_eq!(post_driver_fixes(&app, Some(&jwt), vec![fix(1.0, 1.0, unix_now())]).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_driver_location_rotating_the_token_kills_the_old_one() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Rotate Org").await;
    let (driver_id, old) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-5")).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/drivers/{}/device-token/rotate", driver_id))
        .insert_header(("Authorization", auth))
        .to_request();
    let new = test::read_body_json::<ApiResponse<DriverDeviceToken>, _>(test::call_service(&app, req).await)
        .await
        .data
        .unwrap()
        .device_token;
    assert_ne!(old, new);

    let t = unix_now();
    assert_eq!(post_driver_fixes(&app, Some(&old.to_string()), vec![fix(1.0, 1.0, t)]).await.status().as_u16(), 401);
    assert_eq!(post_driver_fixes(&app, Some(&new.to_string()), vec![fix(1.0, 1.0, t)]).await.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_driver_location_from_an_inactive_driver_is_403() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Inactive Org").await;
    let (driver_id, token) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-6")).await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/drivers/{}", driver_id))
        .insert_header(("Authorization", auth))
        .set_json(&UpdateDriverPayload {
            name: "Phone Driver".to_string(),
            license_number: "DL-PH-1".to_string(),
            phone: "+91 90000 00001".to_string(),
            is_active: false,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    assert_eq!(post_driver_fixes(&app, Some(&token.to_string()), vec![fix(1.0, 1.0, unix_now())]).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_driver_location_with_no_assigned_vehicle_is_409() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Unassigned Org").await;
    let (_d, token) = pair_driver_phone(&app, org.id, &auth, None).await;

    assert_eq!(post_driver_fixes(&app, Some(&token.to_string()), vec![fix(1.0, 1.0, unix_now())]).await.status().as_u16(), 409);
}

#[actix_web::test]
async fn test_driver_location_validates_the_batch() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Validate Org").await;
    let (_d, token) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-7")).await;
    let token = token.to_string();
    let now = unix_now();

    // Empty batch.
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![]).await.status().as_u16(), 400);
    // Oversized batch.
    let big: Vec<_> = (0..(MAX_DRIVER_FIXES_PER_REQUEST as i64 + 1)).map(|i| fix(1.0, 1.0, now - i)).collect();
    assert_eq!(post_driver_fixes(&app, Some(&token), big).await.status().as_u16(), 400);
    // Out-of-range coordinates.
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![fix(120.0, 10.0, now)]).await.status().as_u16(), 400);
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![fix(10.0, 190.0, now)]).await.status().as_u16(), 400);
    // Non-positive and far-future timestamps.
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![fix(1.0, 1.0, 0)]).await.status().as_u16(), 400);
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![fix(1.0, 1.0, now + 3_600)]).await.status().as_u16(), 400);
    // Negative accuracy / speed.
    let mut bad = fix(1.0, 1.0, now);
    bad.accuracy_m = Some(-1.0);
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![bad]).await.status().as_u16(), 400);
    let mut bad = fix(1.0, 1.0, now);
    bad.speed_mps = Some(-5.0);
    assert_eq!(post_driver_fixes(&app, Some(&token), vec![bad]).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_driver_location_one_bad_fix_rejects_the_whole_batch() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Phone Atomic Org").await;
    let (_d, token) = pair_driver_phone(&app, org.id, &auth, Some("PH-VH-8")).await;

    let now = unix_now();
    let resp = post_driver_fixes(
        &app,
        Some(&token.to_string()),
        vec![fix(5.0, 5.0, now - 5), fix(500.0, 5.0, now - 1)],
    )
    .await;
    assert_eq!(resp.status().as_u16(), 400);

    // The good fix in the rejected batch did not move the vehicle.
    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Vehicle>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let v = body.data.unwrap().into_iter().find(|v| v.registration_number == "PH-VH-8").unwrap();
    assert!(v.location.is_none());
}

#[actix_web::test]
async fn test_driver_location_does_not_touch_another_orgs_vehicle() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Phone Tenant A").await;
    let (org_b, auth_b) = setup_org(&app, "Phone Tenant B").await;
    let (_da, token_a) = pair_driver_phone(&app, org_a.id, &auth_a, Some("PH-A-1")).await;
    pair_driver_phone(&app, org_b.id, &auth_b, Some("PH-B-1")).await;

    assert_eq!(post_driver_fixes(&app, Some(&token_a.to_string()), vec![fix(7.0, 7.0, unix_now() - 5)]).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth_b))
        .to_request();
    let body: ApiResponse<Vec<Vehicle>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let v = body.data.unwrap().into_iter().find(|v| v.registration_number == "PH-B-1").unwrap();
    assert!(v.location.is_none(), "org B's vehicle is untouched by org A's driver");
}

#[actix_web::test]
async fn test_rotate_driver_device_token_from_another_org_returns_403() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Token Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Token Attacker Org").await;
    let (driver_id, _t) = pair_driver_phone(&app, org.id, &auth, None).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/drivers/{}/device-token/rotate", driver_id))
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_rotate_driver_device_token_unknown_driver_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, auth) = setup_org(&app, "Token Missing Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/drivers/{}/device-token/rotate", Uuid::new_v4()))
        .insert_header(("Authorization", auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_rotate_driver_device_token_requires_admin_or_dispatcher() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Token Role Org").await;
    let (driver_id, _t) = pair_driver_phone(&app, org.id, &auth, None).await;

    let staff_token = generate_user_token(
        org.id,
        &org.name,
        Some(Uuid::new_v4()),
        OrgRole::WarehouseStaff.as_str(),
    )
    .expect("user token");
    let req = test::TestRequest::post()
        .uri(&format!("/api/drivers/{}/device-token/rotate", driver_id))
        .insert_header(("Authorization", format!("Bearer {}", staff_token)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_driver_responses_never_contain_the_device_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Token Leak Org").await;
    let (_d, token) = pair_driver_phone(&app, org.id, &auth, None).await;

    let req = test::TestRequest::get()
        .uri("/api/drivers")
        .insert_header(("Authorization", auth))
        .to_request();
    let bytes = test::read_body(test::call_service(&app, req).await).await;
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!text.contains(&token.to_string()));
    assert!(!text.contains("device_token"));
}
