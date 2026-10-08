use super::*;

/// Register a vehicle under `org` via the API and return its full
/// response body (so the caller has the server-issued `tracker_key`).
pub(super) async fn register_vehicle(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    reg: &str,
) -> Vehicle {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org_id))
        .insert_header(("Authorization", auth.to_string()))
        .set_json(&CreateVehiclePayload {
            registration_number: reg.to_string(),
            capacity: 50,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    test::read_body_json::<ApiResponse<Vehicle>, _>(resp).await.data.unwrap()
}

#[actix_web::test]
async fn test_list_vehicles_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api/vehicles").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_list_vehicles_scoped_to_authenticated_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let create_payload = CreateOrgPayload {
        name: "Vehicle Scope Org".to_string(),
        address: "7 Fleet Road".to_string(),
        password: "fleet_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: ApiResponse<Organization> = test::read_body_json(resp).await;
    let org = body.data.unwrap();
    let auth_header = make_auth_header(org.id, &org.name);

    let add_vehicle_payload = CreateVehiclePayload {
        registration_number: "SCOPE-VH-001".to_string(),
        capacity: 20,
        unit: "MetricTon".to_string(),
        vehicle_type: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth_header.clone()))
        .set_json(&add_vehicle_payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);

    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth_header))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vec<Vehicle>> = test::read_body_json(resp).await;
    assert!(body.success);
    let vehicles = body.data.unwrap();
    assert!(vehicles.iter().any(|v| v.registration_number == "SCOPE-VH-001"));
}

#[actix_web::test]
async fn test_add_vehicle_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_add_vehicle_to_nonexistent_org_returns_error() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let payload = CreateVehiclePayload {
        registration_number: "ZZ01 XX 0001".to_string(),
        capacity: 10,
        unit: "MetricTon".to_string(),
        vehicle_type: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org_id))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 500);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
    assert!(body.message.contains("Failed to register vehicle"));
}

#[actix_web::test]
async fn test_update_vehicle_location_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let payload = LocationPayload {
        latitude: 19.0760,
        longitude: 72.8777,
        address: Some("Mumbai, Maharashtra".to_string()),
    };
    let req = test::TestRequest::put()
        .uri("/api/vehicles/NONEXISTENT-REG-001/location")
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
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
async fn test_update_vehicle_location_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::put()
        .uri("/api/vehicles/MH12EN3502/location")
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_delete_vehicle_endpoint() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::delete()
        .uri("/api/vehicles/NONEXISTENT-REG-002")
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.message, "Vehicle deleted successfully");
}

#[actix_web::test]
async fn test_edit_vehicle_updates_capacity_and_unit() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Edit Vehicle Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateVehiclePayload {
            registration_number: "EDIT-VH-1".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

    let req = test::TestRequest::put()
        .uri("/api/vehicles/EDIT-VH-1")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateVehiclePayload { capacity: 42, unit: "Box".to_string(), vehicle_type: None })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vehicle> = test::read_body_json(resp).await;
    let v = body.data.unwrap();
    assert_eq!(v.capacity, 42);
    assert_eq!(v.unit, Unit::Box);

    // Persisted.
    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Vehicle>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let stored = body.data.unwrap().into_iter().find(|v| v.registration_number == "EDIT-VH-1").unwrap();
    assert_eq!(stored.capacity, 42);
    assert_eq!(stored.unit, Unit::Box);
}

#[actix_web::test]
async fn test_edit_vehicle_unknown_reg_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, auth) = setup_org(&app, "Edit Vehicle 404 Org").await;

    let req = test::TestRequest::put()
        .uri("/api/vehicles/NO-SUCH-VH")
        .insert_header(("Authorization", auth))
        .set_json(&UpdateVehiclePayload { capacity: 5, unit: "MetricTon".to_string(), vehicle_type: None })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_edit_vehicle_from_another_org_returns_403() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Edit Vehicle Owner").await;
    let (_other, other_auth) = setup_org(&app, "Edit Vehicle Attacker").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&CreateVehiclePayload {
            registration_number: "OWNED-VH-1".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

    let req = test::TestRequest::put()
        .uri("/api/vehicles/OWNED-VH-1")
        .insert_header(("Authorization", other_auth))
        .set_json(&UpdateVehiclePayload { capacity: 999, unit: "MetricTon".to_string(), vehicle_type: None })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_track_endpoint_records_location_with_only_the_tracker_key() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Tracker Push Org").await;
    let vehicle = register_vehicle(&app, org.id, &auth, "TRK-VH-1").await;

    // No Authorization header — the key in the path is the whole credential.
    let req = test::TestRequest::post()
        .uri(&format!("/api/track/{}", vehicle.tracker_key))
        .set_json(&TrackLocationPayload { latitude: 18.5204, longitude: 73.8567 })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Location> = test::read_body_json(resp).await;
    let loc = body.data.unwrap();
    assert_eq!(loc.latitude, 18.5204);
    assert_eq!(loc.longitude, 73.8567);
    assert!(loc.timestamp > 0, "server stamps the time");

    // The push is visible on the authenticated fleet list.
    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Vehicle>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let stored = body.data.unwrap().into_iter().find(|v| v.registration_number == "TRK-VH-1").unwrap();
    assert_eq!(stored.location.unwrap().latitude, 18.5204);
}

#[actix_web::test]
async fn test_track_endpoint_unknown_key_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/track/{}", Uuid::new_v4()))
        .set_json(&TrackLocationPayload { latitude: 1.0, longitude: 2.0 })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_track_endpoint_rejects_out_of_range_coordinates() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Tracker Range Org").await;
    let vehicle = register_vehicle(&app, org.id, &auth, "TRK-VH-2").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/track/{}", vehicle.tracker_key))
        .set_json(&TrackLocationPayload { latitude: 120.0, longitude: 73.0 })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_rotate_tracker_key_issues_a_new_key_and_invalidates_the_old() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Tracker Rotate Org").await;
    let vehicle = register_vehicle(&app, org.id, &auth, "TRK-VH-3").await;
    let old_key = vehicle.tracker_key;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/TRK-VH-3/tracker-key/rotate")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let new_key = test::read_body_json::<ApiResponse<Vehicle>, _>(resp).await.data.unwrap().tracker_key;
    assert_ne!(new_key, old_key);

    // Old key is dead, new key works.
    let req = test::TestRequest::post()
        .uri(&format!("/api/track/{}", old_key))
        .set_json(&TrackLocationPayload { latitude: 1.0, longitude: 1.0 })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);

    let req = test::TestRequest::post()
        .uri(&format!("/api/track/{}", new_key))
        .set_json(&TrackLocationPayload { latitude: 1.0, longitude: 1.0 })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_rotate_tracker_key_from_another_org_returns_403() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Rotate Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Rotate Attacker Org").await;
    register_vehicle(&app, org.id, &auth, "TRK-VH-4").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/TRK-VH-4/tracker-key/rotate")
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_add_vehicle_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let target_org_id = Uuid::new_v4();
    let attacker_org_id = Uuid::new_v4();
    let payload = CreateVehiclePayload {
        registration_number: "HACK-VH-001".to_string(),
        capacity: 10,
        unit: "MetricTon".to_string(),
        vehicle_type: None,
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", target_org_id))
        .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_add_vehicle_with_type_and_change_it_on_edit() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Typed Vehicle Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateVehiclePayload {
            registration_number: "TYPE-VH-1".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
            vehicle_type: Some("Tipper".to_string()),
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let created: ApiResponse<Vehicle> = test::read_body_json(resp).await;
    assert_eq!(created.data.unwrap().vehicle_type, VehicleType::Tipper);

    // Omitting vehicle_type on edit keeps the stored type.
    let req = test::TestRequest::put()
        .uri("/api/vehicles/TYPE-VH-1")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateVehiclePayload { capacity: 11, unit: "MetricTon".to_string(), vehicle_type: None })
        .to_request();
    let body: ApiResponse<Vehicle> = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().vehicle_type, VehicleType::Tipper);

    let req = test::TestRequest::put()
        .uri("/api/vehicles/TYPE-VH-1")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateVehiclePayload {
            capacity: 11,
            unit: "MetricTon".to_string(),
            vehicle_type: Some("Tanker".to_string()),
        })
        .to_request();
    let body: ApiResponse<Vehicle> = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.data.unwrap().vehicle_type, VehicleType::Tanker);

    let req = test::TestRequest::get()
        .uri("/api/vehicles")
        .insert_header(("Authorization", auth))
        .to_request();
    let body: ApiResponse<Vec<Vehicle>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let stored = body.data.unwrap().into_iter().find(|v| v.registration_number == "TYPE-VH-1").unwrap();
    assert_eq!(stored.vehicle_type, VehicleType::Tanker);
}

#[actix_web::test]
async fn test_add_vehicle_without_type_defaults_to_truck() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Untyped Vehicle Org").await;

    // Raw JSON with no vehicle_type field, as older clients send.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth))
        .set_json(serde_json::json!({ "registration_number": "TYPE-VH-2", "capacity": 5, "unit": "Kg" }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let body: ApiResponse<Vehicle> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().vehicle_type, VehicleType::Truck);
}
