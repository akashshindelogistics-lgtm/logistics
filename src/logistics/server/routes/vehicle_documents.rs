//! Vehicle compliance documents (insurance, permits, fitness, ...) and their
//! expiry status.

use crate::logistics::vehicle::document::{ComplianceDocType, VehicleDocument, VehicleDocumentError};
use crate::logistics::vehicle::vehicle::Vehicle;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

/// Create or update (renew) one piece of vehicle compliance paperwork.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct VehicleDocumentPayload {
    /// `Insurance`, `RegistrationCertificate`, `Permit`, `PollutionCertificate`
    /// or `FitnessCertificate` (shorthands `RC` / `PUC` / `FC` also accepted).
    pub doc_type: String,
    /// Policy / certificate number as printed on the document.
    pub document_number: String,
    /// Issue date as ISO `YYYY-MM-DD`, optional.
    #[serde(default)]
    pub issued_on: Option<String>,
    /// Expiry date as ISO `YYYY-MM-DD`. Required.
    pub expires_on: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleDocumentResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<VehicleDocument>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleDocumentListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<VehicleDocument>>,
}

//
// Insurance / RC / permit / PUC / fitness paperwork, each with an expiry date.
// Every route checks the vehicle (or the document's stored org) belongs to the
// authenticated org before acting. See docs/vehicle-compliance.md.

/// Verify a vehicle with `reg` belongs to `auth_org_id`, or build the 404/500
/// response the handler should return early with.
pub(super) fn ensure_owned_vehicle(reg: &str, auth_org_id: Uuid) -> Result<(), HttpResponse> {
    match Vehicle::list_by_org(auth_org_id) {
        Ok(vehicles) if vehicles.iter().any(|v| v.registration_number == reg) => Ok(()),
        Ok(_) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Vehicle not found in this organization".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch vehicle: {}", err),
            data: None,
        })),
    }
}

/// Load a compliance document by id and verify it belongs to `auth_org_id`,
/// or build the 403/404/500 response to return early with.
fn load_owned_vehicle_document(
    doc_id: Uuid,
    auth_org_id: Uuid,
) -> Result<VehicleDocument, HttpResponse> {
    match VehicleDocument::get_by_id(doc_id) {
        Ok(Some(doc)) if doc.org_id == auth_org_id => Ok(doc),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: document belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Vehicle document not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch vehicle document: {}", err),
            data: None,
        })),
    }
}

/// Map a `VehicleDocumentError` to a caller-facing response: a bad date is a
/// `400`, anything else a `500`.
fn vehicle_document_error_response(err: VehicleDocumentError) -> HttpResponse {
    match err {
        VehicleDocumentError::InvalidDate(_) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: err.to_string(),
            data: None,
        }),
        VehicleDocumentError::Db(_) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to save vehicle document: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/vehicles/{reg}/documents",
    tag = "Vehicle compliance",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    responses(
        (status = 200, description = "Compliance documents for the vehicle, soonest expiry first", body = VehicleDocumentListResponse),
        (status = 404, description = "Vehicle not found in this organization", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/vehicles/{reg}/documents")]
pub async fn list_vehicle_documents(
    path: web::Path<String>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let reg = path.into_inner();
    if let Err(resp) = ensure_owned_vehicle(&reg, auth.org_id) {
        return resp;
    }
    match VehicleDocument::list_by_vehicle(&reg) {
        Ok(docs) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} documents", docs.len()),
            data: Some(docs),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicle documents: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/vehicles/{reg}/documents",
    tag = "Vehicle compliance",
    security(("bearer_auth" = [])),
    params(("reg" = String, Path, description = "Vehicle registration number")),
    request_body = VehicleDocumentPayload,
    responses(
        (status = 201, description = "Document recorded", body = VehicleDocumentResponse),
        (status = 400, description = "A supplied date is not a valid ISO date", body = EmptyResponse),
        (status = 404, description = "Vehicle not found in this organization", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/vehicles/{reg}/documents")]
pub async fn add_vehicle_document(
    path: web::Path<String>,
    payload: web::Json<VehicleDocumentPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let reg = path.into_inner();
    if let Err(resp) = ensure_owned_vehicle(&reg, auth.org_id) {
        return resp;
    }
    let body = payload.into_inner();
    match VehicleDocument::create(
        auth.org_id,
        &reg,
        ComplianceDocType::from_str(&body.doc_type),
        body.document_number,
        body.issued_on,
        body.expires_on,
        body.notes,
    ) {
        Ok(doc) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Vehicle document recorded".to_string(),
            data: Some(doc),
        }),
        Err(err) => vehicle_document_error_response(err),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicle-documents/{id}",
    tag = "Vehicle compliance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle document UUID")),
    request_body = VehicleDocumentPayload,
    responses(
        (status = 200, description = "Document updated / renewed", body = VehicleDocumentResponse),
        (status = 400, description = "A supplied date is not a valid ISO date", body = EmptyResponse),
        (status = 403, description = "Document belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Document not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vehicle-documents/{id}")]
pub async fn update_vehicle_document(
    path: web::Path<Uuid>,
    payload: web::Json<VehicleDocumentPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut doc = match load_owned_vehicle_document(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    let body = payload.into_inner();
    match doc.update(
        ComplianceDocType::from_str(&body.doc_type),
        body.document_number,
        body.issued_on,
        body.expires_on,
        body.notes,
    ) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Vehicle document updated".to_string(),
            data: Some(doc),
        }),
        Err(err) => vehicle_document_error_response(err),
    }
}

#[utoipa::path(
    delete,
    path = "/api/vehicle-documents/{id}",
    tag = "Vehicle compliance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vehicle document UUID")),
    responses(
        (status = 200, description = "Document deleted", body = EmptyResponse),
        (status = 403, description = "Document belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Document not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/vehicle-documents/{id}")]
pub async fn delete_vehicle_document(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let doc = match load_owned_vehicle_document(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match doc.delete() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Vehicle document deleted".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete vehicle document: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/vehicle-documents",
    tag = "Vehicle compliance",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Every compliance document across the org's fleet, soonest expiry first", body = VehicleDocumentListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/vehicle-documents")]
pub async fn list_org_vehicle_documents(
    path: web::Path<Uuid>,
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
    match VehicleDocument::list_by_org(org_id) {
        Ok(docs) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} documents", docs.len()),
            data: Some(docs),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicle documents: {}", err),
            data: None,
        }),
    }
}
