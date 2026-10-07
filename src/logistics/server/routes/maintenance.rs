//! Vehicle maintenance schedules and odometer (mileage) readings.

use crate::logistics::vehicle::maintenance::{VehicleMaintenance, VehicleMaintenanceError};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;
use super::vehicle_documents::ensure_owned_vehicle;

/// Create or update one preventive-maintenance schedule item. At least one
/// of `due_on` / `due_at_mileage_km` is required.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct VehicleMaintenancePayload {
    /// Free text, e.g. "Oil change" or "Full service".
    pub description: String,
    /// Due date as ISO `YYYY-MM-DD`, optional.
    #[serde(default)]
    pub due_on: Option<String>,
    /// Due odometer reading in kilometres, optional.
    #[serde(default)]
    pub due_at_mileage_km: Option<i64>,
    /// When this item was last actually serviced, as ISO `YYYY-MM-DD`.
    #[serde(default)]
    pub last_service_on: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Record the vehicle's latest odometer reading against a maintenance item.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct RecordMileagePayload {
    pub current_mileage_km: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleMaintenanceResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<VehicleMaintenance>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleMaintenanceListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<VehicleMaintenance>>,
}

/// Load a maintenance item by id and verify it belongs to `auth_org_id`, or
/// build the 403/404/500 response to return early with.
fn load_owned_vehicle_maintenance(
    item_id: Uuid,
    auth_org_id: Uuid,
) -> Result<VehicleMaintenance, HttpResponse> {
    match VehicleMaintenance::get_by_id(item_id) {
        Ok(Some(item)) if item.org_id == auth_org_id => Ok(item),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: maintenance item belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Vehicle maintenance item not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch vehicle maintenance item: {}", err),
            data: None,
        })),
    }
}

/// Map a `VehicleMaintenanceError` to a caller-facing response: a bad date
/// or a missing due criterion is a `400`, anything else a `500`.
fn vehicle_maintenance_error_response(err: VehicleMaintenanceError) -> HttpResponse {
    match err {
        VehicleMaintenanceError::InvalidDate(_) | VehicleMaintenanceError::NoDueCriterion => {
            HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: err.to_string(),
                data: None,
            })
        }
        VehicleMaintenanceError::Db(_) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to save vehicle maintenance item: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/vehicles/{reg}/maintenance",
    tag = "Vehicle maintenance",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    responses(
        (status = 200, description = "Maintenance items for the vehicle, soonest date-based due date first", body = VehicleMaintenanceListResponse),
        (status = 404, description = "Vehicle not found in this organization", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/vehicles/{reg}/maintenance")]
pub async fn list_vehicle_maintenance(path: web::Path<String>, auth: AuthenticatedOrg) -> impl Responder {
    let reg = path.into_inner();
    if let Err(resp) = ensure_owned_vehicle(&reg, auth.org_id) {
        return resp;
    }
    match VehicleMaintenance::list_by_vehicle(&reg) {
        Ok(items) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} maintenance items", items.len()),
            data: Some(items),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicle maintenance items: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/vehicles/{reg}/maintenance",
    tag = "Vehicle maintenance",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    request_body = VehicleMaintenancePayload,
    responses(
        (status = 201, description = "Maintenance item recorded", body = VehicleMaintenanceResponse),
        (status = 400, description = "A supplied date is invalid, or neither due_on nor due_at_mileage_km was given", body = EmptyResponse),
        (status = 404, description = "Vehicle not found in this organization", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/vehicles/{reg}/maintenance")]
pub async fn add_vehicle_maintenance(
    path: web::Path<String>,
    payload: web::Json<VehicleMaintenancePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let reg = path.into_inner();
    if let Err(resp) = ensure_owned_vehicle(&reg, auth.org_id) {
        return resp;
    }
    let body = payload.into_inner();
    match VehicleMaintenance::create(
        auth.org_id,
        &reg,
        body.description,
        body.due_on,
        body.due_at_mileage_km,
        body.last_service_on,
        body.notes,
    ) {
        Ok(item) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Vehicle maintenance item recorded".to_string(),
            data: Some(item),
        }),
        Err(err) => vehicle_maintenance_error_response(err),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicle-maintenance/{id}",
    tag = "Vehicle maintenance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle maintenance item UUID")),
    request_body = VehicleMaintenancePayload,
    responses(
        (status = 200, description = "Maintenance item updated", body = VehicleMaintenanceResponse),
        (status = 400, description = "A supplied date is invalid, or neither due_on nor due_at_mileage_km was given", body = EmptyResponse),
        (status = 403, description = "Item belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Item not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicle-maintenance/{id}")]
pub async fn update_vehicle_maintenance(
    path: web::Path<Uuid>,
    payload: web::Json<VehicleMaintenancePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut item = match load_owned_vehicle_maintenance(path.into_inner(), auth.org_id) {
        Ok(i) => i,
        Err(resp) => return resp,
    };
    let body = payload.into_inner();
    match item.update(body.description, body.due_on, body.due_at_mileage_km, body.last_service_on, body.notes) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Vehicle maintenance item updated".to_string(),
            data: Some(item),
        }),
        Err(err) => vehicle_maintenance_error_response(err),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicle-maintenance/{id}/mileage",
    tag = "Vehicle maintenance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle maintenance item UUID")),
    request_body = RecordMileagePayload,
    responses(
        (status = 200, description = "Odometer reading recorded", body = VehicleMaintenanceResponse),
        (status = 403, description = "Item belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Item not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicle-maintenance/{id}/mileage")]
pub async fn record_vehicle_maintenance_mileage(
    path: web::Path<Uuid>,
    payload: web::Json<RecordMileagePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut item = match load_owned_vehicle_maintenance(path.into_inner(), auth.org_id) {
        Ok(i) => i,
        Err(resp) => return resp,
    };
    match item.record_mileage(payload.into_inner().current_mileage_km) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Odometer reading recorded".to_string(),
            data: Some(item),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to record odometer reading: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/vehicle-maintenance/{id}",
    tag = "Vehicle maintenance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle maintenance item UUID")),
    responses(
        (status = 200, description = "Maintenance item deleted", body = EmptyResponse),
        (status = 403, description = "Item belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Item not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/vehicle-maintenance/{id}")]
pub async fn delete_vehicle_maintenance(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let item = match load_owned_vehicle_maintenance(path.into_inner(), auth.org_id) {
        Ok(i) => i,
        Err(resp) => return resp,
    };
    match item.delete() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Vehicle maintenance item deleted".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete vehicle maintenance item: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/vehicle-maintenance",
    tag = "Vehicle maintenance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Every maintenance item across the org's fleet, soonest date-based due date first", body = VehicleMaintenanceListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/vehicle-maintenance")]
pub async fn list_org_vehicle_maintenance(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match VehicleMaintenance::list_by_org(org_id) {
        Ok(items) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} maintenance items", items.len()),
            data: Some(items),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicle maintenance items: {}", err),
            data: None,
        }),
    }
}
