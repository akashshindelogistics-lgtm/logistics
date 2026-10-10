use super::*;

/// Walk a fresh dispatch through PENDING -> CONFIRMED -> LOADED ->
/// IN_TRANSIT via the API, returning its auth header and id so a test
/// can attempt the final IN_TRANSIT -> DELIVERED move itself.
async fn advance_to_in_transit(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_name: &str,
) -> (Uuid, String) {
    let (_org, dispatch, auth) = setup_dispatch(app, org_name).await;
    for status in [
        DispatchStatus::Confirmed,
        DispatchStatus::Loaded,
        DispatchStatus::InTransit,
    ] {
        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&UpdateDispatchStatusPayload {
                status,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(app, req).await;
        assert_eq!(
            resp.status().as_u16(),
            200,
            "advancing to {status} should succeed"
        );
    }
    (dispatch.id, auth)
}

async fn dispatch_notifications(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    dispatch_id: Uuid,
    auth: &str,
) -> Vec<Notification> {
    let req = test::TestRequest::get()
        .uri(&format!("/api/dispatches/{}/notifications", dispatch_id))
        .insert_header(("Authorization", auth.to_string()))
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    test::read_body_json::<ApiResponse<Vec<Notification>>, _>(resp).await.data.unwrap()
}

#[actix_web::test]
async fn test_list_dispatches_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api/dispatches").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_dispatch_rejects_customer_from_another_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Dispatch Cust A").await;
    let (org_b, auth_b) = setup_org(&app, "Dispatch Cust B").await;
    let foreign_customer = create_customer_via_api(&app, org_b.id, &auth_b, "B's Customer").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org_a.id))
        .insert_header(("Authorization", auth_a))
        .set_json(&DispatchRequestPayload {
            vehicle_source: VehicleSource::Own,
            vendor_id: None,
            customer_id: foreign_customer.id,
            line_items: vec![DispatchLineItemPayload {
                stock_description: "Anything".to_string(),
                requested_quantity: 1,
            }],
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.message.contains("different organization"), "{}", body.message);
}

#[actix_web::test]
async fn test_dispatch_stock_no_stock_returns_error() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let payload = DispatchRequestPayload {
        vehicle_source: VehicleSource::Own,
        vendor_id: None,
        customer_id,
        line_items: vec![DispatchLineItemPayload {
            stock_description: "Nonexistent Stock Description".to_string(),
            requested_quantity: 10,
        }],
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org_id))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_dispatch_stock_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org_id))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_dispatch_stock_carries_multiple_line_items() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    // setup_dispatch already dispatched "Dispatch Test Goods" x10 on the
    // one godown+vehicle; reuse its org/customer for a second, multi-line
    // dispatch. The vehicle is now on an active trip, so add a second
    // vehicle + driver and more stock first.
    let (org, first, auth) = setup_dispatch(&app, "Multi Line Dispatch Org").await;
    assert_eq!(first.line_items.len(), 1);
    // The dispatch route response carries the stock's category through.
    assert_eq!(first.line_items[0].category, "General");

    // Second vehicle + active driver.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateVehiclePayload {
            registration_number: "DISP-VH-002".to_string(),
            capacity: 100_000,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    test::call_service(&app, req).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/drivers", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateDriverPayload {
            name: "Second Driver".to_string(),
            license_number: "DISP-LIC-002".to_string(),
            phone: "+91 90000 00001".to_string(),
        })
        .to_request();
    let driver: ApiResponse<Driver> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let req = test::TestRequest::put()
        .uri("/api/vehicles/DISP-VH-002/driver")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&AssignDriverPayload { driver_id: Some(driver.data.unwrap().id) })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    // A second stock item in the same org (a fresh godown).
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateGodownPayload {
            name: "Second Godown".to_string(),
            address: "9 Second Road".to_string(),
            max_capacity: None,
        })
        .to_request();
    let godown2: ApiResponse<Godown> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let godown2 = godown2.data.unwrap();
    for (desc, qty) in [("Bricks", 500i64), ("Tiles", 300i64)] {
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown2.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateStockPayload {
                volume_in_size: 1,
                quantity: qty,
                description: desc.to_string(),
                category: "General".to_string(),
                reorder_threshold: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);
    }

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&DispatchRequestPayload {
            vehicle_source: VehicleSource::Own,
            vendor_id: None,
            customer_id: first.customer_id,
            line_items: vec![
                DispatchLineItemPayload { stock_description: "Bricks".to_string(), requested_quantity: 120 },
                DispatchLineItemPayload { stock_description: "Tiles".to_string(), requested_quantity: 40 },
            ],
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
    let order = body.data.unwrap();
    assert_eq!(order.line_items.len(), 2);
    assert_eq!(order.line_items[0].stock_description, "Bricks");
    assert_eq!(order.line_items[1].stock_description, "Tiles");
    assert_eq!(order.line_items.iter().map(|li| li.quantity).sum::<i64>(), 160);

    // A duplicated description is rejected with 400.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&DispatchRequestPayload {
            vehicle_source: VehicleSource::Own,
            vendor_id: None,
            customer_id: first.customer_id,
            line_items: vec![
                DispatchLineItemPayload { stock_description: "Bricks".to_string(), requested_quantity: 1 },
                DispatchLineItemPayload { stock_description: "Bricks".to_string(), requested_quantity: 2 },
            ],
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_list_dispatches_with_valid_token_returns_200() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let org_id = Uuid::new_v4();
    let req = test::TestRequest::get()
        .uri("/api/dispatches")
        .insert_header(("Authorization", make_auth_header(org_id, "Test")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<Vec<DispatchOrder>> = test::read_body_json(resp).await;
    assert!(body.success);
    // No dispatches for a brand-new UUID org_id — empty list is valid
    assert_eq!(body.data.unwrap().len(), 0);
}

#[actix_web::test]
async fn test_update_dispatch_status_without_token_returns_401() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", Uuid::new_v4()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Confirmed,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_update_dispatch_status_nonexistent_dispatch_returns_404() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", Uuid::new_v4()))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Confirmed,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_update_dispatch_status_invalid_payload() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", Uuid::new_v4()))
        .insert_header(("Content-Type", "application/json"))
        .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
        .set_payload("{bad json}")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_update_dispatch_status_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, _auth) = setup_dispatch(&app, "Status Owner Org").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch.id))
        .insert_header((
            "Authorization",
            make_auth_header(Uuid::new_v4(), "Attacker"),
        ))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Confirmed,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_update_dispatch_status_valid_transition_succeeds() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, auth) = setup_dispatch(&app, "Status Transition Org").await;
    assert_eq!(dispatch.status, DispatchStatus::Pending);

    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch.id))
        .insert_header(("Authorization", auth))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Confirmed,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
    assert!(body.success);
    let updated = body.data.unwrap();
    assert_eq!(updated.status, DispatchStatus::Confirmed);
    assert_eq!(updated.status_history.len(), 2);
    assert_eq!(updated.status_history[0].status, DispatchStatus::Pending);
    assert_eq!(updated.status_history[1].status, DispatchStatus::Confirmed);
}

#[actix_web::test]
async fn test_update_dispatch_status_rejects_illegal_transition() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, auth) = setup_dispatch(&app, "Status Illegal Org").await;

    // PENDING -> DELIVERED skips CONFIRMED/LOADED/IN_TRANSIT — must be rejected,
    // even with proof of delivery attached (the state-machine check runs first).
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch.id))
        .insert_header(("Authorization", auth))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Delivered,
            proof_of_delivery: Some(ProofOfDeliveryPayload {
                receiver_name: "Priya Sharma".to_string(),
                signature_or_photo_url: "https://example.com/sig.png".to_string(),
            }),
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_update_dispatch_status_returned_credits_stock_back_into_a_godown() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, dispatch, auth) = setup_dispatch(&app, "Return Flow Org").await;
    // setup_dispatch stocked "Dispatch Test Goods" x100 and dispatched 10 of it.

    async fn goods_qty(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        org_id: Uuid,
        auth: &str,
    ) -> i64 {
        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{org_id}/godowns"))
            .insert_header(("Authorization", auth.to_string()))
            .to_request();
        let body: ApiResponse<Vec<Godown>> =
            test::read_body_json(test::call_service(app, req).await).await;
        body.data
            .unwrap()
            .iter()
            .flat_map(|g| &g.stock)
            .filter(|s| s.description == "Dispatch Test Goods")
            .map(|s| s.quantity)
            .sum()
    }

    assert_eq!(goods_qty(&app, org.id, &auth).await, 90, "10 units were dispatched");

    for status in [
        DispatchStatus::Confirmed,
        DispatchStatus::Loaded,
        DispatchStatus::InTransit,
    ] {
        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&UpdateDispatchStatusPayload {
                status,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);
    }

    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Returned,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
    assert_eq!(body.data.unwrap().status, DispatchStatus::Returned);

    // The 10 returned units are back in the godown.
    assert_eq!(goods_qty(&app, org.id, &auth).await, 100);
}

#[actix_web::test]
async fn test_update_dispatch_status_delivered_without_proof_returns_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (dispatch_id, auth) = advance_to_in_transit(&app, "POD Missing Org").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch_id))
        .insert_header(("Authorization", auth))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Delivered,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
    assert!(body.message.contains("Proof of delivery"));
}

#[actix_web::test]
async fn test_update_dispatch_status_delivered_with_proof_succeeds() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (dispatch_id, auth) = advance_to_in_transit(&app, "POD Present Org").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch_id))
        .insert_header(("Authorization", auth))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Delivered,
            proof_of_delivery: Some(ProofOfDeliveryPayload {
                receiver_name: "Priya Sharma".to_string(),
                signature_or_photo_url: "https://example.com/sig.png".to_string(),
            }),
            return_to_godown_id: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
    assert!(body.success);
    let updated = body.data.unwrap();
    assert_eq!(updated.status, DispatchStatus::Delivered);
    let proof = updated.proof_of_delivery.expect("proof should be set");
    assert_eq!(proof.receiver_name, "Priya Sharma");
    assert_eq!(proof.signature_or_photo_url, "https://example.com/sig.png");
    assert!(proof.delivered_at > 0);

    // And it round-trips through a fresh fetch, not just the response.
    let fetched = DispatchOrder::get_by_id(dispatch_id).unwrap().unwrap();
    assert!(fetched.proof_of_delivery.is_some());
}

#[actix_web::test]
async fn test_dispatch_stock_returns_403_for_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let target_org_id = Uuid::new_v4();
    let attacker_org_id = Uuid::new_v4();
    let payload = DispatchRequestPayload {
        vehicle_source: VehicleSource::Own,
        vendor_id: None,
        customer_id: Uuid::new_v4(),
        line_items: vec![DispatchLineItemPayload {
            stock_description: "Stolen Stock".to_string(),
            requested_quantity: 10,
        }],
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", target_org_id))
        .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
        .set_json(&payload)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 403);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(!body.success);
}

#[actix_web::test]
async fn test_creating_a_dispatch_records_customer_and_driver_notifications() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, auth) = setup_dispatch(&app, "Notify Create Org").await;

    let notifs = dispatch_notifications(&app, dispatch.id, &auth).await;
    assert_eq!(notifs.len(), 2);
    assert!(notifs.iter().all(|n| n.event == NotificationEvent::DispatchCreated));

    // setup_dispatch's customer has no contact details → SKIPPED.
    let customer = notifs.iter().find(|n| n.recipient_kind == "customer").unwrap();
    assert_eq!(customer.status, NotificationStatus::Skipped);

    // The assigned driver has a phone → a QUEUED SMS.
    let driver = notifs.iter().find(|n| n.recipient_kind == "driver").unwrap();
    assert_eq!(driver.status, NotificationStatus::Queued);
    assert_eq!(driver.channel, NotificationChannel::Sms);
}

#[actix_web::test]
async fn test_delivering_a_dispatch_records_a_delivered_notification() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (dispatch_id, auth) = advance_to_in_transit(&app, "Notify Deliver Org").await;

    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch_id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Delivered,
            proof_of_delivery: Some(ProofOfDeliveryPayload {
                receiver_name: "R".to_string(),
                signature_or_photo_url: "https://x/y.png".to_string(),
            }),
            return_to_godown_id: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let notifs = dispatch_notifications(&app, dispatch_id, &auth).await;
    let delivered: Vec<_> = notifs
        .iter()
        .filter(|n| n.event == NotificationEvent::DispatchDelivered)
        .collect();
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].recipient_kind, "customer");
    assert!(delivered[0].body.contains("delivered"));
}

#[actix_web::test]
async fn test_dispatcher_can_dispatch_but_warehouse_staff_cannot() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, dispatch, admin) = setup_dispatch(&app, "Role Dispatch Org").await;
    let customer_id = dispatch.customer_id;

    // setup_dispatch already put the one vehicle on a PENDING trip; cancel
    // it (as Admin) so a fresh dispatch has a free vehicle to pick.
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", dispatch.id))
        .insert_header(("Authorization", admin.clone()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Cancelled,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let dispatcher = add_user_and_login(&app, org.id, &admin, "d@example.com", OrgRole::Dispatcher).await;
    let warehouse = add_user_and_login(&app, org.id, &admin, "w@example.com", OrgRole::WarehouseStaff).await;

    let body = DispatchRequestPayload {

        vehicle_source: VehicleSource::Own,

        vendor_id: None,
        customer_id,
        line_items: vec![DispatchLineItemPayload {
            stock_description: "Dispatch Test Goods".to_string(),
            requested_quantity: 1,
        }],
    };

    // Warehouse staff: 403.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", warehouse))
        .set_json(&body)
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    // Dispatcher: allowed (200 — the setup org has stock + a free vehicle).
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", dispatcher))
        .set_json(&body)
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_dispatch_rejected_when_no_vehicle_has_an_active_driver() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "NoDriver Co").await;

    // Vehicle, godown+stock, customer with a location — but no driver.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateVehiclePayload {
            registration_number: "ND-VH-1".to_string(),
            capacity: 20,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateGodownPayload {
            name: "ND Godown".to_string(),
            address: "1 ND Road".to_string(),
            max_capacity: None,
        })
        .to_request();
    let godown: ApiResponse<Godown> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let godown = godown.data.unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateStockPayload {
            volume_in_size: 1,
            quantity: 100,
            description: "Widgets".to_string(),
            category: "General".to_string(),
            reorder_threshold: None,
        })
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateCustomerPayload {
            name: "ND Customer".to_string(),
            address: "2 ND Lane".to_string(),
            latitude: None,
            longitude: None,
            location_address: None,
            phone: None,
            email: None,
        })
        .to_request();
    let customer: ApiResponse<Customer> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let customer = customer.data.unwrap();

    let req = test::TestRequest::put()
        .uri(&format!("/api/customers/{}/location", customer.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&LocationPayload {
            latitude: 19.07,
            longitude: 72.87,
            address: Some("Mumbai".to_string()),
        })
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", auth))
        .set_json(&DispatchRequestPayload {
            vehicle_source: VehicleSource::Own,
            vendor_id: None,
            customer_id: customer.id,
            line_items: vec![DispatchLineItemPayload {
                stock_description: "Widgets".to_string(),
                requested_quantity: 5,
            }],
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.message.contains("active assigned driver"), "{}", body.message);
}
