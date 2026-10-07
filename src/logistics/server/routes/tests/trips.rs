use super::*;

async fn located_customer(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    name: &str,
) -> Uuid {
    located_customer_at(app, org_id, auth, name, 19.07, 72.87).await
}

fn trip_stop(customer_id: Uuid, qty: i64) -> TripStopPayload {
    TripStopPayload {
        customer_id,
        line_items: vec![DispatchLineItemPayload {
            stock_description: "Dispatch Test Goods".to_string(),
            requested_quantity: qty,
        }],
    }
}

#[actix_web::test]
async fn test_create_a_two_stop_trip_and_read_it_back() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    // setup_dispatch stocks "Dispatch Test Goods" and registers one vehicle
    // + active driver, then makes a first dispatch — cancel it to free the
    // truck for the trip.
    let (org, first, auth) = setup_dispatch(&app, "Trip Route Org").await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", first.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Cancelled,
            proof_of_delivery: None,
            return_to_godown_id: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let c1 = located_customer(&app, org.id, &auth, "Stop One").await;
    let c2 = located_customer(&app, org.id, &auth, "Stop Two").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateTripPayload { stops: vec![trip_stop(c1, 3), trip_stop(c2, 4)], optimize_route: false, vehicle_source: VehicleSource::Own, vendor_id: None })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let trip = test::read_body_json::<ApiResponse<Trip>, _>(resp).await.data.unwrap();
    assert_eq!(trip.stops.len(), 2);
    assert_eq!(trip.status, TripStatus::Planned);
    let veh = trip.vehicle_registration_number.clone();
    assert!(trip.stops.iter().all(|s| s.vehicle_registration_number == veh));
    assert_eq!(
        trip.stops.iter().filter_map(|s| s.stop_sequence).collect::<Vec<_>>(),
        vec![1, 2]
    );

    // GET /api/trips/{id}
    let req = test::TestRequest::get()
        .uri(&format!("/api/trips/{}", trip.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let got = test::read_body_json::<ApiResponse<Trip>, _>(test::call_service(&app, req).await)
        .await.data.unwrap();
    assert_eq!(got.id, trip.id);
    assert_eq!(got.stops.len(), 2);

    // The trip's stops appear on the org's dispatch list, linked by trip_id.
    let req = test::TestRequest::get()
        .uri("/api/dispatches")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let dispatches = test::read_body_json::<ApiResponse<Vec<DispatchOrder>>, _>(
        test::call_service(&app, req).await,
    ).await.data.unwrap();
    assert_eq!(dispatches.iter().filter(|d| d.trip_id == Some(trip.id)).count(), 2);

    // And on the org's trip list.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let trips = test::read_body_json::<ApiResponse<Vec<Trip>>, _>(
        test::call_service(&app, req).await,
    ).await.data.unwrap();
    assert_eq!(trips.len(), 1);
}

#[actix_web::test]
async fn test_trip_rejects_bad_stops_and_wrong_role() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, first, auth) = setup_dispatch(&app, "Trip Guard Org").await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", first.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Cancelled, proof_of_delivery: None, return_to_godown_id: None,
        })
        .to_request();
    test::call_service(&app, req).await;

    let c1 = located_customer(&app, org.id, &auth, "G Stop One").await;

    // One stop -> 400.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateTripPayload { stops: vec![trip_stop(c1, 1)], optimize_route: false, vehicle_source: VehicleSource::Own, vendor_id: None })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);

    // Unknown customer -> 400.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateTripPayload { stops: vec![trip_stop(c1, 1), trip_stop(Uuid::new_v4(), 1)], optimize_route: false, vehicle_source: VehicleSource::Own, vendor_id: None })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);

    // Warehouse-staff role -> 403.
    let wh = add_user_and_login(&app, org.id, &auth, "trip-wh@example.com", OrgRole::WarehouseStaff).await;
    let c2 = located_customer(&app, org.id, &auth, "G Stop Two").await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", wh))
        .set_json(&CreateTripPayload { stops: vec![trip_stop(c1, 1), trip_stop(c2, 1)], optimize_route: false, vehicle_source: VehicleSource::Own, vendor_id: None })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_trip_optimize_route_reorders_stops_by_proximity_via_the_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, first, auth) = setup_dispatch(&app, "Trip Optimize Org").await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/dispatches/{}/status", first.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&UpdateDispatchStatusPayload {
            status: DispatchStatus::Cancelled, proof_of_delivery: None, return_to_godown_id: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    // c1 is the fixed start; c_far is much farther from c1 than c_near is.
    let c1 = located_customer_at(&app, org.id, &auth, "Start", 19.07, 72.87).await;
    let c_far = located_customer_at(&app, org.id, &auth, "Far Stop", 19.50, 73.30).await;
    let c_near = located_customer_at(&app, org.id, &auth, "Near Stop", 19.08, 72.88).await;

    // Given in "far, then near" order, but with optimize_route the API
    // should visit the nearer one second.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateTripPayload {
            vehicle_source: VehicleSource::Own,
            vendor_id: None,
            stops: vec![trip_stop(c1, 1), trip_stop(c_far, 1), trip_stop(c_near, 1)],
            optimize_route: true,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let trip = test::read_body_json::<ApiResponse<Trip>, _>(resp).await.data.unwrap();

    let ordered_customers: Vec<Uuid> = trip.stops.iter().map(|s| s.customer_id).collect();
    assert_eq!(ordered_customers, vec![c1, c_near, c_far]);
}
