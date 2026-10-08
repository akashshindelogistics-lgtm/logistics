//! Route tests, one module per domain, driven through `config_routes` with
//! a throwaway [`TestDb`] per test. Fixtures shared by several domains live
//! here.

use super::*;
use crate::logistics::test_support::TestDb;
use actix_web::{test, App};
use crate::logistics::ai::assistant::AssistantAnswer;
use crate::logistics::auth::auth::{generate_token, OrgSummary};
use crate::logistics::billing::invoice::{CustomerBillingSummary, Invoice, PaymentStatus};
use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::{
    DispatchLineItemInput, DispatchOrder, DispatchStatus, VehicleSource,
};
use crate::logistics::dispatch::trip::{Trip, TripStatus};
use crate::logistics::driver::driver::Driver;
use crate::logistics::godown::godown::Godown;
use crate::logistics::godown::transfer::StockTransfer;
use crate::logistics::notification::notification::{
    Notification, NotificationChannel, NotificationEvent, NotificationStatus,
};
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::reports::OpsReport;
use crate::logistics::stock::stock::Stock;
use crate::logistics::upload::upload::MAX_UPLOAD_BYTES;
use crate::logistics::user::user::{OrgRole, OrgUser};
use crate::logistics::vendor::hire::{HireStatus, VehicleHire, VendorPayment};
use crate::logistics::vendor::vendor::VehicleVendor;
use crate::logistics::vehicle::document::{ComplianceDocType, ComplianceStatus, VehicleDocument};
use crate::logistics::vehicle::maintenance::{MaintenanceStatus, VehicleMaintenance};
use crate::logistics::vehicle::vehicle::{Location, Unit, Vehicle};
use uuid::Uuid;

mod auth;
mod health;
mod orgs;
mod vehicles;
mod drivers;
mod vendors;
mod hires;
mod vehicle_documents;
mod maintenance;
mod godowns;
mod customers;
mod dispatches;
mod trips;
mod uploads;
mod ai;
mod billing;
mod reports;
mod notifications;
mod users;

fn make_auth_header(org_id: Uuid, org_name: &str) -> String {
    let token = generate_token(org_id, org_name).expect("Failed to generate test token");
    format!("Bearer {}", token)
}

/// Create an org with a vehicle, a godown with stock, and a customer
/// with a location, then dispatch stock to that customer, returning
/// `(org, dispatch, auth_header)`. Shared by the dispatch status and
/// summary route tests below.
async fn setup_dispatch(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_name: &str,
) -> (Organization, DispatchOrder, String) {
    let create_payload = CreateOrgPayload {
        name: org_name.to_string(),
        address: format!("1 {} Road", org_name),
        password: "dispatch_test_pass".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&create_payload)
        .to_request();
    let body: ApiResponse<Organization> =
        test::read_body_json(test::call_service(app, req).await).await;
    let org = body.data.unwrap();
    let auth = make_auth_header(org.id, &org.name);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/vehicles", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateVehiclePayload {
            registration_number: "DISP-VH-001".to_string(),
            capacity: 100_000,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    test::call_service(app, req).await;

    // A vehicle needs an active assigned driver to be dispatch-eligible.
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/drivers", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateDriverPayload {
            name: format!("{} Driver", org_name),
            license_number: "DISP-LIC-001".to_string(),
            phone: "+91 90000 00000".to_string(),
        })
        .to_request();
    let body: ApiResponse<Driver> =
        test::read_body_json(test::call_service(app, req).await).await;
    let driver = body.data.unwrap();

    let req = test::TestRequest::put()
        .uri("/api/vehicles/DISP-VH-001/driver")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&AssignDriverPayload {
            driver_id: Some(driver.id),
        })
        .to_request();
    assert_eq!(test::call_service(app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/godowns", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateGodownPayload {
            name: format!("{} Godown", org_name),
            address: format!("1 {} Godown Road", org_name),
            max_capacity: None,
        })
        .to_request();
    let body: ApiResponse<Godown> =
        test::read_body_json(test::call_service(app, req).await).await;
    let godown = body.data.unwrap();

    let req = test::TestRequest::post()
        .uri(&format!("/api/godowns/{}/stock", godown.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateStockPayload {
            volume_in_size: 100,
            quantity: 100,
            description: "Dispatch Test Goods".to_string(),
            category: "General".to_string(),
            reorder_threshold: None,
        })
        .to_request();
    test::call_service(app, req).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/customers", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&CreateCustomerPayload {
            name: format!("{} Customer", org_name),
            address: "2 Test Lane".to_string(),
            latitude: None,
            longitude: None,
            location_address: None,
            phone: None,
            email: None,
        })
        .to_request();
    let body: ApiResponse<Customer> =
        test::read_body_json(test::call_service(app, req).await).await;
    let customer = body.data.unwrap();

    let req = test::TestRequest::put()
        .uri(&format!("/api/customers/{}/location", customer.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Mumbai".to_string()),
        })
        .to_request();
    test::call_service(app, req).await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/dispatch", org.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&DispatchRequestPayload {
            vehicle_source: VehicleSource::Own,
            vendor_id: None,
            customer_id: customer.id,
            line_items: vec![DispatchLineItemPayload {
                stock_description: "Dispatch Test Goods".to_string(),
                requested_quantity: 10,
            }],
        })
        .to_request();
    let body: ApiResponse<DispatchOrder> =
        test::read_body_json(test::call_service(app, req).await).await;
    let dispatch = body.data.unwrap();

    (org, dispatch, auth)
}

async fn create_customer_via_api(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    name: &str,
) -> Customer {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{org_id}/customers"))
        .insert_header(("Authorization", auth.to_string()))
        .set_json(&CreateCustomerPayload {
            name: name.to_string(),
            address: format!("1 {name} Lane"),
            latitude: None,
            longitude: None,
            location_address: None,
            phone: None,
            email: None,
        })
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    test::read_body_json::<ApiResponse<Customer>, _>(resp)
        .await
        .data
        .unwrap()
}

async fn located_customer_at(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    name: &str,
    latitude: f64,
    longitude: f64,
) -> Uuid {
    let customer = create_customer_via_api(app, org_id, auth, name).await;
    let req = test::TestRequest::put()
        .uri(&format!("/api/customers/{}/location", customer.id))
        .insert_header(("Authorization", auth.to_string()))
        .set_json(&LocationPayload { latitude, longitude, address: None })
        .to_request();
    assert_eq!(test::call_service(app, req).await.status().as_u16(), 200);
    customer.id
}

/// Build a raw `multipart/form-data` body with one file field, for tests
/// — actix-web's test utilities have no multipart helper of their own.
fn multipart_body(boundary: &str, filename: &str, content_type: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// As Admin `admin_auth`, create a team member in `org_id` with `role`,
/// then log in as them and return their `Bearer …` header.
async fn add_user_and_login(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    admin_auth: &str,
    email: &str,
    role: OrgRole,
) -> String {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{org_id}/users"))
        .insert_header(("Authorization", admin_auth.to_string()))
        .set_json(&CreateUserPayload {
            name: format!("User {email}"),
            email: email.to_string(),
            password: "team-member-pw".to_string(),
            role,
        })
        .to_request();
    assert_eq!(test::call_service(app, req).await.status().as_u16(), 201);

    let req = test::TestRequest::post()
        .uri("/api/auth/user-login")
        .set_json(&UserLoginPayload {
            email: email.to_string(),
            password: "team-member-pw".to_string(),
        })
        .to_request();
    let resp = test::call_service(app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let data = test::read_body_json::<ApiResponse<LoginData>, _>(resp).await.data.unwrap();
    assert_eq!(data.role, role.as_str());
    format!("Bearer {}", data.token)
}

/// Create an org (with credentials) and return `(org, auth_header)`.
async fn setup_org(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_name: &str,
) -> (Organization, String) {
    let req = test::TestRequest::post()
        .uri("/api/orgs")
        .set_json(&CreateOrgPayload {
            name: org_name.to_string(),
            address: format!("1 {org_name} Road"),
            password: "driver_test_pass".to_string(),
        })
        .to_request();
    let body: ApiResponse<Organization> =
        test::read_body_json(test::call_service(app, req).await).await;
    let org = body.data.unwrap();
    let auth = make_auth_header(org.id, &org.name);
    (org, auth)
}

async fn add_vehicle_via_api(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    org_id: Uuid,
    auth: &str,
    reg: &str,
) {
    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{org_id}/vehicles"))
        .insert_header(("Authorization", auth.to_string()))
        .set_json(&CreateVehiclePayload {
            registration_number: reg.to_string(),
            capacity: 20,
            unit: "MetricTon".to_string(),
            vehicle_type: None,
        })
        .to_request();
    assert_eq!(test::call_service(app, req).await.status().as_u16(), 201);
}

/// An ISO `YYYY-MM-DD` string `offset` days from today (UTC).
fn iso_date_offset(offset: i64) -> String {
    let target = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64)
        .div_euclid(86_400)
        + offset;
    let z = target + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn vendor_payload(name: &str) -> CreateVendorPayload {
    CreateVendorPayload {
        name: name.to_string(),
        contact_person: Some("Anil".to_string()),
        phone: "+91 98200 00000".to_string(),
        gstin: Some("27AAPFU0939F1ZV".to_string()),
        notes: None,
    }
}
