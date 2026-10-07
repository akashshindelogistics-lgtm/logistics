//! Fleet vehicles: CRUD, location updates, and the tracker-key GPS push
//! (`POST /api/track/{tracker_key}`).

use crate::logistics::orgs::orgs::Organization;
use crate::logistics::vehicle::vehicle::{Unit, Vehicle};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

/// Body a GPS tracker device sends to `POST /api/track/{tracker_key}`. No
/// address — a device only knows coordinates; the server stamps the time.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TrackLocationPayload {
    pub latitude: f64,
    pub longitude: f64,
}

/// `true` when a coordinate pair is a real point on Earth.
fn coordinates_in_range(latitude: f64, longitude: f64) -> bool {
    (-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude)
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateVehiclePayload {
    pub registration_number: String,
    pub capacity: i64,
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateVehiclePayload {
    pub capacity: i64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Vehicle>>,
}

#[utoipa::path(
    get,
    path = "/api/vehicles",
    tag = "Vehicles",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of vehicles for authenticated organization", body = VehicleListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/vehicles")]
pub async fn list_vehicles(auth: AuthenticatedOrg) -> impl Responder {
    match Vehicle::list_by_org(auth.org_id) {
        Ok(vehicles) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} vehicles", vehicles.len()),
            data: Some(vehicles),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicles: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/vehicles",
    tag = "Vehicles",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateVehiclePayload,
    responses(
        (status = 201, description = "Vehicle registered successfully", body = VehicleResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/vehicles")]
pub async fn add_vehicle(
    path: web::Path<Uuid>,
    payload: web::Json<CreateVehiclePayload>,
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
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        godowns: Vec::new(),
        location: None,
    };

    let unit = Unit::from_str(&payload.unit);
    let vehicle = Vehicle::new(&payload.registration_number, payload.capacity, unit);

    match vehicle.add_new_vehicle_to_org(&org) {
        Ok(_) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Vehicle registered successfully".to_string(),
            data: Some(vehicle),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to register vehicle: {}", err),
            data: None,
        }),
    }
}

/// Verify a vehicle with this registration number exists and belongs to
/// `auth_org_id`, or return the 403/404/500 response to bail out with.
fn check_owned_vehicle(reg: &str, auth_org_id: Uuid) -> Result<(), HttpResponse> {
    match Vehicle::org_of(reg) {
        Ok(Some(org_id)) if org_id == auth_org_id => Ok(()),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: vehicle belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Vehicle not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch vehicle: {}", err),
            data: None,
        })),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicles/{reg}",
    tag = "Vehicles",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    request_body = UpdateVehiclePayload,
    responses(
        (status = 200, description = "Vehicle updated successfully", body = VehicleResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Vehicle not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicles/{reg}")]
pub async fn edit_vehicle(
    path: web::Path<String>,
    payload: web::Json<UpdateVehiclePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let reg = path.into_inner();
    if let Err(resp) = check_owned_vehicle(&reg, auth.org_id) {
        return resp;
    }

    let mut vehicle = Vehicle::new(&reg, payload.capacity, Unit::from_str(&payload.unit));
    match vehicle.update_vehicle(payload.capacity, Unit::from_str(&payload.unit)) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Vehicle updated successfully".to_string(),
            data: Some(vehicle),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update vehicle: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicles/{reg}/location",
    tag = "Vehicles",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Vehicle location updated successfully", body = LocationResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicles/{reg}/location")]
pub async fn update_vehicle_location(
    path: web::Path<String>,
    payload: web::Json<LocationPayload>,
    _auth: AuthenticatedOrg,
) -> impl Responder {
    let reg_number = path.into_inner();
    let mut vehicle = Vehicle::new(&reg_number, 0, Unit::MetricTon);

    match vehicle.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Vehicle location updated successfully".to_string(),
            data: vehicle.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update vehicle location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/track/{tracker_key}",
    tag = "Vehicles",
    params(("tracker_key" = Uuid, Path, description = "The vehicle's GPS tracker key")),
    request_body = TrackLocationPayload,
    responses(
        (status = 200, description = "Location recorded", body = LocationResponse),
        (status = 400, description = "Coordinates out of range", body = EmptyResponse),
        (status = 404, description = "No vehicle has this tracker key", body = EmptyResponse)
    )
)]
/// Automatic location push from a GPS tracker fitted to a vehicle. The
/// `tracker_key` in the path is the whole credential — no bearer token —
/// so a device can report without holding an org login. This is the
/// unattended counterpart to `PUT /api/vehicles/{reg}/location`.
#[post("/track/{tracker_key}")]
pub async fn track_vehicle_location(
    path: web::Path<Uuid>,
    payload: web::Json<TrackLocationPayload>,
) -> impl Responder {
    if !coordinates_in_range(payload.latitude, payload.longitude) {
        return HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: "latitude must be between -90 and 90 and longitude between -180 and 180"
                .to_string(),
            data: None,
        });
    }

    let mut vehicle = match Vehicle::by_tracker_key(path.into_inner()) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<String> {
                success: false,
                message: "No vehicle has this tracker key".to_string(),
                data: None,
            })
        }
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to look up tracker key: {}", err),
                data: None,
            })
        }
    };

    match vehicle.update_location(payload.latitude, payload.longitude, None::<String>) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Location recorded".to_string(),
            data: vehicle.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to record location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/vehicles/{reg}/tracker-key/rotate",
    tag = "Vehicles",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    responses(
        (status = 200, description = "A fresh tracker key was issued", body = VehicleResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Vehicle not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
/// Issue a new GPS tracker key for a vehicle, invalidating the previous one.
/// Use this when a tracker device is lost or its key may have leaked — every
/// device then has to be reconfigured with the new key.
#[post("/vehicles/{reg}/tracker-key/rotate")]
pub async fn rotate_vehicle_tracker_key(
    path: web::Path<String>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let reg = path.into_inner();
    if let Err(resp) = check_owned_vehicle(&reg, auth.org_id) {
        return resp;
    }

    let mut vehicle = match Vehicle::list_by_org(auth.org_id) {
        Ok(vehicles) => match vehicles.into_iter().find(|v| v.registration_number == reg) {
            Some(v) => v,
            None => {
                return HttpResponse::NotFound().json(ApiResponse::<String> {
                    success: false,
                    message: "Vehicle not found".to_string(),
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

    match vehicle.rotate_tracker_key() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "A fresh tracker key was issued".to_string(),
            data: Some(vehicle),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to rotate tracker key: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/vehicles/{reg}",
    tag = "Vehicles",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    responses(
        (status = 200, description = "Vehicle deleted successfully", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/vehicles/{reg}")]
pub async fn delete_vehicle(path: web::Path<String>, _auth: AuthenticatedOrg) -> impl Responder {
    let reg_number = path.into_inner();
    let vehicle = Vehicle::new(&reg_number, 0, Unit::MetricTon);

    match vehicle.remove_vehicle() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Vehicle deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete vehicle: {}", err),
            data: None,
        }),
    }
}
