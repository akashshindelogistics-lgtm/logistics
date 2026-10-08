//! Drivers (`/api/drivers`, `/api/orgs/{id}/drivers`), assigning one to a
//! vehicle, and the driver's phone: pairing it with a device token and the
//! location reports it sends (`POST /api/driver/location`).

use crate::logistics::driver::driver::Driver;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::vehicle::vehicle::{Location, Vehicle};
use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use super::vehicles::coordinates_in_range;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateDriverPayload {
    pub name: String,
    pub license_number: String,
    pub phone: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateDriverPayload {
    pub name: String,
    pub license_number: String,
    pub phone: String,
    /// Whether the driver is available to run a trip. A vehicle whose
    /// assigned driver is inactive cannot be selected for dispatch.
    pub is_active: bool,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct AssignDriverPayload {
    /// Driver to assign to the vehicle, or `null` to clear the assignment.
    #[serde(default)]
    pub driver_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Driver>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Driver>>,
}

/// Load a driver by id, returning an error `HttpResponse` unless it exists
/// and belongs to `auth_org_id`.
fn load_owned_driver(driver_id: Uuid, auth_org_id: Uuid) -> Result<Driver, HttpResponse> {
    match Driver::get_by_id(driver_id) {
        Ok(Some(driver)) if driver.org_id == auth_org_id => Ok(driver),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: driver belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Driver not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch driver: {}", err),
            data: None,
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/drivers",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Drivers for the authenticated organization", body = DriverListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/drivers")]
pub async fn list_drivers(auth: AuthenticatedOrg) -> impl Responder {
    match Driver::list_by_org(auth.org_id) {
        Ok(drivers) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} drivers", drivers.len()),
            data: Some(drivers),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list drivers: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/drivers",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateDriverPayload,
    responses(
        (status = 201, description = "Driver created successfully", body = DriverResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/drivers")]
pub async fn add_driver(
    path: web::Path<Uuid>,
    payload: web::Json<CreateDriverPayload>,
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
    match Driver::create(
        org_id,
        &payload.name,
        &payload.license_number,
        &payload.phone,
    ) {
        Ok(driver) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Driver created successfully".to_string(),
            data: Some(driver),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create driver: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/drivers/{id}",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Driver UUID")),
    request_body = UpdateDriverPayload,
    responses(
        (status = 200, description = "Driver updated successfully", body = DriverResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Driver not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/drivers/{id}")]
pub async fn update_driver(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateDriverPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut driver = match load_owned_driver(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match driver.update(
        &payload.name,
        &payload.license_number,
        &payload.phone,
        payload.is_active,
    ) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Driver updated successfully".to_string(),
            data: Some(driver),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update driver: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/drivers/{id}",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Driver UUID")),
    responses(
        (status = 200, description = "Driver deleted successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Driver not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/drivers/{id}")]
pub async fn delete_driver(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let driver = match load_owned_driver(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match driver.delete() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Driver deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete driver: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicles/{reg}/driver",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    request_body = AssignDriverPayload,
    responses(
        (status = 200, description = "Driver assignment updated", body = VehicleResponse),
        (status = 400, description = "Driver is not in this organization", body = EmptyResponse),
        (status = 404, description = "Vehicle not found in this organization", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicles/{reg}/driver")]
pub async fn assign_vehicle_driver(
    path: web::Path<String>,
    payload: web::Json<AssignDriverPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let reg_number = path.into_inner();

    let mut vehicle = match Vehicle::list_by_org(auth.org_id) {
        Ok(vehicles) => match vehicles
            .into_iter()
            .find(|v| v.registration_number == reg_number)
        {
            Some(v) => v,
            None => {
                return HttpResponse::NotFound().json(ApiResponse::<String> {
                    success: false,
                    message: "Vehicle not found in this organization".to_string(),
                    data: None,
                })
            }
        },
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch vehicle: {}", err),
                data: None,
            })
        }
    };

    // A driver assignment must reference one of this org's own drivers.
    // "belongs to another org" and "not found" both collapse to a 400 here —
    // from the caller's side it's just an invalid driver_id for them.
    if let Some(driver_id) = payload.driver_id
        && load_owned_driver(driver_id, auth.org_id).is_err()
    {
        return HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: "driver_id is not a driver in this organization".to_string(),
            data: None,
        });
    }

    match vehicle.assign_driver(payload.driver_id) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: match payload.driver_id {
                Some(_) => "Driver assigned to vehicle".to_string(),
                None => "Driver assignment cleared".to_string(),
            },
            data: Some(vehicle),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to assign driver: {}", err),
            data: None,
        }),
    }
}

// ── Driver phone tracking ─────────────────────────────────────────────────

/// How many fixes one `POST /api/driver/location` call may carry. A phone
/// that was offline for a while flushes its queue in batches of this size.
pub(super) const MAX_DRIVER_FIXES_PER_REQUEST: usize = 100;
/// How far ahead of the server clock a fix's `recorded_at` may be, to absorb
/// phone clock skew without letting a wrong clock pin the vehicle's location
/// in the future (which would make every later real fix look "older").
const MAX_FIX_CLOCK_SKEW_SECS: i64 = 300;

/// One position captured by the driver's phone.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct DriverLocationFix {
    pub latitude: f64,
    pub longitude: f64,
    /// When the phone captured the fix, in unix seconds. Not the time it was
    /// uploaded: an offline phone sends old fixes late.
    pub recorded_at: i64,
    /// Horizontal accuracy in metres, as reported by the OS.
    #[serde(default)]
    pub accuracy_m: Option<f64>,
    /// Ground speed in metres per second.
    #[serde(default)]
    pub speed_mps: Option<f64>,
}

/// Body of `POST /api/driver/location`.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DriverLocationPayload {
    pub fixes: Vec<DriverLocationFix>,
}

/// Result of a driver location report.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverLocationResult {
    /// How many fixes in the batch were valid and considered.
    pub accepted: usize,
    /// `false` when the whole batch was older than what the vehicle already
    /// has (for example a late duplicate upload), so nothing moved.
    pub location_updated: bool,
    pub vehicle_registration_number: String,
    /// The vehicle's location after the report.
    pub location: Option<Location>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverLocationResultResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DriverLocationResult>,
}

/// A newly issued driver device token. The plain token is shown exactly once.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverDeviceToken {
    pub driver_id: Uuid,
    pub device_token: Uuid,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverDeviceTokenResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DriverDeviceToken>,
}

/// The vehicle a driver's location reports move.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverMeVehicle {
    pub registration_number: String,
    /// The vehicle's last known position, from any source (this phone, a
    /// hardware tracker or a manual update).
    pub location: Option<Location>,
}

/// The paired driver, as seen from their own phone.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverMe {
    pub driver_id: Uuid,
    pub name: String,
    pub org_id: Uuid,
    pub org_name: String,
    /// `false` means location reports are refused (`403`) until a dispatcher
    /// marks the driver active again.
    pub is_active: bool,
    /// `None` means no vehicle is assigned, so location reports are refused
    /// (`409`) until a dispatcher assigns one.
    pub vehicle: Option<DriverMeVehicle>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverMeResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DriverMe>,
}

fn json_error(status: actix_web::http::StatusCode, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status).json(ApiResponse::<String> {
        success: false,
        message: message.into(),
        data: None,
    })
}

/// Resolve the driver from a device token in `Authorization: Bearer <token>`.
/// `401` for a missing, malformed or unknown token; an org JWT is not a
/// device token.
fn driver_from_device_token(req: &HttpRequest) -> Result<Driver, HttpResponse> {
    use actix_web::http::StatusCode;

    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .and_then(|t| Uuid::parse_str(t.trim()).ok())
        .ok_or_else(|| {
            json_error(StatusCode::UNAUTHORIZED, "Missing or malformed driver device token")
        })?;

    match Driver::by_device_token(token) {
        Ok(Some(d)) => Ok(d),
        Ok(None) => Err(json_error(StatusCode::UNAUTHORIZED, "Unknown driver device token")),
        Err(err) => Err(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to look up device token: {}", err),
        )),
    }
}

/// Check one fix from a driver's phone. Returns the reason it is unusable.
fn validate_driver_fix(fix: &DriverLocationFix, now: i64) -> Result<(), String> {
    if !coordinates_in_range(fix.latitude, fix.longitude) {
        return Err(
            "latitude must be between -90 and 90 and longitude between -180 and 180".to_string(),
        );
    }
    if fix.recorded_at <= 0 {
        return Err("recorded_at must be a positive unix timestamp in seconds".to_string());
    }
    if fix.recorded_at > now + MAX_FIX_CLOCK_SKEW_SECS {
        return Err(format!(
            "recorded_at is more than {} seconds in the future — check the phone's clock",
            MAX_FIX_CLOCK_SKEW_SECS
        ));
    }
    if fix.accuracy_m.is_some_and(|a| !a.is_finite() || a < 0.0) {
        return Err("accuracy_m must be a non-negative number".to_string());
    }
    if fix.speed_mps.is_some_and(|v| !v.is_finite() || v < 0.0) {
        return Err("speed_mps must be a non-negative number".to_string());
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/driver/location",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    request_body = DriverLocationPayload,
    responses(
        (status = 200, description = "Fixes processed", body = DriverLocationResultResponse),
        (status = 400, description = "Empty or oversized batch, or an invalid fix", body = EmptyResponse),
        (status = 401, description = "Missing or unknown device token", body = EmptyResponse),
        (status = 403, description = "The driver is inactive", body = EmptyResponse),
        (status = 409, description = "The driver has no assigned vehicle", body = EmptyResponse)
    )
)]
/// Location report from the driver's phone app. Authenticated by the driver's
/// device token (`Authorization: Bearer <token>`, issued by
/// `POST /api/drivers/{id}/device-token/rotate`) — not an org login. The
/// server works out which vehicle the driver is assigned to and moves it to
/// the newest fix in the batch. Fixes are timestamped by the phone, and the
/// vehicle's location only ever moves forward, so a late or repeated upload
/// cannot pull it backwards. The whole batch is rejected if any fix is
/// invalid, so the app can fix or drop the offending fix and resend rather
/// than guess which ones landed.
#[post("/driver/location")]
pub async fn report_driver_location(
    req: HttpRequest,
    payload: web::Json<DriverLocationPayload>,
) -> impl Responder {
    use actix_web::http::StatusCode;

    let driver = match driver_from_device_token(&req) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    if !driver.is_active {
        return json_error(StatusCode::FORBIDDEN, "This driver is inactive");
    }

    if payload.fixes.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "fixes must not be empty");
    }
    if payload.fixes.len() > MAX_DRIVER_FIXES_PER_REQUEST {
        return json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "at most {} fixes may be sent per request",
                MAX_DRIVER_FIXES_PER_REQUEST
            ),
        );
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    for (i, fix) in payload.fixes.iter().enumerate() {
        if let Err(reason) = validate_driver_fix(fix, now) {
            return json_error(StatusCode::BAD_REQUEST, format!("fixes[{}]: {}", i, reason));
        }
    }

    let mut vehicle = match Vehicle::by_assigned_driver(driver.id) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return json_error(
                StatusCode::CONFLICT,
                "This driver has no assigned vehicle — ask your dispatcher to assign one",
            )
        }
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to find the driver's vehicle: {}", err),
            )
        }
    };

    // Only the newest fix can move the vehicle; older ones in the batch would
    // be overwritten immediately.
    let newest = payload
        .fixes
        .iter()
        .max_by_key(|f| f.recorded_at)
        .expect("batch is non-empty");
    let updated = match vehicle.record_fix(newest.latitude, newest.longitude, newest.recorded_at) {
        Ok(u) => u,
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to record location: {}", err),
            )
        }
    };

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: if updated {
            "Location recorded".to_string()
        } else {
            "Location not updated: the vehicle already has a newer position".to_string()
        },
        data: Some(DriverLocationResult {
            accepted: payload.fixes.len(),
            location_updated: updated,
            vehicle_registration_number: vehicle.registration_number.clone(),
            location: vehicle.location.clone(),
        }),
    })
}

#[utoipa::path(
    get,
    path = "/api/driver/me",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "The driver the device token belongs to, their organization and assigned vehicle", body = DriverMeResponse),
        (status = 401, description = "Missing or unknown device token", body = EmptyResponse)
    )
)]
/// Who the driver's phone is paired as. Authenticated by the driver's device
/// token, like `POST /api/driver/location`. The phone app calls it when pairing
/// (to check the token and server address before saving them) and on its
/// status screen, so the driver sees their name and vehicle before the first
/// location upload. An inactive driver or one with no vehicle still gets a
/// `200` (`is_active: false` / `vehicle: null`), so the app can say why
/// location reports would be refused instead of failing to pair.
#[get("/driver/me")]
pub async fn driver_me(req: HttpRequest) -> impl Responder {
    use actix_web::http::StatusCode;

    let driver = match driver_from_device_token(&req) {
        Ok(d) => d,
        Err(resp) => return resp,
    };

    let org_name = match Organization::name_by_id(driver.org_id) {
        Ok(Some(name)) => name,
        Ok(None) => {
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "The driver's organization no longer exists")
        }
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to load the organization: {}", err),
            )
        }
    };

    let vehicle = match Vehicle::by_assigned_driver(driver.id) {
        Ok(v) => v.map(|v| DriverMeVehicle {
            registration_number: v.registration_number,
            location: v.location,
        }),
        Err(err) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to find the driver's vehicle: {}", err),
            )
        }
    };

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Driver found".to_string(),
        data: Some(DriverMe {
            driver_id: driver.id,
            name: driver.name,
            org_id: driver.org_id,
            org_name,
            is_active: driver.is_active,
            vehicle,
        }),
    })
}

#[utoipa::path(
    post,
    path = "/api/drivers/{id}/device-token/rotate",
    tag = "Drivers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Driver UUID")),
    responses(
        (status = 200, description = "A fresh device token was issued; it is shown only once", body = DriverDeviceTokenResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Driver not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
/// Issue a new device token for a driver's phone, invalidating the previous
/// one. Use it to pair a phone for the first time and again if the phone is
/// lost. The plain token is returned only in this response — the server keeps
/// just a hash — so hand it to the driver straight away. Requires an Admin or
/// Dispatcher.
#[post("/drivers/{id}/device-token/rotate")]
pub async fn rotate_driver_device_token(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let driver = match load_owned_driver(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match driver.rotate_device_token() {
        Ok(token) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "A fresh device token was issued".to_string(),
            data: Some(DriverDeviceToken {
                driver_id: driver.id,
                device_token: token,
            }),
        }),
        Err(err) => json_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to rotate device token: {}", err),
        ),
    }
}
