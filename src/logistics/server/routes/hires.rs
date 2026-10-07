//! Vehicle hires from vendors and the payments recorded against them.

use crate::logistics::customer::customer::Customer;
use crate::logistics::vendor::hire::{
    HireAssignment, HireError, HireStatus, VehicleHire, VendorPayment,
};
use crate::logistics::vehicle::vehicle::Unit;
use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;
use super::dispatches::notify_dispatch_created;

/// The vendor's truck, driver and agreed rate, entered after the vendor
/// confirms a hire.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AssignHirePayload {
    pub registration_number: String,
    /// Must be at least the hire's `required_volume`.
    pub capacity: i64,
    #[serde(default = "default_hire_unit")]
    pub unit: Unit,
    pub driver_name: String,
    pub driver_phone: String,
    #[serde(default)]
    pub driver_license: Option<String>,
    /// Agreed hire cost for the whole trip, whole currency units, > 0.
    pub freight_amount: i64,
    /// Paid up front; 0..=freight_amount. Defaults to 0.
    #[serde(default)]
    pub advance_paid: i64,
}

/// A payment to a vendor against one hire, after the advance.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct VendorPaymentPayload {
    /// Whole currency units; > 0 and no more than the hire's `balance_due`.
    pub amount: i64,
    /// ISO `YYYY-MM-DD`; defaults to today.
    #[serde(default)]
    pub paid_on: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn default_hire_unit() -> Unit {
    Unit::MetricTon
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct HireListQuery {
    /// Only hires in this status: REQUESTED, CONFIRMED, RELEASED or CANCELLED.
    #[serde(default)]
    pub status: Option<HireStatus>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleHireResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<VehicleHire>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleHireListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<VehicleHire>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VendorPaymentListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<VendorPayment>>,
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/vehicle-hires",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID"), HireListQuery),
    responses(
        (status = 200, description = "The organization's vehicle hires, newest first", body = VehicleHireListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/vehicle-hires")]
pub async fn list_vehicle_hires(
    path: web::Path<Uuid>,
    query: web::Query<HireListQuery>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match VehicleHire::list_by_org(org_id, query.status) {
        Ok(hires) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} vehicle hires", hires.len()),
            data: Some(hires),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicle hires: {}", err),
            data: None,
        }),
    }
}

/// Load a hire by id, returning an error `HttpResponse` unless it exists and
/// belongs to the caller's organization.
fn load_owned_hire(hire_id: Uuid, auth_org_id: Uuid) -> Result<VehicleHire, HttpResponse> {
    match VehicleHire::get_by_id(hire_id) {
        Ok(Some(h)) if h.org_id == auth_org_id => Ok(h),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: hire belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Vehicle hire not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch vehicle hire: {}", err),
            data: None,
        })),
    }
}

fn hire_error_response(action: &str, err: HireError) -> HttpResponse {
    match err {
        HireError::InvalidInput(msg) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: msg,
            data: None,
        }),
        HireError::Conflict(msg) => HttpResponse::Conflict().json(ApiResponse::<String> {
            success: false,
            message: msg,
            data: None,
        }),
        HireError::Db(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to {action}: {err}"),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/vehicle-hires/{id}/payments",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle hire UUID")),
    request_body = VendorPaymentPayload,
    responses(
        (status = 201, description = "Payment recorded; returns the hire with its new balance", body = VehicleHireResponse),
        (status = 400, description = "Non-positive amount, more than the balance owed, or a bad date", body = EmptyResponse),
        (status = 403, description = "Forbidden (other org, or not Admin/Dispatcher)", body = EmptyResponse),
        (status = 404, description = "Hire not found", body = EmptyResponse),
        (status = 409, description = "No truck or rate assigned to the hire yet", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/vehicle-hires/{id}/payments")]
pub async fn record_vendor_payment(
    path: web::Path<Uuid>,
    payload: web::Json<VendorPaymentPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let mut hire = match load_owned_hire(path.into_inner(), auth.org_id) {
        Ok(h) => h,
        Err(resp) => return resp,
    };
    let payload = payload.into_inner();
    match hire.record_payment(payload.amount, payload.paid_on, payload.note) {
        Ok(_) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Vendor payment recorded".to_string(),
            data: Some(hire),
        }),
        Err(err) => hire_error_response("record vendor payment", err),
    }
}

#[utoipa::path(
    get,
    path = "/api/vehicle-hires/{id}/payments",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle hire UUID")),
    responses(
        (status = 200, description = "Payments after the advance, oldest first", body = VendorPaymentListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Hire not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/vehicle-hires/{id}/payments")]
pub async fn list_vendor_payments(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let hire = match load_owned_hire(path.into_inner(), auth.org_id) {
        Ok(h) => h,
        Err(resp) => return resp,
    };
    match hire.payments() {
        Ok(payments) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} vendor payments", payments.len()),
            data: Some(payments),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vendor payments: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicle-hires/{id}/assign",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle hire UUID")),
    request_body = AssignHirePayload,
    responses(
        (status = 200, description = "Truck assigned; the hire is CONFIRMED and its dispatches are PENDING", body = VehicleHireResponse),
        (status = 400, description = "Missing field, bad amount, or truck too small for the load", body = EmptyResponse),
        (status = 403, description = "Forbidden (other org, or not Admin/Dispatcher)", body = EmptyResponse),
        (status = 404, description = "Hire not found", body = EmptyResponse),
        (status = 409, description = "Hire already assigned or closed", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicle-hires/{id}/assign")]
pub async fn assign_vehicle_hire(
    path: web::Path<Uuid>,
    payload: web::Json<AssignHirePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let mut hire = match load_owned_hire(path.into_inner(), auth.org_id) {
        Ok(h) => h,
        Err(resp) => return resp,
    };

    let payload = payload.into_inner();
    let assignment = HireAssignment {
        registration_number: payload.registration_number,
        capacity: payload.capacity,
        unit: payload.unit,
        driver_name: payload.driver_name,
        driver_phone: payload.driver_phone,
        driver_license: payload.driver_license,
        freight_amount: payload.freight_amount,
        advance_paid: payload.advance_paid,
    };
    match hire.assign(assignment) {
        Ok(released) => {
            // The dispatches are now real: tell each customer and the hired
            // driver, as an own-fleet dispatch does at creation.
            for order in &released {
                if let Ok(Some(customer)) = Customer::get_by_id(order.customer_id) {
                    notify_dispatch_created(auth.org_id, order, &customer).await;
                }
            }
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                message: format!(
                    "Hired truck assigned; {} dispatch(es) moved to PENDING",
                    released.len()
                ),
                data: Some(hire),
            })
        }
        Err(err) => hire_error_response("assign vehicle hire", err),
    }
}
