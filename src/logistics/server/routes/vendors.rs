//! Vehicle vendors — the third parties an org hires vehicles from.

use crate::logistics::vendor::hire::VehicleHire;
use crate::logistics::vendor::vendor::{VehicleVendor, VendorError, VendorInput};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

/// Create a vehicle vendor (transporter / broker the org hires trucks from).
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateVendorPayload {
    pub name: String,
    #[serde(default)]
    pub contact_person: Option<String>,
    pub phone: String,
    /// Optional 15-character GSTIN.
    #[serde(default)]
    pub gstin: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateVendorPayload {
    pub name: String,
    #[serde(default)]
    pub contact_person: Option<String>,
    pub phone: String,
    #[serde(default)]
    pub gstin: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Inactive vendors stay listed but can't be picked for a new hire.
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VendorResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<VehicleVendor>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VendorListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<VehicleVendor>>,
}

/// Load a vendor by id, returning an error `HttpResponse` unless it exists
/// and belongs to the caller's organization.
fn load_owned_vendor(vendor_id: Uuid, auth_org_id: Uuid) -> Result<VehicleVendor, HttpResponse> {
    match VehicleVendor::get_by_id(vendor_id) {
        Ok(Some(vendor)) if vendor.org_id == auth_org_id => Ok(vendor),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: vendor belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Vendor not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch vendor: {}", err),
            data: None,
        })),
    }
}

fn vendor_error_response(action: &str, err: VendorError) -> HttpResponse {
    match err {
        VendorError::InvalidInput(msg) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: msg,
            data: None,
        }),
        VendorError::Db(e) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to {action} vendor: {e}"),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/vendors",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The organization's vehicle vendors, active first", body = VendorListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/vendors")]
pub async fn list_vendors(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match VehicleVendor::list_by_org(org_id) {
        Ok(vendors) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} vendors", vendors.len()),
            data: Some(vendors),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vendors: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/vendors",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateVendorPayload,
    responses(
        (status = 201, description = "Vendor created", body = VendorResponse),
        (status = 400, description = "Blank name/phone or malformed GSTIN", body = EmptyResponse),
        (status = 403, description = "Forbidden (other org, or not Admin/Dispatcher)", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/vendors")]
pub async fn add_vendor(
    path: web::Path<Uuid>,
    payload: web::Json<CreateVendorPayload>,
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
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let payload = payload.into_inner();
    let input = VendorInput {
        name: payload.name,
        contact_person: payload.contact_person,
        phone: payload.phone,
        gstin: payload.gstin,
        notes: payload.notes,
    };
    match VehicleVendor::create(org_id, input) {
        Ok(vendor) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Vendor created successfully".to_string(),
            data: Some(vendor),
        }),
        Err(err) => vendor_error_response("create", err),
    }
}

#[utoipa::path(
    put,
    path = "/api/vendors/{id}",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vendor UUID")),
    request_body = UpdateVendorPayload,
    responses(
        (status = 200, description = "Vendor updated", body = VendorResponse),
        (status = 400, description = "Blank name/phone or malformed GSTIN", body = EmptyResponse),
        (status = 403, description = "Forbidden (other org, or not Admin/Dispatcher)", body = EmptyResponse),
        (status = 404, description = "Vendor not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/vendors/{id}")]
pub async fn update_vendor(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateVendorPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut vendor = match load_owned_vendor(path.into_inner(), auth.org_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let payload = payload.into_inner();
    let input = VendorInput {
        name: payload.name,
        contact_person: payload.contact_person,
        phone: payload.phone,
        gstin: payload.gstin,
        notes: payload.notes,
    };
    match vendor.update(input, payload.is_active) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Vendor updated successfully".to_string(),
            data: Some(vendor),
        }),
        Err(err) => vendor_error_response("update", err),
    }
}

#[utoipa::path(
    delete,
    path = "/api/vendors/{id}",
    tag = "Vehicle vendors",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Vendor UUID")),
    responses(
        (status = 200, description = "Vendor deleted", body = EmptyResponse),
        (status = 409, description = "Vendor has hire history; deactivate it instead", body = EmptyResponse),
        (status = 403, description = "Forbidden (other org, or not Admin/Dispatcher)", body = EmptyResponse),
        (status = 404, description = "Vendor not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/vendors/{id}")]
pub async fn delete_vendor(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let vendor = match load_owned_vendor(path.into_inner(), auth.org_id) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    match VehicleHire::exists_for_vendor(vendor.id) {
        Ok(true) => {
            return HttpResponse::Conflict().json(ApiResponse::<String> {
                success: false,
                message: format!(
                    "{} has hire history and can't be deleted; deactivate it instead",
                    vendor.name
                ),
                data: None,
            })
        }
        Ok(false) => {}
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to check vendor hires: {}", err),
                data: None,
            })
        }
    }
    match vendor.delete() {
        Ok(()) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Vendor deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete vendor: {}", err),
            data: None,
        }),
    }
}
