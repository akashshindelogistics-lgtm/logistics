//! Drivers (`/api/drivers`, `/api/orgs/{id}/drivers`) and assigning one to a
//! vehicle.

use crate::logistics::driver::driver::Driver;
use crate::logistics::vehicle::vehicle::Vehicle;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
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
