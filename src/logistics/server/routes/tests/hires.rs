use super::*;

/// An org with no vehicles: one godown with 50 "Hire Goods" (volume 1),
/// a located customer, and an active vendor created through the API.
async fn setup_hire_org(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    name: &str,
) -> (Organization, String, Customer, VehicleVendor) {
    let (org, auth) = setup_org(app, name).await;
    let godown = Godown::create(org.id, "Hire Godown", "MIDC", None).expect("godown");
    Stock::new(1, 50, "Hire Goods").add_to_godown(godown.id).expect("stock");
    let mut customer = Customer::create_customer(org.id, "Hire Buyer", "Baner").expect("customer");
    customer.update_location(18.56, 73.78, Some("Baner")).expect("location");

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vendors", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&vendor_payload("Route Vendor"))
        .to_request();
    let body: ApiResponse<VehicleVendor> =
        test::read_body_json(test::call_service(app, req).await).await;
    (org, auth, customer, body.data.expect("vendor"))
}

fn hired_dispatch_payload(customer_id: Uuid, vendor_id: Option<Uuid>) -> DispatchRequestPayload {
    DispatchRequestPayload {
        customer_id,
        line_items: vec![DispatchLineItemPayload {
            stock_description: "Hire Goods".to_string(),
            requested_quantity: 5,
        }],
        vehicle_source: VehicleSource::Hired,
        vendor_id,
    }
}

fn assign_payload(reg: &str, capacity: i64) -> AssignHirePayload {
    AssignHirePayload {
        registration_number: reg.to_string(),
        capacity,
        unit: Unit::MetricTon,
        driver_name: "Vendor Driver".to_string(),
        driver_phone: "+91 97000 00000".to_string(),
        driver_license: Some("MH-DL-1".to_string()),
        freight_amount: 8_000,
        advance_paid: 6_000,
    }
}

#[actix_web::test]
async fn test_hired_dispatch_then_assign_over_the_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth, customer, vendor) = setup_hire_org(&app, "Hire Ops").await;

    // HIRED without a vendor -> 400.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&hired_dispatch_payload(customer.id, None))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);

    // With a vendor -> AWAITING_VEHICLE, and nobody is notified yet.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&hired_dispatch_payload(customer.id, Some(vendor.id)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let order = test::read_body_json::<ApiResponse<DispatchOrder>, _>(resp).await.data.unwrap();
    assert_eq!(order.status, DispatchStatus::AwaitingVehicle);
    assert_eq!(order.vehicle_registration_number, None);
    assert!(Notification::list_by_dispatch(order.id).unwrap().is_empty());

    // The hire is listed as REQUESTED.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vehicle-hires?status=REQUESTED", org.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let hires = test::read_body_json::<ApiResponse<Vec<VehicleHire>>, _>(
        test::call_service(&app, req).await,
    )
    .await
    .data
    .unwrap();
    assert_eq!(hires.len(), 1);
    assert_eq!(hires[0].vendor_name, "Route Vendor");
    let hire_id = hires[0].id;

    // Too small a truck -> 400.
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{hire_id}/assign"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&assign_payload("MH12 HR 9", 4))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);

    // Assign -> CONFIRMED, dispatch PENDING with the truck, notifications out.
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{hire_id}/assign"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&assign_payload("MH12 HR 9", 10))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let hire = test::read_body_json::<ApiResponse<VehicleHire>, _>(resp).await.data.unwrap();
    assert_eq!(hire.status, HireStatus::Confirmed);

    let dispatch = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
    assert_eq!(dispatch.status, DispatchStatus::Pending);
    assert_eq!(dispatch.vehicle_registration_number.as_deref(), Some("MH12 HR 9"));
    let recipients: Vec<String> = Notification::list_by_dispatch(order.id)
        .unwrap()
        .into_iter()
        .map(|n| n.recipient)
        .collect();
    assert_eq!(recipients.len(), 2, "customer + hired driver: {recipients:?}");
    assert!(recipients.contains(&"+91 97000 00000".to_string()), "{recipients:?}");

    // Assigning twice -> 409.
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{hire_id}/assign"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&assign_payload("MH12 HR 10", 10))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 409);

    // The vendor now has history, so it can't be deleted.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/vendors/{}", vendor.id))
        .insert_header(("Authorization", auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 409);
}

#[actix_web::test]
async fn test_vehicle_hire_routes_enforce_org_and_role() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth, customer, vendor) = setup_hire_org(&app, "Hire Guard").await;
    let order = Organization::get_by_id(org.id)
        .unwrap()
        .unwrap()
        .dispatch_stock_on_hired_vehicle(
            &customer,
            &[DispatchLineItemInput { stock_description: "Hire Goods".into(), requested_quantity: 1 }],
            vendor.id,
        )
        .expect("hired dispatch");
    let hire_id = order.hire_id.unwrap();

    let other = make_auth_header(Uuid::new_v4(), "Attacker");
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{hire_id}/assign"))
        .insert_header(("Authorization", other.clone()))
        .set_json(&assign_payload("X", 10))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vehicle-hires", org.id))
        .insert_header(("Authorization", other))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let warehouse =
        add_user_and_login(&app, org.id, &auth, "hire-wh@example.com", OrgRole::WarehouseStaff).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{hire_id}/assign"))
        .insert_header(("Authorization", warehouse))
        .set_json(&assign_payload("X", 10))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{}/assign", Uuid::new_v4()))
        .insert_header(("Authorization", auth))
        .set_json(&assign_payload("X", 10))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_hired_trip_over_the_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth, first, vendor) = setup_hire_org(&app, "Hire Trips").await;
    let mut second = Customer::create_customer(org.id, "Second Stop", "Aundh").expect("customer");
    second.update_location(18.55, 73.80, Some("Aundh")).expect("location");

    let stop = |customer_id: Uuid| TripStopPayload {
        customer_id,
        line_items: vec![DispatchLineItemPayload {
            stock_description: "Hire Goods".to_string(),
            requested_quantity: 2,
        }],
    };
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/trips", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateTripPayload {
            stops: vec![stop(first.id), stop(second.id)],
            optimize_route: false,
            vehicle_source: VehicleSource::Hired,
            vendor_id: Some(vendor.id),
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let trip = test::read_body_json::<ApiResponse<Trip>, _>(resp).await.data.unwrap();
    assert_eq!(trip.status, TripStatus::Planned);
    assert!(trip.stops.iter().all(|s| s.status == DispatchStatus::AwaitingVehicle));

    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{}/assign", trip.hire_id.unwrap()))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&assign_payload("MH12 TRIP 9", 4))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri(&format!("/api/trips/{}", trip.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let trip = test::read_body_json::<ApiResponse<Trip>, _>(test::call_service(&app, req).await)
        .await
        .data
        .unwrap();
    assert_eq!(trip.vehicle_registration_number.as_deref(), Some("MH12 TRIP 9"));
    assert!(trip.stops.iter().all(|s| s.status == DispatchStatus::Pending));
}

#[actix_web::test]
async fn test_vendor_payments_over_the_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth, customer, vendor) = setup_hire_org(&app, "Hire Payments").await;
    let order = Organization::get_by_id(org.id)
        .unwrap()
        .unwrap()
        .dispatch_stock_on_hired_vehicle(
            &customer,
            &[DispatchLineItemInput { stock_description: "Hire Goods".into(), requested_quantity: 1 }],
            vendor.id,
        )
        .expect("hired dispatch");
    let hire_id = order.hire_id.unwrap();
    let pay = |amount: i64| VendorPaymentPayload {
        amount,
        paid_on: Some("2026-09-24".to_string()),
        note: Some("balance on POD".to_string()),
    };

    // Before a truck is assigned there is nothing to pay -> 409.
    let req = test::TestRequest::post()
        .uri(&format!("/api/vehicle-hires/{hire_id}/payments"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&pay(100))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 409);

    // Assign at 8,000 with 6,000 advance.
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-hires/{hire_id}/assign"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&assign_payload("MH12 PAY 9", 10))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    // Overpaying -> 400; warehouse staff -> 403.
    let req = test::TestRequest::post()
        .uri(&format!("/api/vehicle-hires/{hire_id}/payments"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&pay(2_001))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
    let warehouse =
        add_user_and_login(&app, org.id, &auth, "pay-wh@example.com", OrgRole::WarehouseStaff).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/vehicle-hires/{hire_id}/payments"))
        .insert_header(("Authorization", warehouse.clone()))
        .set_json(&pay(100))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    // Pay the balance.
    let req = test::TestRequest::post()
        .uri(&format!("/api/vehicle-hires/{hire_id}/payments"))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&pay(2_000))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let hire = test::read_body_json::<ApiResponse<VehicleHire>, _>(resp).await.data.unwrap();
    assert_eq!((hire.total_paid, hire.balance_due), (8_000, Some(0)));

    // Every role can read the payment history.
    let req = test::TestRequest::get()
        .uri(&format!("/api/vehicle-hires/{hire_id}/payments"))
        .insert_header(("Authorization", warehouse))
        .to_request();
    let payments = test::read_body_json::<ApiResponse<Vec<VendorPayment>>, _>(
        test::call_service(&app, req).await,
    )
    .await
    .data
    .unwrap();
    assert_eq!(payments.len(), 1);
    assert_eq!(payments[0].note.as_deref(), Some("balance on POD"));

    // The report shows the vendor as fully paid.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/reports", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let report = test::read_body_json::<ApiResponse<OpsReport>, _>(test::call_service(&app, req).await)
        .await
        .data
        .unwrap();
    assert_eq!(report.hired_transport.hire_cost_total, 8_000);
    assert_eq!(report.hired_transport.outstanding_to_vendors, 0);
}
