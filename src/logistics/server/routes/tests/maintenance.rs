use super::*;

#[actix_web::test]
async fn test_vehicle_maintenance_crud_via_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Maintenance Co").await;
    add_vehicle_via_api(&app, org.id, &auth, "KA07-M2-1111").await;

    // Schedule an oil change due in 5 days — "due soon".
    let req = test::TestRequest::post()
        .uri("/api/vehicles/KA07-M2-1111/maintenance")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&VehicleMaintenancePayload {
            description: "Oil change".to_string(),
            due_on: Some(iso_date_offset(5)),
            due_at_mileage_km: Some(50_000),
            last_service_on: Some(iso_date_offset(-175)),
            notes: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let created: ApiResponse<VehicleMaintenance> = test::read_body_json(resp).await;
    let item = created.data.unwrap();
    assert_eq!(item.status, MaintenanceStatus::DueSoon);
    assert_eq!(item.km_until_due, None); // no odometer reading recorded yet

    // It shows up in the per-vehicle list.
    let req = test::TestRequest::get()
        .uri("/api/vehicles/KA07-M2-1111/maintenance")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let list: ApiResponse<Vec<VehicleMaintenance>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(list.data.unwrap().len(), 1);

    // Record an odometer reading past the due mileage -> overdue by mileage.
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-maintenance/{}/mileage", item.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&RecordMileagePayload { current_mileage_km: 50_500 })
        .to_request();
    let recorded: ApiResponse<VehicleMaintenance> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let recorded = recorded.data.unwrap();
    assert_eq!(recorded.status, MaintenanceStatus::Overdue);
    assert_eq!(recorded.current_mileage_km, Some(50_500));

    // Update it: push the due date and mileage forward (the service was done).
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-maintenance/{}", item.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&VehicleMaintenancePayload {
            description: "Oil change".to_string(),
            due_on: Some(iso_date_offset(180)),
            due_at_mileage_km: Some(60_000),
            last_service_on: Some(iso_date_offset(0)),
            notes: Some("done at the depot".to_string()),
        })
        .to_request();
    let updated: ApiResponse<VehicleMaintenance> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let updated = updated.data.unwrap();
    assert_eq!(updated.status, MaintenanceStatus::UpToDate);
    assert_eq!(updated.notes, Some("done at the depot".to_string()));

    // The org-wide maintenance list sees it too.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vehicle-maintenance", org.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let org_list: ApiResponse<Vec<VehicleMaintenance>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(org_list.data.unwrap().len(), 1);

    // Delete it.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/vehicle-maintenance/{}", item.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri("/api/vehicles/KA07-M2-1111/maintenance")
        .insert_header(("Authorization", auth))
        .to_request();
    let list: ApiResponse<Vec<VehicleMaintenance>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert!(list.data.unwrap().is_empty());
}

#[actix_web::test]
async fn test_add_vehicle_maintenance_rejects_no_due_criterion_with_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, auth) = setup_org(&app, "No Criterion Co").await;
    add_vehicle_via_api(&app, _org.id, &auth, "NC-VH-1").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/NC-VH-1/maintenance")
        .insert_header(("Authorization", auth))
        .set_json(&VehicleMaintenancePayload {
            description: "Oil change".to_string(),
            due_on: None,
            due_at_mileage_km: None,
            last_service_on: None,
            notes: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_add_vehicle_maintenance_404_for_a_vehicle_in_another_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Fleet Owner").await;
    let (_org_b, auth_b) = setup_org(&app, "Fleet Other").await;
    add_vehicle_via_api(&app, org_a.id, &auth_a, "OWN-VH-2").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/OWN-VH-2/maintenance")
        .insert_header(("Authorization", auth_b))
        .set_json(&VehicleMaintenancePayload {
            description: "Sneaky service".to_string(),
            due_on: Some(iso_date_offset(30)),
            due_at_mileage_km: None,
            last_service_on: None,
            notes: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_update_vehicle_maintenance_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Maint Owner").await;
    let (_org_b, auth_b) = setup_org(&app, "Maint Intruder").await;
    add_vehicle_via_api(&app, org_a.id, &auth_a, "MAINT-VH-1").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/MAINT-VH-1/maintenance")
        .insert_header(("Authorization", auth_a))
        .set_json(&VehicleMaintenancePayload {
            description: "Brake pads".to_string(),
            due_on: Some(iso_date_offset(90)),
            due_at_mileage_km: None,
            last_service_on: None,
            notes: None,
        })
        .to_request();
    let created: ApiResponse<VehicleMaintenance> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let item_id = created.data.unwrap().id;

    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-maintenance/{item_id}"))
        .insert_header(("Authorization", auth_b))
        .set_json(&VehicleMaintenancePayload {
            description: "Hijacked".to_string(),
            due_on: Some(iso_date_offset(90)),
            due_at_mileage_km: None,
            last_service_on: None,
            notes: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_list_org_vehicle_maintenance_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, _auth_a) = setup_org(&app, "Maint Fleet A").await;
    let (_org_b, auth_b) = setup_org(&app, "Maint Fleet B").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vehicle-maintenance", org_a.id))
        .insert_header(("Authorization", auth_b))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}
