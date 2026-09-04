use crate::logistics::auth::auth::{
    decode_token, generate_token, OrgCredentials, OrgSummary,
};
use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::{
    DispatchLineItem, DispatchLineItemInput, DispatchOrder, DispatchStatus, DispatchStatusEvent,
    ProofOfDelivery, ProofOfDeliveryInput,
};
use crate::logistics::driver::driver::Driver;
use crate::logistics::godown::godown::Godown;
use crate::logistics::godown::transfer::{StockTransfer, TransferError};
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::stock::stock::Stock;
use crate::logistics::vehicle::document::{
    ComplianceDocType, ComplianceStatus, VehicleDocument, VehicleDocumentError,
};
use crate::logistics::vehicle::vehicle::{Location, Unit, Vehicle};
use actix_web::{delete, dev::Payload, get, post, put, web, FromRequest, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

// ── Auth extractor ───────────────────────────────────────────────────────────

pub struct AuthenticatedOrg {
    pub org_id: Uuid,
    pub org_name: String,
}

impl FromRequest for AuthenticatedOrg {
    type Error = actix_web::Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let auth_header = req.headers().get("Authorization");

        let token = match auth_header {
            None => {
                return ready(Err(actix_web::error::ErrorUnauthorized(
                    "Missing Authorization header",
                )))
            }
            Some(val) => match val.to_str() {
                Ok(s) if s.starts_with("Bearer ") => s[7..].to_string(),
                _ => {
                    return ready(Err(actix_web::error::ErrorUnauthorized(
                        "Invalid Authorization header format",
                    )))
                }
            },
        };

        match decode_token(&token) {
            Ok(claims) => match Uuid::parse_str(&claims.org_id) {
                Ok(org_id) => ready(Ok(AuthenticatedOrg {
                    org_id,
                    org_name: claims.org_name,
                })),
                Err(_) => ready(Err(actix_web::error::ErrorUnauthorized(
                    "Invalid org_id in token",
                ))),
            },
            Err(_) => ready(Err(actix_web::error::ErrorUnauthorized(
                "Invalid or expired token",
            ))),
        }
    }
}

// ── Payload types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateOrgPayload {
    pub name: String,
    pub address: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateOrgPayload {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LocationPayload {
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
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

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
    /// Optional reorder point — the item is flagged (`below_threshold`) once
    /// `quantity` drops under this.
    #[serde(default)]
    pub reorder_threshold: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
    /// Optional reorder point. Sending `null` (or omitting it) clears any
    /// existing threshold.
    #[serde(default)]
    pub reorder_threshold: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateGodownPayload {
    pub name: String,
    pub address: String,
    /// Optional total volume cap for the godown, in the same units as a
    /// stock item's `volume_in_size * quantity`. Omit for no limit.
    #[serde(default)]
    pub max_capacity: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateGodownPayload {
    pub name: String,
    pub address: String,
    /// Optional total volume cap. Sending `null` (or omitting it) removes an
    /// existing cap.
    #[serde(default)]
    pub max_capacity: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TransferStockPayload {
    /// Godown to move the stock into. Must belong to the same organization as
    /// the source godown in the path and be a different godown.
    pub to_godown_id: Uuid,
    /// Description of the stock item to move, as held in the source godown.
    pub description: String,
    /// Number of units to move. Must be positive and not exceed what the
    /// source godown holds.
    pub quantity: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateCustomerPayload {
    pub name: String,
    pub address: String,
}

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

/// One line on a dispatch request: a stock description and how many units of
/// it to send.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DispatchLineItemPayload {
    pub stock_description: String,
    pub requested_quantity: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DispatchRequestPayload {
    pub customer_id: Uuid,
    /// The stock lines this shipment carries. Must contain at least one item;
    /// a description may not be repeated.
    pub line_items: Vec<DispatchLineItemPayload>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateDispatchStatusPayload {
    pub status: DispatchStatus,
    /// Required when `status` is `DELIVERED`; rejected with `400` if
    /// missing. Ignored for every other status.
    #[serde(default)]
    pub proof_of_delivery: Option<ProofOfDeliveryPayload>,
    /// Only used when `status` is `RETURNED`: the godown that should receive
    /// the returned stock. Optional — the server falls back to a godown that
    /// already holds one of the returned items, or the org's first godown.
    #[serde(default)]
    pub return_to_godown_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ProofOfDeliveryPayload {
    pub receiver_name: String,
    pub signature_or_photo_url: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LoginPayload {
    pub org_id: Uuid,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginData {
    pub token: String,
    pub org_id: String,
    pub org_name: String,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T: ToSchema> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrgResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Organization>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vehicle>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StockResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Stock>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GodownResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Godown>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GodownListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Godown>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StockTransferResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<StockTransfer>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StockTransferListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<StockTransfer>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Customer>,
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

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DispatchOrderResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DispatchOrder>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LocationResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Location>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmptyResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrgListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Organization>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Vehicle>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Customer>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DispatchOrderListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<DispatchOrder>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrgSummaryListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<OrgSummary>>,
}

// ── Auth handlers (public) ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/auth/orgs",
    tag = "Auth",
    responses(
        (status = 200, description = "List of registered organizations for login", body = OrgSummaryListResponse)
    )
)]
#[get("/auth/orgs")]
pub async fn auth_orgs() -> impl Responder {
    match OrgCredentials::list_summaries() {
        Ok(summaries) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} organizations", summaries.len()),
            data: Some(summaries),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list organizations: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "Auth",
    request_body = LoginPayload,
    responses(
        (status = 200, description = "Login successful, returns JWT token", body = OrgResponse),
        (status = 401, description = "Invalid credentials", body = EmptyResponse)
    )
)]
#[post("/auth/login")]
pub async fn auth_login(payload: web::Json<LoginPayload>) -> impl Responder {
    match OrgCredentials::verify_login(payload.org_id, &payload.password) {
        Ok(Some(org_name)) => {
            match generate_token(payload.org_id, &org_name) {
                Ok(token) => HttpResponse::Ok().json(ApiResponse {
                    success: true,
                    message: "Login successful".to_string(),
                    data: Some(LoginData {
                        token,
                        org_id: payload.org_id.to_string(),
                        org_name,
                    }),
                }),
                Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
                    success: false,
                    message: format!("Failed to generate token: {}", err),
                    data: None,
                }),
            }
        }
        Ok(None) => HttpResponse::Unauthorized().json(ApiResponse::<String> {
            success: false,
            message: "Invalid organization ID or password".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Authentication error: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "Auth",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Current authenticated organization", body = OrgResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse),
        (status = 404, description = "Organization not found", body = EmptyResponse)
    )
)]
#[get("/auth/me")]
pub async fn auth_me(auth: AuthenticatedOrg) -> impl Responder {
    match Organization::get_by_id(auth.org_id) {
        Ok(Some(org)) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Authenticated organization retrieved".to_string(),
            data: Some(org),
        }),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Organization not found".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to retrieve organization: {}", err),
            data: None,
        }),
    }
}

// ── Health (public) ───────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "Health",
    responses(
        (status = 200, description = "System is operational", body = EmptyResponse)
    )
)]
#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::<String> {
        success: true,
        message: "Logistics system REST API operational".to_string(),
        data: None,
    })
}

// ── Organization handlers (protected) ─────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/orgs",
    tag = "Organizations",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Authenticated organization", body = OrgListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs")]
pub async fn list_orgs(auth: AuthenticatedOrg) -> impl Responder {
    match Organization::get_by_id(auth.org_id) {
        Ok(Some(org)) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Retrieved organization".to_string(),
            data: Some(vec![org]),
        }),
        Ok(None) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "No organization found".to_string(),
            data: Some(Vec::<Organization>::new()),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to retrieve organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}",
    tag = "Organizations",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Organization detail with vehicles and stock", body = OrgResponse),
        (status = 403, description = "Forbidden: can only access your own organization", body = EmptyResponse),
        (status = 404, description = "Organization not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}")]
pub async fn get_org(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only view your own organization".to_string(),
            data: None,
        });
    }
    match Organization::get_by_id(org_id) {
        Ok(Some(org)) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Organization retrieved successfully".to_string(),
            data: Some(org),
        }),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Organization not found".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to retrieve organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs",
    tag = "Organizations",
    request_body = CreateOrgPayload,
    responses(
        (status = 201, description = "Organization created successfully", body = OrgResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[post("/orgs")]
pub async fn create_org(payload: web::Json<CreateOrgPayload>) -> impl Responder {
    match Organization::create_organization(&payload.name, &payload.address) {
        Ok(org) => {
            if let Err(err) = OrgCredentials::create(org.id, &org.name, &payload.password) {
                return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                    success: false,
                    message: format!("Failed to save credentials: {}", err),
                    data: None,
                });
            }
            HttpResponse::Created().json(ApiResponse {
                success: true,
                message: "Organization created successfully".to_string(),
                data: Some(org),
            })
        }
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/orgs/{id}",
    tag = "Organizations",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = UpdateOrgPayload,
    responses(
        (status = 200, description = "Organization updated successfully", body = OrgResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}")]
pub async fn update_org(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateOrgPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only update your own organization".to_string(),
            data: None,
        });
    }
    let mut org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        godowns: Vec::new(),
        location: None,
    };

    match org.update_organization(&payload.name, &payload.address) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Organization updated successfully".to_string(),
            data: Some(org),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/orgs/{id}/location",
    tag = "Organizations",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Organization location updated successfully", body = LocationResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}/location")]
pub async fn update_org_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
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
    let mut org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        godowns: Vec::new(),
        location: None,
    };

    match org.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Organization location updated successfully".to_string(),
            data: org.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update organization location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/orgs/{id}",
    tag = "Organizations",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Organization deleted successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/orgs/{id}")]
pub async fn delete_org(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only delete your own organization".to_string(),
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

    match org.remove_organization() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Organization deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete organization: {}", err),
            data: None,
        }),
    }
}

// ── Vehicle handlers (protected) ──────────────────────────────────────────────

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

// ── Driver handlers (protected) ───────────────────────────────────────────────

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

// ── Vehicle compliance document handlers (protected) ─────────────────────────
//
// Insurance / RC / permit / PUC / fitness paperwork, each with an expiry date.
// Every route checks the vehicle (or the document's stored org) belongs to the
// authenticated org before acting. See docs/vehicle-compliance.md.

/// Verify a vehicle with `reg` belongs to `auth_org_id`, or build the 404/500
/// response the handler should return early with.
fn ensure_owned_vehicle(reg: &str, auth_org_id: Uuid) -> Result<(), HttpResponse> {
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

// ── Godown handlers (protected) ───────────────────────────────────────────────
//
// Stock is held in a godown, not directly on the organization. Every
// `/api/godowns/{gid}...` route loads the godown and checks it belongs to the
// authenticated org before acting on it. See docs/godowns.md.

/// Load a godown by id and verify it belongs to `auth_org_id`, or build the
/// 403/404/500 response a handler should return early with.
fn load_owned_godown(godown_id: Uuid, auth_org_id: Uuid) -> Result<Godown, HttpResponse> {
    match Godown::get_by_id(godown_id) {
        Ok(Some(godown)) if godown.org_id == auth_org_id => Ok(godown),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: godown belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Godown not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch godown: {}", err),
            data: None,
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/godowns",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "List of godowns for the organization", body = GodownListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/godowns")]
pub async fn list_godowns(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match Godown::list_by_org(org_id) {
        Ok(godowns) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} godowns", godowns.len()),
            data: Some(godowns),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list godowns: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/godowns",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateGodownPayload,
    responses(
        (status = 201, description = "Godown created successfully", body = GodownResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/godowns")]
pub async fn create_godown(
    path: web::Path<Uuid>,
    payload: web::Json<CreateGodownPayload>,
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
    match Godown::create(org_id, &payload.name, &payload.address, payload.max_capacity) {
        Ok(godown) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Godown created successfully".to_string(),
            data: Some(godown),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create godown: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/godowns/{gid}",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Godown UUID")),
    responses(
        (status = 200, description = "Godown with its stock", body = GodownResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/godowns/{gid}")]
pub async fn get_godown(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let godown = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };
    HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Godown retrieved successfully".to_string(),
        data: Some(godown),
    })
}

#[utoipa::path(
    put,
    path = "/api/godowns/{gid}",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Godown UUID")),
    request_body = UpdateGodownPayload,
    responses(
        (status = 200, description = "Godown updated successfully", body = GodownResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/godowns/{gid}")]
pub async fn update_godown(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateGodownPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut godown = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };
    match godown.update(&payload.name, &payload.address, payload.max_capacity) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Godown updated successfully".to_string(),
            data: Some(godown),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update godown: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/godowns/{gid}",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Godown UUID")),
    responses(
        (status = 200, description = "Godown deleted successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/godowns/{gid}")]
pub async fn delete_godown(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let godown = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };
    match godown.remove() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Godown deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete godown: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/godowns/{gid}/location",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Godown UUID")),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Godown location updated successfully", body = LocationResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/godowns/{gid}/location")]
pub async fn update_godown_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut godown = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };
    match godown.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Godown location updated successfully".to_string(),
            data: godown.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update godown location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/godowns/{gid}/stock",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Godown UUID")),
    request_body = CreateStockPayload,
    responses(
        (status = 201, description = "Stock added successfully", body = StockResponse),
        (status = 409, description = "Would exceed the godown's max_capacity", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/godowns/{gid}/stock")]
pub async fn add_godown_stock(
    path: web::Path<Uuid>,
    payload: web::Json<CreateStockPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let godown = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };

    let incoming_volume = payload.volume_in_size.saturating_mul(payload.quantity);
    if let Err(msg) = godown.check_capacity_for(incoming_volume, None) {
        return HttpResponse::Conflict().json(ApiResponse::<String> {
            success: false,
            message: msg,
            data: None,
        });
    }

    let stock = Stock::new(
        payload.volume_in_size,
        payload.quantity,
        &payload.description,
    )
    .with_reorder_threshold(payload.reorder_threshold);

    match stock.add_to_godown(godown.id) {
        Ok(_) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Stock added successfully".to_string(),
            data: Some(stock),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to add stock: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/godowns/{gid}/stock",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Godown UUID")),
    request_body = UpdateStockPayload,
    responses(
        (status = 200, description = "Stock updated successfully", body = StockResponse),
        (status = 409, description = "Would exceed the godown's max_capacity", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/godowns/{gid}/stock")]
pub async fn update_godown_stock(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateStockPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let godown = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };

    let incoming_volume = payload.volume_in_size.saturating_mul(payload.quantity);
    if let Err(msg) = godown.check_capacity_for(incoming_volume, Some(&payload.description)) {
        return HttpResponse::Conflict().json(ApiResponse::<String> {
            success: false,
            message: msg,
            data: None,
        });
    }

    let mut stock = Stock::new(0, 0, &payload.description);
    match stock.update_in_godown(
        godown.id,
        payload.volume_in_size,
        payload.quantity,
        payload.reorder_threshold,
    ) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Stock updated successfully".to_string(),
            data: Some(stock),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update stock: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/godowns/{gid}/stock/{desc}",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(
        ("gid" = Uuid, Path, description = "Godown UUID"),
        ("desc" = String, Path, description = "Stock item description")
    ),
    responses(
        (status = 200, description = "Stock removed successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Godown not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/godowns/{gid}/stock/{desc}")]
pub async fn delete_godown_stock(
    path: web::Path<(Uuid, String)>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let (godown_id, desc) = path.into_inner();
    let godown = match load_owned_godown(godown_id, auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };
    let stock = Stock::new(0, 0, &desc);
    match stock.remove_from_godown(godown.id) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Stock removed successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to remove stock: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/godowns/{gid}/transfer",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("gid" = Uuid, Path, description = "Source godown UUID")),
    request_body = TransferStockPayload,
    responses(
        (status = 201, description = "Stock transferred and recorded", body = StockTransferResponse),
        (status = 400, description = "Same godown, missing item, or not enough stock", body = EmptyResponse),
        (status = 409, description = "Would exceed the destination godown's max_capacity", body = EmptyResponse),
        (status = 403, description = "A godown belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "A godown was not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/godowns/{gid}/transfer")]
pub async fn transfer_godown_stock(
    path: web::Path<Uuid>,
    payload: web::Json<TransferStockPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let from = match load_owned_godown(path.into_inner(), auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };
    let to = match load_owned_godown(payload.to_godown_id, auth.org_id) {
        Ok(g) => g,
        Err(resp) => return resp,
    };

    match StockTransfer::execute(&from, &to, &payload.description, payload.quantity) {
        Ok(transfer) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: format!(
                "Moved {} units of {} from {} to {}",
                transfer.quantity, transfer.description, from.name, to.name
            ),
            data: Some(transfer),
        }),
        Err(TransferError::DestinationCapacity(msg)) => {
            HttpResponse::Conflict().json(ApiResponse::<String> {
                success: false,
                message: msg,
                data: None,
            })
        }
        Err(err @ TransferError::Db(_)) => {
            HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to transfer stock: {}", err),
                data: None,
            })
        }
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: err.to_string(),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/stock-transfers",
    tag = "Godowns",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Godown-to-godown transfer history, most recent first", body = StockTransferListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/stock-transfers")]
pub async fn list_stock_transfers(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match StockTransfer::list_by_org(org_id) {
        Ok(transfers) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} stock transfers", transfers.len()),
            data: Some(transfers),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list stock transfers: {}", err),
            data: None,
        }),
    }
}

// ── Customer handlers (protected) ─────────────────────────────────────────────

/// Load a customer by id and verify it belongs to `auth_org_id`, or build the
/// 403/404/500 response a handler should return early with. Mirrors
/// `load_owned_driver`.
fn load_owned_customer(customer_id: Uuid, auth_org_id: Uuid) -> Result<Customer, HttpResponse> {
    match Customer::get_by_id(customer_id) {
        Ok(Some(customer)) if customer.org_id == auth_org_id => Ok(customer),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: customer belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Customer not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch customer: {}", err),
            data: None,
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/customers",
    tag = "Customers",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Customers for the authenticated organization", body = CustomerListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/customers")]
pub async fn list_customers(auth: AuthenticatedOrg) -> impl Responder {
    match Customer::list_by_org(auth.org_id) {
        Ok(customers) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} customers", customers.len()),
            data: Some(customers),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list customers: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/customers",
    tag = "Customers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateCustomerPayload,
    responses(
        (status = 201, description = "Customer created successfully", body = CustomerResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/customers")]
pub async fn add_customer(
    path: web::Path<Uuid>,
    payload: web::Json<CreateCustomerPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only add customers to your own organization"
                .to_string(),
            data: None,
        });
    }
    match Customer::create_customer(org_id, &payload.name, &payload.address) {
        Ok(customer) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Customer created successfully".to_string(),
            data: Some(customer),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create customer: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/customers/{id}/location",
    tag = "Customers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Customer UUID")),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Customer location updated successfully", body = LocationResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Customer not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/customers/{id}/location")]
pub async fn update_customer_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut customer = match load_owned_customer(path.into_inner(), auth.org_id) {
        Ok(c) => c,
        Err(resp) => return resp,
    };

    match customer.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Customer location updated successfully".to_string(),
            data: customer.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update customer location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/customers/{id}",
    tag = "Customers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Customer UUID")),
    responses(
        (status = 200, description = "Customer deleted successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Customer not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/customers/{id}")]
pub async fn delete_customer(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let customer = match load_owned_customer(path.into_inner(), auth.org_id) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    match customer.delete() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Customer deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete customer: {}", err),
            data: None,
        }),
    }
}

// ── Dispatch handlers (protected) ─────────────────────────────────────────────

/// Load a dispatch by id and verify it belongs to `auth_org_id`, or build the
/// 403/404/500 response a handler should return early with.
fn load_owned_dispatch(
    dispatch_id: Uuid,
    auth_org_id: Uuid,
) -> Result<DispatchOrder, HttpResponse> {
    match DispatchOrder::get_by_id(dispatch_id) {
        Ok(Some(dispatch)) if dispatch.org_id == auth_org_id => Ok(dispatch),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: dispatch belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Dispatch order not found".to_string(),
            data: None,
        })),
        Err(err) => Err(
            HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch dispatch: {}", err),
                data: None,
            }),
        ),
    }
}

#[utoipa::path(
    get,
    path = "/api/dispatches",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of dispatch orders for authenticated organization", body = DispatchOrderListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/dispatches")]
pub async fn list_dispatches(auth: AuthenticatedOrg) -> impl Responder {
    match DispatchOrder::list_by_org(auth.org_id) {
        Ok(orders) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} dispatch orders", orders.len()),
            data: Some(orders),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list dispatch orders: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/dispatch",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = DispatchRequestPayload,
    responses(
        (status = 200, description = "Stock dispatched successfully", body = DispatchOrderResponse),
        (status = 400, description = "Dispatch request failed", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/dispatch")]
pub async fn dispatch_stock(
    path: web::Path<Uuid>,
    payload: web::Json<DispatchRequestPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only dispatch from your own organization".to_string(),
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

    let customer = match Customer::get_by_id(payload.customer_id) {
        Ok(Some(c)) if c.org_id == auth.org_id => c,
        Ok(Some(_)) => {
            return HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: "Customer belongs to a different organization".to_string(),
                data: None,
            })
        }
        Ok(None) => {
            return HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: "Customer not found".to_string(),
                data: None,
            })
        }
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch customer: {}", err),
                data: None,
            })
        }
    };

    let line_items: Vec<DispatchLineItemInput> = payload
        .line_items
        .iter()
        .map(|li| DispatchLineItemInput {
            stock_description: li.stock_description.clone(),
            requested_quantity: li.requested_quantity,
        })
        .collect();

    match org.dispatch_stock_to_customer(&customer, &line_items) {
        Ok(order) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Stock dispatched successfully".to_string(),
            data: Some(order),
        }),
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: format!("Dispatch failed: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/dispatches/{id}/status",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Dispatch order UUID")),
    request_body = UpdateDispatchStatusPayload,
    responses(
        (status = 200, description = "Dispatch status updated", body = DispatchOrderResponse),
        (status = 400, description = "Illegal transition for the dispatch's current status", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Dispatch not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/dispatches/{id}/status")]
pub async fn update_dispatch_status(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateDispatchStatusPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut dispatch = match load_owned_dispatch(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };

    let proof = payload
        .proof_of_delivery
        .as_ref()
        .map(|p| ProofOfDeliveryInput {
            receiver_name: p.receiver_name.clone(),
            signature_or_photo_url: p.signature_or_photo_url.clone(),
        });

    match dispatch.transition_to(payload.status, proof, payload.return_to_godown_id) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Dispatch status updated to {}", dispatch.status),
            data: Some(dispatch),
        }),
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: format!("Status update failed: {}", err),
            data: None,
        }),
    }
}

// ── AI dispatch summary (protected) ──────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/dispatches/{id}/summary",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Dispatch order UUID")),
    responses(
        (status = 200, description = "AI-generated plain-English status summary", body = EmptyResponse),
        (status = 404, description = "Dispatch not found", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/dispatches/{id}/summary")]
pub async fn get_dispatch_summary(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let dispatch = match load_owned_dispatch(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };

    let customer = match Customer::get_by_id(dispatch.customer_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<String> {
                success: false,
                message: "Customer for this dispatch not found".to_string(),
                data: None,
            })
        }
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch customer: {}", err),
                data: None,
            })
        }
    };

    match crate::logistics::ai::status::generate_dispatch_summary(&dispatch, &customer).await {
        Ok(summary) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Summary generated".to_string(),
            data: Some(summary),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to generate summary: {}", err),
            data: None,
        }),
    }
}

// ── OpenAPI + routing ─────────────────────────────────────────────────────────

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    servers(
        (url = "http://127.0.0.1:8080", description = "Local development server"),
    ),
    paths(
        health_check,
        auth_orgs,
        auth_login,
        auth_me,
        list_orgs,
        get_org,
        create_org,
        update_org,
        update_org_location,
        delete_org,
        list_vehicles,
        add_vehicle,
        edit_vehicle,
        update_vehicle_location,
        delete_vehicle,
        list_drivers,
        add_driver,
        update_driver,
        delete_driver,
        assign_vehicle_driver,
        list_vehicle_documents,
        add_vehicle_document,
        update_vehicle_document,
        delete_vehicle_document,
        list_org_vehicle_documents,
        list_godowns,
        create_godown,
        get_godown,
        update_godown,
        delete_godown,
        update_godown_location,
        add_godown_stock,
        update_godown_stock,
        delete_godown_stock,
        transfer_godown_stock,
        list_stock_transfers,
        list_customers,
        add_customer,
        update_customer_location,
        delete_customer,
        list_dispatches,
        dispatch_stock,
        update_dispatch_status,
        get_dispatch_summary,
    ),
    components(
        schemas(
            LoginPayload, LoginData,
            CreateOrgPayload, UpdateOrgPayload, LocationPayload,
            CreateVehiclePayload, UpdateVehiclePayload, CreateStockPayload, UpdateStockPayload,
            CreateGodownPayload, UpdateGodownPayload,
            CreateCustomerPayload, DispatchRequestPayload, DispatchLineItemPayload,
            CreateDriverPayload, UpdateDriverPayload, AssignDriverPayload,
            VehicleDocumentPayload,
            TransferStockPayload,
            UpdateDispatchStatusPayload, ProofOfDeliveryPayload,
            Organization, Vehicle, Unit, Location, Stock, Godown, StockTransfer, Customer, Driver,
            VehicleDocument, ComplianceDocType, ComplianceStatus,
            DispatchOrder, DispatchLineItem, DispatchStatus, DispatchStatusEvent, ProofOfDelivery,
            OrgSummary,
            OrgResponse, OrgListResponse, VehicleResponse, VehicleListResponse,
            VehicleDocumentResponse, VehicleDocumentListResponse,
            StockResponse, GodownResponse, GodownListResponse,
            StockTransferResponse, StockTransferListResponse,
            CustomerResponse, CustomerListResponse,
            DriverResponse, DriverListResponse,
            DispatchOrderResponse, DispatchOrderListResponse,
            LocationResponse, OrgSummaryListResponse, EmptyResponse,
        )
    ),
    tags(
        (name = "Health", description = "Health check"),
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Organizations", description = "Organization management"),
        (name = "Vehicles", description = "Vehicle fleet management"),
        (name = "Drivers", description = "Driver records and vehicle assignment"),
        (name = "Vehicle compliance", description = "Vehicle paperwork (insurance, RC, permit, PUC, fitness) and expiry tracking"),
        (name = "Godowns", description = "Warehouse (godown) and stock management"),
        (name = "Customers", description = "Customer management"),
        (name = "Dispatch", description = "Stock dispatch"),
    )
)]
pub struct ApiDoc;

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(health_check)
            .service(auth_orgs)
            .service(auth_login)
            .service(auth_me)
            .service(list_orgs)
            .service(get_org)
            .service(create_org)
            .service(update_org)
            .service(update_org_location)
            .service(delete_org)
            .service(list_vehicles)
            .service(add_vehicle)
            .service(edit_vehicle)
            .service(update_vehicle_location)
            .service(delete_vehicle)
            .service(list_drivers)
            .service(add_driver)
            .service(update_driver)
            .service(delete_driver)
            .service(assign_vehicle_driver)
            .service(list_vehicle_documents)
            .service(add_vehicle_document)
            .service(update_vehicle_document)
            .service(delete_vehicle_document)
            .service(list_org_vehicle_documents)
            .service(list_godowns)
            .service(create_godown)
            .service(get_godown)
            .service(update_godown)
            .service(delete_godown)
            .service(update_godown_location)
            .service(add_godown_stock)
            .service(update_godown_stock)
            .service(delete_godown_stock)
            .service(transfer_godown_stock)
            .service(list_stock_transfers)
            .service(list_customers)
            .service(add_customer)
            .service(update_customer_location)
            .service(delete_customer)
            .service(list_dispatches)
            .service(dispatch_stock)
            .service(update_dispatch_status)
            .service(get_dispatch_summary),
    )
    .service(
        SwaggerUi::new("/swagger-ui/{_:.*}")
            .url("/api-docs/openapi.json", ApiDoc::openapi()),
    );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::test_support::TestDb;
    use actix_web::{test, App};
    use crate::logistics::auth::auth::generate_token;

    fn make_auth_header(org_id: Uuid, org_name: &str) -> String {
        let token = generate_token(org_id, org_name).expect("Failed to generate test token");
        format!("Bearer {}", token)
    }

    // ── Health ────────────────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_health_check_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Logistics system REST API operational");
    }

    #[actix_web::test]
    async fn test_swagger_ui_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/swagger-ui/").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success() || resp.status().is_redirection());
    }

    #[actix_web::test]
    async fn test_openapi_json_spec_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api-docs/openapi.json").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    // ── Auth: login ───────────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_auth_login_with_valid_credentials_returns_token() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Login Test Org".to_string(),
            address: "1 Auth Road".to_string(),
            password: "login_pass_123".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org_id = body.data.unwrap().id;

        let login_payload = LoginPayload {
            org_id,
            password: "login_pass_123".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(&login_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<LoginData> = test::read_body_json(resp).await;
        assert!(body.success);
        assert!(!body.data.unwrap().token.is_empty());
    }

    #[actix_web::test]
    async fn test_auth_login_with_wrong_password_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Wrong Pass Org".to_string(),
            address: "2 Auth Road".to_string(),
            password: "correct_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org_id = body.data.unwrap().id;

        let login_payload = LoginPayload {
            org_id,
            password: "wrong_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(&login_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_auth_login_with_nonexistent_org_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let login_payload = LoginPayload {
            org_id: Uuid::new_v4(),
            password: "any_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(&login_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_auth_login_invalid_payload_returns_400() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    // ── Auth: public org list ─────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_auth_orgs_returns_list_without_auth() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        // First register an org so the list is not empty
        let create_payload = CreateOrgPayload {
            name: "List Orgs Org".to_string(),
            address: "3 Auth Road".to_string(),
            password: "list_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        test::call_service(&app, req).await;

        let req = test::TestRequest::get().uri("/api/auth/orgs").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vec<OrgSummary>> = test::read_body_json(resp).await;
        assert!(body.success);
        assert!(body.data.unwrap().len() >= 1);
    }

    // ── Auth: me endpoint ─────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_auth_me_with_valid_token() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Me Endpoint Org".to_string(),
            address: "4 Auth Road".to_string(),
            password: "me_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();

        let req = test::TestRequest::get()
            .uri("/api/auth/me")
            .insert_header(("Authorization", make_auth_header(org.id, &org.name)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.data.unwrap().id, org.id);
    }

    #[actix_web::test]
    async fn test_auth_me_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/auth/me").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    // ── Protected route guard ─────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_list_vehicles_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/vehicles").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_list_dispatches_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/dispatches").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_list_orgs_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/orgs").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_list_customers_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/customers").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    // ── Org scoping ───────────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_get_org_own_org_returns_200() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Own Org Test".to_string(),
            address: "5 Auth Road".to_string(),
            password: "own_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();

        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}", org.id))
            .insert_header(("Authorization", make_auth_header(org.id, &org.name)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
    }

    #[actix_web::test]
    async fn test_get_org_different_org_returns_403() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Org A Forbidden".to_string(),
            address: "6 Auth Road".to_string(),
            password: "pass_a".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org_a = body.data.unwrap();

        // Use a different org_id in the token
        let different_org_id = Uuid::new_v4();
        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}", org_a.id))
            .insert_header(("Authorization", make_auth_header(different_org_id, "Org B")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
    }

    // ── Vehicle scoping ───────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_list_vehicles_scoped_to_authenticated_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Vehicle Scope Org".to_string(),
            address: "7 Fleet Road".to_string(),
            password: "fleet_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let add_vehicle_payload = CreateVehiclePayload {
            registration_number: "SCOPE-VH-001".to_string(),
            capacity: 20,
            unit: "MetricTon".to_string(),
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org.id))
            .insert_header(("Authorization", auth_header.clone()))
            .set_json(&add_vehicle_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);

        let req = test::TestRequest::get()
            .uri("/api/vehicles")
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vec<Vehicle>> = test::read_body_json(resp).await;
        assert!(body.success);
        let vehicles = body.data.unwrap();
        assert!(vehicles.iter().any(|v| v.registration_number == "SCOPE-VH-001"));
    }

    // ── Org creation ──────────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_create_org_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let payload = CreateOrgPayload {
            name: "API Test Express Org".to_string(),
            address: "100 Server Hub, Cyber City".to_string(),
            password: "test_password_123".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        assert!(body.success);
        let org = body.data.unwrap();
        assert_eq!(org.name, "API Test Express Org");
    }

    #[actix_web::test]
    async fn test_create_org_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    // ── Other existing route tests (updated for auth) ─────────────────────────

    #[actix_web::test]
    async fn test_update_org_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Update Org Test".to_string(),
            address: "Initial Address".to_string(),
            password: "update_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let update_payload = UpdateOrgPayload {
            name: "Updated Org Name".to_string(),
            address: "456 Updated Ave, New City".to_string(),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}", org.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&update_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.data.unwrap().name, "Updated Org Name");
    }

    #[actix_web::test]
    async fn test_update_org_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test Org")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_org_location_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Location Org".to_string(),
            address: "Loc Address".to_string(),
            password: "loc_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let payload = LocationPayload {
            latitude: 28.6139,
            longitude: 77.2090,
            address: Some("New Delhi, India".to_string()),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/location", org.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        assert!(body.success);
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 28.6139);
        assert_eq!(loc.longitude, 77.2090);
    }

    #[actix_web::test]
    async fn test_delete_org_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Org To Delete".to_string(),
            address: "Delete Address".to_string(),
            password: "del_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let req = test::TestRequest::delete()
            .uri(&format!("/api/orgs/{}", org.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Organization deleted successfully");
    }

    #[actix_web::test]
    async fn test_add_vehicle_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_add_vehicle_to_nonexistent_org_returns_error() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = CreateVehiclePayload {
            registration_number: "ZZ01 XX 0001".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org_id))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 500);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.message.contains("Failed to register vehicle"));
    }

    #[actix_web::test]
    async fn test_update_vehicle_location_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Mumbai, Maharashtra".to_string()),
        };
        let req = test::TestRequest::put()
            .uri("/api/vehicles/NONEXISTENT-REG-001/location")
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        assert!(body.success);
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 19.0760);
        assert_eq!(loc.longitude, 72.8777);
    }

    #[actix_web::test]
    async fn test_update_vehicle_location_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri("/api/vehicles/MH12EN3502/location")
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_delete_vehicle_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::delete()
            .uri("/api/vehicles/NONEXISTENT-REG-002")
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Vehicle deleted successfully");
    }

    #[actix_web::test]
    async fn test_edit_vehicle_updates_capacity_and_unit() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Edit Vehicle Org").await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateVehiclePayload {
                registration_number: "EDIT-VH-1".to_string(),
                capacity: 10,
                unit: "MetricTon".to_string(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

        let req = test::TestRequest::put()
            .uri("/api/vehicles/EDIT-VH-1")
            .insert_header(("Authorization", auth.clone()))
            .set_json(&UpdateVehiclePayload { capacity: 42, unit: "Box".to_string() })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vehicle> = test::read_body_json(resp).await;
        let v = body.data.unwrap();
        assert_eq!(v.capacity, 42);
        assert_eq!(v.unit, Unit::Box);

        // Persisted.
        let req = test::TestRequest::get()
            .uri("/api/vehicles")
            .insert_header(("Authorization", auth))
            .to_request();
        let body: ApiResponse<Vec<Vehicle>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let stored = body.data.unwrap().into_iter().find(|v| v.registration_number == "EDIT-VH-1").unwrap();
        assert_eq!(stored.capacity, 42);
        assert_eq!(stored.unit, Unit::Box);
    }

    #[actix_web::test]
    async fn test_edit_vehicle_unknown_reg_returns_404() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, auth) = setup_org(&app, "Edit Vehicle 404 Org").await;

        let req = test::TestRequest::put()
            .uri("/api/vehicles/NO-SUCH-VH")
            .insert_header(("Authorization", auth))
            .set_json(&UpdateVehiclePayload { capacity: 5, unit: "MetricTon".to_string() })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_edit_vehicle_from_another_org_returns_403() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Edit Vehicle Owner").await;
        let (_other, other_auth) = setup_org(&app, "Edit Vehicle Attacker").await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org.id))
            .insert_header(("Authorization", auth))
            .set_json(&CreateVehiclePayload {
                registration_number: "OWNED-VH-1".to_string(),
                capacity: 10,
                unit: "MetricTon".to_string(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

        let req = test::TestRequest::put()
            .uri("/api/vehicles/OWNED-VH-1")
            .insert_header(("Authorization", other_auth))
            .set_json(&UpdateVehiclePayload { capacity: 999, unit: "MetricTon".to_string() })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    }

    /// Create an org (with credentials) and one godown under it, returning
    /// `(org, godown, auth_header)`. Shared by the godown/stock route tests below.
    async fn setup_org_with_godown(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        org_name: &str,
        godown_name: &str,
    ) -> (Organization, Godown, String) {
        let create_payload = CreateOrgPayload {
            name: org_name.to_string(),
            address: format!("1 {} Road", org_name),
            password: "godown_test_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let body: ApiResponse<Organization> =
            test::read_body_json(test::call_service(app, req).await).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org.id))
            .insert_header(("Authorization", auth_header.clone()))
            .set_json(&CreateGodownPayload {
                name: godown_name.to_string(),
                address: format!("Plot 1, {}", godown_name),
                max_capacity: None,
            })
            .to_request();
        let body: ApiResponse<Godown> =
            test::read_body_json(test::call_service(app, req).await).await;
        let godown = body.data.unwrap();

        (org, godown, auth_header)
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

    #[actix_web::test]
    async fn test_create_godown_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_create_godown_for_nonexistent_org_returns_error() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = CreateGodownPayload {
            name: "Ghost Godown".to_string(),
            address: "Nowhere".to_string(),
            max_capacity: None,
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org_id))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 500);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.message.contains("Failed to create godown"));
    }

    #[actix_web::test]
    async fn test_list_godowns_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, godown, auth_header) =
            setup_org_with_godown(&app, "List Godowns Org", "List Godown").await;

        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/godowns", org.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vec<Godown>> = test::read_body_json(resp).await;
        assert!(body.success);
        let godowns = body.data.unwrap();
        assert_eq!(godowns.len(), 1);
        assert_eq!(godowns[0].id, godown.id);
    }

    #[actix_web::test]
    async fn test_get_godown_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Get Godown Org", "Get Godown").await;

        let req = test::TestRequest::get()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Godown> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.data.unwrap().id, godown.id);
    }

    #[actix_web::test]
    async fn test_get_godown_not_found_returns_404() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/api/godowns/{}", Uuid::new_v4()))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_get_godown_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, _auth_header) =
            setup_org_with_godown(&app, "Owner Org", "Owner Godown").await;

        let req = test::TestRequest::get()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_update_godown_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Update Godown Org", "Old Godown Name").await;

        let payload = UpdateGodownPayload {
            name: "New Godown Name".to_string(),
            address: "New Godown Address".to_string(),
            max_capacity: None,
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Godown> = test::read_body_json(resp).await;
        assert!(body.success);
        let updated = body.data.unwrap();
        assert_eq!(updated.name, "New Godown Name");
        assert_eq!(updated.address, "New Godown Address");
    }

    #[actix_web::test]
    async fn test_update_godown_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}", Uuid::new_v4()))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_godown_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, _auth_header) =
            setup_org_with_godown(&app, "Update Owner Org", "Update Owner Godown").await;

        let payload = UpdateGodownPayload {
            name: "Hacked Godown".to_string(),
            address: "Hacked Address".to_string(),
            max_capacity: None,
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_update_godown_location_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Godown Location Org", "Godown Location Warehouse").await;

        let payload = LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Mumbai, Maharashtra".to_string()),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}/location", godown.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        assert!(body.success);
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 19.0760);
        assert_eq!(loc.longitude, 72.8777);
    }

    #[actix_web::test]
    async fn test_update_godown_location_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}/location", Uuid::new_v4()))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_delete_godown_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, godown, auth_header) =
            setup_org_with_godown(&app, "Delete Godown Org", "Doomed Godown").await;

        let req = test::TestRequest::delete()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", auth_header.clone()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Godown deleted successfully");

        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/godowns", org.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Vec<Godown>> = test::read_body_json(resp).await;
        assert!(body.data.unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_delete_godown_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, _auth_header) =
            setup_org_with_godown(&app, "Delete Owner Org", "Delete Owner Godown").await;

        let req = test::TestRequest::delete()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_add_godown_stock_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", Uuid::new_v4()))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_add_godown_stock_nonexistent_godown_returns_404() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let payload = CreateStockPayload {
            volume_in_size: 50,
            quantity: 100,
            description: "Ghost Stock".to_string(),
            reorder_threshold: None,
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", Uuid::new_v4()))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_add_godown_stock_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, _auth_header) =
            setup_org_with_godown(&app, "Stock Owner Org", "Stock Owner Godown").await;

        let payload = CreateStockPayload {
            volume_in_size: 50,
            quantity: 100,
            description: "Stolen Goods".to_string(),
            reorder_threshold: None,
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_update_godown_stock_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Update Stock Org", "Update Stock Godown").await;

        let payload = UpdateStockPayload {
            volume_in_size: 200,
            quantity: 75,
            description: "Nonexistent Stock Description".to_string(),
            reorder_threshold: None,
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Stock> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Stock updated successfully");
    }

    #[actix_web::test]
    async fn test_update_godown_stock_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}/stock", Uuid::new_v4()))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_godown_stock_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, _auth_header) =
            setup_org_with_godown(&app, "Update Stock Owner Org", "Update Stock Owner Godown").await;

        let payload = UpdateStockPayload {
            volume_in_size: 999,
            quantity: 999,
            description: "Tampered Stock".to_string(),
            reorder_threshold: None,
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_delete_godown_stock_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Delete Stock Org", "Delete Stock Godown").await;

        let req = test::TestRequest::delete()
            .uri(&format!("/api/godowns/{}/stock/nonexistent-description", godown.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Stock removed successfully");
    }

    #[actix_web::test]
    async fn test_delete_godown_stock_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, _auth_header) =
            setup_org_with_godown(&app, "Delete Stock Owner Org", "Delete Stock Owner Godown").await;

        let req = test::TestRequest::delete()
            .uri(&format!("/api/godowns/{}/stock/some-item", godown.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    /// Add a second godown to an org that already has `auth_header`, returning
    /// the new godown. Used by the stock-transfer route tests.
    async fn add_godown(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        org_id: Uuid,
        auth_header: &str,
        name: &str,
        max_capacity: Option<i64>,
    ) -> Godown {
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org_id))
            .insert_header(("Authorization", auth_header.to_string()))
            .set_json(&CreateGodownPayload {
                name: name.to_string(),
                address: format!("Plot 2, {}", name),
                max_capacity,
            })
            .to_request();
        let body: ApiResponse<Godown> =
            test::read_body_json(test::call_service(app, req).await).await;
        body.data.unwrap()
    }

    async fn seed_stock(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        godown_id: Uuid,
        auth_header: &str,
        description: &str,
        volume_in_size: i64,
        quantity: i64,
    ) {
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown_id))
            .insert_header(("Authorization", auth_header.to_string()))
            .set_json(&CreateStockPayload {
                volume_in_size,
                quantity,
                description: description.to_string(),
                reorder_threshold: None,
            })
            .to_request();
        assert_eq!(test::call_service(app, req).await.status().as_u16(), 201);
    }

    #[actix_web::test]
    async fn test_transfer_godown_stock_moves_units_and_records_the_move() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, from, auth_header) =
            setup_org_with_godown(&app, "Transfer Org", "Source Godown").await;
        let to = add_godown(&app, org.id, &auth_header, "Dest Godown", None).await;
        seed_stock(&app, from.id, &auth_header, "Cement", 5, 100).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/transfer", from.id))
            .insert_header(("Authorization", auth_header.clone()))
            .set_json(&TransferStockPayload {
                to_godown_id: to.id,
                description: "Cement".to_string(),
                quantity: 40,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<StockTransfer> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.data.unwrap().quantity, 40);

        // The audit trail now lists the one transfer.
        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/stock-transfers", org.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let body: ApiResponse<Vec<StockTransfer>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let transfers = body.data.unwrap();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].from_godown_id, from.id);
        assert_eq!(transfers[0].to_godown_id, to.id);
        assert_eq!(transfers[0].description, "Cement");
    }

    #[actix_web::test]
    async fn test_transfer_godown_stock_rejects_insufficient_quantity_with_400() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, from, auth_header) =
            setup_org_with_godown(&app, "Short Transfer Org", "Source").await;
        let to = add_godown(&app, org.id, &auth_header, "Dest", None).await;
        seed_stock(&app, from.id, &auth_header, "Bricks", 2, 10).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/transfer", from.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&TransferStockPayload {
                to_godown_id: to.id,
                description: "Bricks".to_string(),
                quantity: 50,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_transfer_godown_stock_over_destination_capacity_returns_409() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, from, auth_header) =
            setup_org_with_godown(&app, "Cap Transfer Org", "Source").await;
        let to = add_godown(&app, org.id, &auth_header, "Tiny Dest", Some(100)).await;
        seed_stock(&app, from.id, &auth_header, "Tiles", 10, 50).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/transfer", from.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&TransferStockPayload {
                to_godown_id: to.id,
                description: "Tiles".to_string(),
                quantity: 11, // 11 * 10 = 110 > 100
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 409);
    }

    #[actix_web::test]
    async fn test_transfer_godown_stock_returns_403_for_a_foreign_destination() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org_a, from, auth_a) =
            setup_org_with_godown(&app, "Owner Org", "Owner Source").await;
        let (_org_b, foreign, _auth_b) =
            setup_org_with_godown(&app, "Other Org", "Other Godown").await;
        seed_stock(&app, from.id, &auth_a, "Sacks", 1, 20).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/transfer", from.id))
            .insert_header(("Authorization", auth_a))
            .set_json(&TransferStockPayload {
                to_godown_id: foreign.id,
                description: "Sacks".to_string(),
                quantity: 5,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
    }

    #[actix_web::test]
    async fn test_list_stock_transfers_returns_403_for_a_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, _from, _auth) =
            setup_org_with_godown(&app, "Transfers Owner Org", "G").await;

        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/stock-transfers", org.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
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
            })
            .to_request();
        let resp = test::call_service(app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        test::read_body_json::<ApiResponse<Customer>, _>(resp)
            .await
            .data
            .unwrap()
    }

    #[actix_web::test]
    async fn test_create_customer_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Customer API Org").await;

        let customer = create_customer_via_api(&app, org.id, &auth, "API Test Customer").await;
        assert_eq!(customer.name, "API Test Customer");
        assert_eq!(customer.org_id, org.id);
    }

    #[actix_web::test]
    async fn test_create_customer_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, _auth) = setup_org(&app, "Owner Cust Org").await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/customers", org.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .set_json(&CreateCustomerPayload {
                name: "Poached".to_string(),
                address: "x".to_string(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    }

    #[actix_web::test]
    async fn test_create_customer_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/customers", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_list_customers_is_org_scoped() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org_a, auth_a) = setup_org(&app, "Cust Scope A").await;
        let (org_b, auth_b) = setup_org(&app, "Cust Scope B").await;

        create_customer_via_api(&app, org_a.id, &auth_a, "A One").await;
        create_customer_via_api(&app, org_a.id, &auth_a, "A Two").await;
        create_customer_via_api(&app, org_b.id, &auth_b, "B One").await;

        let req = test::TestRequest::get()
            .uri("/api/customers")
            .insert_header(("Authorization", auth_a))
            .to_request();
        let body: ApiResponse<Vec<Customer>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let customers = body.data.unwrap();
        assert_eq!(customers.len(), 2);
        assert!(customers.iter().all(|c| c.org_id == org_a.id));
    }

    #[actix_web::test]
    async fn test_update_customer_location_endpoint() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Cust Loc Org").await;
        let customer = create_customer_via_api(&app, org.id, &auth, "Located Co").await;

        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer.id))
            .insert_header(("Authorization", auth))
            .set_json(&LocationPayload {
                latitude: 19.0760,
                longitude: 72.8777,
                address: Some("Bandra West, Mumbai".to_string()),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 19.0760);
        assert_eq!(loc.longitude, 72.8777);
    }

    #[actix_web::test]
    async fn test_update_customer_location_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Cust Loc Owner").await;
        let customer = create_customer_via_api(&app, org.id, &auth, "Owned Co").await;

        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .set_json(&LocationPayload {
                latitude: 1.0,
                longitude: 1.0,
                address: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    }

    #[actix_web::test]
    async fn test_delete_customer_and_cross_org_guard() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Cust Del Org").await;
        let customer = create_customer_via_api(&app, org.id, &auth, "Deletable Co").await;

        // A different org cannot delete it.
        let req = test::TestRequest::delete()
            .uri(&format!("/api/customers/{}", customer.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

        // The owner can.
        let req = test::TestRequest::delete()
            .uri(&format!("/api/customers/{}", customer.id))
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        let req = test::TestRequest::get()
            .uri("/api/customers")
            .insert_header(("Authorization", auth))
            .to_request();
        let body: ApiResponse<Vec<Customer>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert!(body.data.unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_dispatch_rejects_customer_from_another_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org_a, auth_a) = setup_org(&app, "Dispatch Cust A").await;
        let (org_b, auth_b) = setup_org(&app, "Dispatch Cust B").await;
        let foreign_customer = create_customer_via_api(&app, org_b.id, &auth_b, "B's Customer").await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org_a.id))
            .insert_header(("Authorization", auth_a))
            .set_json(&DispatchRequestPayload {
                customer_id: foreign_customer.id,
                line_items: vec![DispatchLineItemPayload {
                    stock_description: "Anything".to_string(),
                    requested_quantity: 1,
                }],
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.message.contains("different organization"), "{}", body.message);
    }

    #[actix_web::test]
    async fn test_dispatch_stock_no_stock_returns_error() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let payload = DispatchRequestPayload {
            customer_id,
            line_items: vec![DispatchLineItemPayload {
                stock_description: "Nonexistent Stock Description".to_string(),
                requested_quantity: 10,
            }],
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org_id))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_dispatch_stock_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_dispatch_stock_carries_multiple_line_items() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        // setup_dispatch already dispatched "Dispatch Test Goods" x10 on the
        // one godown+vehicle; reuse its org/customer for a second, multi-line
        // dispatch. The vehicle is now on an active trip, so add a second
        // vehicle + driver and more stock first.
        let (org, first, auth) = setup_dispatch(&app, "Multi Line Dispatch Org").await;
        assert_eq!(first.line_items.len(), 1);

        // Second vehicle + active driver.
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateVehiclePayload {
                registration_number: "DISP-VH-002".to_string(),
                capacity: 100_000,
                unit: "MetricTon".to_string(),
            })
            .to_request();
        test::call_service(&app, req).await;
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/drivers", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateDriverPayload {
                name: "Second Driver".to_string(),
                license_number: "DISP-LIC-002".to_string(),
                phone: "+91 90000 00001".to_string(),
            })
            .to_request();
        let driver: ApiResponse<Driver> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let req = test::TestRequest::put()
            .uri("/api/vehicles/DISP-VH-002/driver")
            .insert_header(("Authorization", auth.clone()))
            .set_json(&AssignDriverPayload { driver_id: Some(driver.data.unwrap().id) })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        // A second stock item in the same org (a fresh godown).
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateGodownPayload {
                name: "Second Godown".to_string(),
                address: "9 Second Road".to_string(),
                max_capacity: None,
            })
            .to_request();
        let godown2: ApiResponse<Godown> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let godown2 = godown2.data.unwrap();
        for (desc, qty) in [("Bricks", 500i64), ("Tiles", 300i64)] {
            let req = test::TestRequest::post()
                .uri(&format!("/api/godowns/{}/stock", godown2.id))
                .insert_header(("Authorization", auth.clone()))
                .set_json(&CreateStockPayload {
                    volume_in_size: 1,
                    quantity: qty,
                    description: desc.to_string(),
                    reorder_threshold: None,
                })
                .to_request();
            assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);
        }

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&DispatchRequestPayload {
                customer_id: first.customer_id,
                line_items: vec![
                    DispatchLineItemPayload { stock_description: "Bricks".to_string(), requested_quantity: 120 },
                    DispatchLineItemPayload { stock_description: "Tiles".to_string(), requested_quantity: 40 },
                ],
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
        let order = body.data.unwrap();
        assert_eq!(order.line_items.len(), 2);
        assert_eq!(order.line_items[0].stock_description, "Bricks");
        assert_eq!(order.line_items[1].stock_description, "Tiles");
        assert_eq!(order.line_items.iter().map(|li| li.quantity).sum::<i64>(), 160);

        // A duplicated description is rejected with 400.
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org.id))
            .insert_header(("Authorization", auth))
            .set_json(&DispatchRequestPayload {
                customer_id: first.customer_id,
                line_items: vec![
                    DispatchLineItemPayload { stock_description: "Bricks".to_string(), requested_quantity: 1 },
                    DispatchLineItemPayload { stock_description: "Bricks".to_string(), requested_quantity: 2 },
                ],
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
    }

    // ── GET /api/orgs success path ────────────────────────────────────────────

    #[actix_web::test]
    async fn test_list_orgs_with_valid_token_returns_own_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "List Orgs Success Org".to_string(),
            address: "10 Success Rd".to_string(),
            password: "list_orgs_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let req = test::TestRequest::get()
            .uri("/api/orgs")
            .insert_header(("Authorization", auth_header))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vec<Organization>> = test::read_body_json(resp).await;
        assert!(body.success);
        let orgs = body.data.unwrap();
        assert_eq!(orgs.len(), 1);
        assert_eq!(orgs[0].id, org.id);
    }

    // ── GET /api/customers success path ──────────────────────────────────────

    #[actix_web::test]
    async fn test_list_customers_with_valid_token_returns_200() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::get()
            .uri("/api/customers")
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vec<Customer>> = test::read_body_json(resp).await;
        assert!(body.success);
        assert!(body.data.is_some());
    }

    // ── GET /api/dispatches success path ─────────────────────────────────────

    #[actix_web::test]
    async fn test_list_dispatches_with_valid_token_returns_200() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::get()
            .uri("/api/dispatches")
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Vec<DispatchOrder>> = test::read_body_json(resp).await;
        assert!(body.success);
        // No dispatches for a brand-new UUID org_id — empty list is valid
        assert_eq!(body.data.unwrap().len(), 0);
    }

    // ── POST /api/godowns/{gid}/stock success path ────────────────────────────

    #[actix_web::test]
    async fn test_add_stock_to_own_godown_returns_201() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Stock Test Org", "Stock Test Godown").await;

        let stock_payload = CreateStockPayload {
            volume_in_size: 100,
            quantity: 500,
            description: "Test Widget".to_string(),
            reorder_threshold: None,
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&stock_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<Stock> = test::read_body_json(resp).await;
        assert!(body.success);
        let stock = body.data.unwrap();
        assert_eq!(stock.description, "Test Widget");
        assert_eq!(stock.quantity, 500);
    }

    #[actix_web::test]
    async fn test_create_godown_persists_max_capacity() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Capacity Org".to_string(),
            address: "1 Capacity Road".to_string(),
            password: "pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let org: ApiResponse<Organization> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let org = org.data.unwrap();
        let auth = make_auth_header(org.id, &org.name);

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org.id))
            .insert_header(("Authorization", auth))
            .set_json(&CreateGodownPayload {
                name: "Capped".to_string(),
                address: "Bay 1".to_string(),
                max_capacity: Some(2_000),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<Godown> = test::read_body_json(resp).await;
        assert_eq!(body.data.unwrap().max_capacity, Some(2_000));
    }

    #[actix_web::test]
    async fn test_add_godown_stock_over_capacity_returns_409() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Overfill Org", "Small Godown").await;

        // Give the godown a tight cap.
        let req = test::TestRequest::put()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", auth_header.clone()))
            .set_json(&UpdateGodownPayload {
                name: godown.name.clone(),
                address: godown.address.clone(),
                max_capacity: Some(1_000),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        // 20 * 60 = 1200 > 1000 -> rejected.
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", auth_header.clone()))
            .set_json(&CreateStockPayload {
                volume_in_size: 20,
                quantity: 60,
                description: "Bulky Crates".to_string(),
                reorder_threshold: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 409);

        // 20 * 40 = 800 <= 1000 -> accepted.
        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", auth_header))
            .set_json(&CreateStockPayload {
                volume_in_size: 20,
                quantity: 40,
                description: "Bulky Crates".to_string(),
                reorder_threshold: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);
    }

    #[actix_web::test]
    async fn test_add_godown_stock_reports_below_threshold() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, godown, auth_header) =
            setup_org_with_godown(&app, "Reorder Org", "Reorder Godown").await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", auth_header.clone()))
            .set_json(&CreateStockPayload {
                volume_in_size: 1,
                quantity: 8,
                description: "Label Rolls".to_string(),
                reorder_threshold: Some(25),
            })
            .to_request();
        let body: ApiResponse<Stock> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let stock = body.data.unwrap();
        assert_eq!(stock.reorder_threshold, Some(25));
        assert!(stock.below_threshold);

        // And it survives a reload of the godown.
        let req = test::TestRequest::get()
            .uri(&format!("/api/godowns/{}", godown.id))
            .insert_header(("Authorization", auth_header))
            .to_request();
        let body: ApiResponse<Godown> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let reloaded = body.data.unwrap();
        let item = reloaded
            .stock
            .iter()
            .find(|s| s.description == "Label Rolls")
            .unwrap();
        assert!(item.below_threshold);
    }

    // ── Invalid-payload tests for routes missing them ─────────────────────────

    #[actix_web::test]
    async fn test_update_org_location_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/location", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_customer_location_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let customer_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    // ── Cross-org 403 tests for every mutating org-scoped route ──────────────

    #[actix_web::test]
    async fn test_update_org_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = UpdateOrgPayload {
            name: "Hacked Name".to_string(),
            address: "Hacked Address".to_string(),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_update_org_location_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = LocationPayload {
            latitude: 0.0,
            longitude: 0.0,
            address: None,
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/location", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_delete_org_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let req = test::TestRequest::delete()
            .uri(&format!("/api/orgs/{}", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_add_vehicle_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = CreateVehiclePayload {
            registration_number: "HACK-VH-001".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_create_godown_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = CreateGodownPayload {
            name: "Stolen Godown".to_string(),
            address: "Stolen Address".to_string(),
            max_capacity: None,
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    // ── GET /api/dispatches/{id}/summary ─────────────────────────────────────

    #[actix_web::test]
    async fn test_get_dispatch_summary_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/api/dispatches/{}/summary", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_get_dispatch_summary_nonexistent_dispatch_returns_404() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::get()
            .uri(&format!("/api/dispatches/{}/summary", Uuid::new_v4()))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.message.contains("not found"));
    }

    #[actix_web::test]
    async fn test_get_dispatch_summary_different_org_returns_403() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, dispatch, _auth) = setup_dispatch(&app, "Summary Source Org").await;

        // Request summary with a different org's token — must be 403
        let req = test::TestRequest::get()
            .uri(&format!("/api/dispatches/{}/summary", dispatch.id))
            .insert_header((
                "Authorization",
                make_auth_header(Uuid::new_v4(), "Attacker"),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    // ── PUT /api/dispatches/{id}/status ───────────────────────────────────────

    #[actix_web::test]
    async fn test_update_dispatch_status_without_token_returns_401() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", Uuid::new_v4()))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Confirmed,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_nonexistent_dispatch_returns_404() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", Uuid::new_v4()))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Confirmed,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_invalid_payload() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", Uuid::new_v4()))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, dispatch, _auth) = setup_dispatch(&app, "Status Owner Org").await;

        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch.id))
            .insert_header((
                "Authorization",
                make_auth_header(Uuid::new_v4(), "Attacker"),
            ))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Confirmed,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_valid_transition_succeeds() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, dispatch, auth) = setup_dispatch(&app, "Status Transition Org").await;
        assert_eq!(dispatch.status, DispatchStatus::Pending);

        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch.id))
            .insert_header(("Authorization", auth))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Confirmed,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
        assert!(body.success);
        let updated = body.data.unwrap();
        assert_eq!(updated.status, DispatchStatus::Confirmed);
        assert_eq!(updated.status_history.len(), 2);
        assert_eq!(updated.status_history[0].status, DispatchStatus::Pending);
        assert_eq!(updated.status_history[1].status, DispatchStatus::Confirmed);
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_rejects_illegal_transition() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (_org, dispatch, auth) = setup_dispatch(&app, "Status Illegal Org").await;

        // PENDING -> DELIVERED skips CONFIRMED/LOADED/IN_TRANSIT — must be rejected,
        // even with proof of delivery attached (the state-machine check runs first).
        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch.id))
            .insert_header(("Authorization", auth))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Delivered,
                proof_of_delivery: Some(ProofOfDeliveryPayload {
                    receiver_name: "Priya Sharma".to_string(),
                    signature_or_photo_url: "https://example.com/sig.png".to_string(),
                }),
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_returned_credits_stock_back_into_a_godown() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, dispatch, auth) = setup_dispatch(&app, "Return Flow Org").await;
        // setup_dispatch stocked "Dispatch Test Goods" x100 and dispatched 10 of it.

        async fn goods_qty(
            app: &impl actix_web::dev::Service<
                actix_http::Request,
                Response = actix_web::dev::ServiceResponse,
                Error = actix_web::Error,
            >,
            org_id: Uuid,
            auth: &str,
        ) -> i64 {
            let req = test::TestRequest::get()
                .uri(&format!("/api/orgs/{org_id}/godowns"))
                .insert_header(("Authorization", auth.to_string()))
                .to_request();
            let body: ApiResponse<Vec<Godown>> =
                test::read_body_json(test::call_service(app, req).await).await;
            body.data
                .unwrap()
                .iter()
                .flat_map(|g| &g.stock)
                .filter(|s| s.description == "Dispatch Test Goods")
                .map(|s| s.quantity)
                .sum()
        }

        assert_eq!(goods_qty(&app, org.id, &auth).await, 90, "10 units were dispatched");

        for status in [
            DispatchStatus::Confirmed,
            DispatchStatus::Loaded,
            DispatchStatus::InTransit,
        ] {
            let req = test::TestRequest::put()
                .uri(&format!("/api/dispatches/{}/status", dispatch.id))
                .insert_header(("Authorization", auth.clone()))
                .set_json(&UpdateDispatchStatusPayload {
                    status,
                    proof_of_delivery: None,
                    return_to_godown_id: None,
                })
                .to_request();
            assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);
        }

        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Returned,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
        assert_eq!(body.data.unwrap().status, DispatchStatus::Returned);

        // The 10 returned units are back in the godown.
        assert_eq!(goods_qty(&app, org.id, &auth).await, 100);
    }

    /// Walk a fresh dispatch through PENDING -> CONFIRMED -> LOADED ->
    /// IN_TRANSIT via the API, returning its auth header and id so a test
    /// can attempt the final IN_TRANSIT -> DELIVERED move itself.
    async fn advance_to_in_transit(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        org_name: &str,
    ) -> (Uuid, String) {
        let (_org, dispatch, auth) = setup_dispatch(app, org_name).await;
        for status in [
            DispatchStatus::Confirmed,
            DispatchStatus::Loaded,
            DispatchStatus::InTransit,
        ] {
            let req = test::TestRequest::put()
                .uri(&format!("/api/dispatches/{}/status", dispatch.id))
                .insert_header(("Authorization", auth.clone()))
                .set_json(&UpdateDispatchStatusPayload {
                    status,
                    proof_of_delivery: None,
                    return_to_godown_id: None,
                })
                .to_request();
            let resp = test::call_service(app, req).await;
            assert_eq!(
                resp.status().as_u16(),
                200,
                "advancing to {status} should succeed"
            );
        }
        (dispatch.id, auth)
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_delivered_without_proof_returns_400() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (dispatch_id, auth) = advance_to_in_transit(&app, "POD Missing Org").await;

        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch_id))
            .insert_header(("Authorization", auth))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Delivered,
                proof_of_delivery: None,
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.message.contains("Proof of delivery"));
    }

    #[actix_web::test]
    async fn test_update_dispatch_status_delivered_with_proof_succeeds() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (dispatch_id, auth) = advance_to_in_transit(&app, "POD Present Org").await;

        let req = test::TestRequest::put()
            .uri(&format!("/api/dispatches/{}/status", dispatch_id))
            .insert_header(("Authorization", auth))
            .set_json(&UpdateDispatchStatusPayload {
                status: DispatchStatus::Delivered,
                proof_of_delivery: Some(ProofOfDeliveryPayload {
                    receiver_name: "Priya Sharma".to_string(),
                    signature_or_photo_url: "https://example.com/sig.png".to_string(),
                }),
                return_to_godown_id: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<DispatchOrder> = test::read_body_json(resp).await;
        assert!(body.success);
        let updated = body.data.unwrap();
        assert_eq!(updated.status, DispatchStatus::Delivered);
        let proof = updated.proof_of_delivery.expect("proof should be set");
        assert_eq!(proof.receiver_name, "Priya Sharma");
        assert_eq!(proof.signature_or_photo_url, "https://example.com/sig.png");
        assert!(proof.delivered_at > 0);

        // And it round-trips through a fresh fetch, not just the response.
        let fetched = DispatchOrder::get_by_id(dispatch_id).unwrap().unwrap();
        assert!(fetched.proof_of_delivery.is_some());
    }

    #[actix_web::test]
    async fn test_dispatch_stock_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = DispatchRequestPayload {
            customer_id: Uuid::new_v4(),
            line_items: vec![DispatchLineItemPayload {
                stock_description: "Stolen Stock".to_string(),
                requested_quantity: 10,
            }],
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    // ── Drivers ─────────────────────────────────────────────────────────────

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

    async fn create_driver_via_api(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        org_id: Uuid,
        auth: &str,
        name: &str,
    ) -> Driver {
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{org_id}/drivers"))
            .insert_header(("Authorization", auth.to_string()))
            .set_json(&CreateDriverPayload {
                name: name.to_string(),
                license_number: format!("LIC-{name}"),
                phone: "+91 90000 00000".to_string(),
            })
            .to_request();
        let resp = test::call_service(app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        test::read_body_json::<ApiResponse<Driver>, _>(resp)
            .await
            .data
            .unwrap()
    }

    #[actix_web::test]
    async fn test_driver_crud_and_listing() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Fleet Ops").await;

        let driver = create_driver_via_api(&app, org.id, &auth, "Ravi").await;
        assert!(driver.is_active);

        // Update: rename + deactivate.
        let req = test::TestRequest::put()
            .uri(&format!("/api/drivers/{}", driver.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&UpdateDriverPayload {
                name: "Ravi Kumar".to_string(),
                license_number: "LIC-Ravi".to_string(),
                phone: "+91 90000 00001".to_string(),
                is_active: false,
            })
            .to_request();
        let body: ApiResponse<Driver> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(body.data.as_ref().unwrap().name, "Ravi Kumar");
        assert!(!body.data.unwrap().is_active);

        // List.
        let req = test::TestRequest::get()
            .uri("/api/drivers")
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        let body: ApiResponse<Vec<Driver>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(body.data.unwrap().len(), 1);

        // Delete.
        let req = test::TestRequest::delete()
            .uri(&format!("/api/drivers/{}", driver.id))
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        let req = test::TestRequest::get()
            .uri("/api/drivers")
            .insert_header(("Authorization", auth))
            .to_request();
        let body: ApiResponse<Vec<Driver>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert!(body.data.unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_update_driver_returns_403_for_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Owner Org").await;
        let driver = create_driver_via_api(&app, org.id, &auth, "Owned").await;

        let req = test::TestRequest::put()
            .uri(&format!("/api/drivers/{}", driver.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .set_json(&UpdateDriverPayload {
                name: "Hijacked".to_string(),
                license_number: "X".to_string(),
                phone: "0".to_string(),
                is_active: true,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    }

    #[actix_web::test]
    async fn test_assign_vehicle_driver_rejects_foreign_driver() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org_a, auth_a) = setup_org(&app, "Org A").await;
        let (org_b, auth_b) = setup_org(&app, "Org B").await;

        // Org A gets a vehicle.
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org_a.id))
            .insert_header(("Authorization", auth_a.clone()))
            .set_json(&CreateVehiclePayload {
                registration_number: "A-VH-1".to_string(),
                capacity: 10,
                unit: "MetricTon".to_string(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

        // Org B's driver can't be assigned to org A's vehicle.
        let foreign = create_driver_via_api(&app, org_b.id, &auth_b, "Bee").await;
        let req = test::TestRequest::put()
            .uri("/api/vehicles/A-VH-1/driver")
            .insert_header(("Authorization", auth_a.clone()))
            .set_json(&AssignDriverPayload { driver_id: Some(foreign.id) })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);

        // Org A's own driver assigns fine, and clearing works.
        let own = create_driver_via_api(&app, org_a.id, &auth_a, "Ay").await;
        let req = test::TestRequest::put()
            .uri("/api/vehicles/A-VH-1/driver")
            .insert_header(("Authorization", auth_a.clone()))
            .set_json(&AssignDriverPayload { driver_id: Some(own.id) })
            .to_request();
        let body: ApiResponse<Vehicle> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(body.data.unwrap().assigned_driver_id, Some(own.id));

        let req = test::TestRequest::put()
            .uri("/api/vehicles/A-VH-1/driver")
            .insert_header(("Authorization", auth_a))
            .set_json(&AssignDriverPayload { driver_id: None })
            .to_request();
        let body: ApiResponse<Vehicle> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(body.data.unwrap().assigned_driver_id, None);
    }

    #[actix_web::test]
    async fn test_dispatch_rejected_when_no_vehicle_has_an_active_driver() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "NoDriver Co").await;

        // Vehicle, godown+stock, customer with a location — but no driver.
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateVehiclePayload {
                registration_number: "ND-VH-1".to_string(),
                capacity: 20,
                unit: "MetricTon".to_string(),
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 201);

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/godowns", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateGodownPayload {
                name: "ND Godown".to_string(),
                address: "1 ND Road".to_string(),
                max_capacity: None,
            })
            .to_request();
        let godown: ApiResponse<Godown> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let godown = godown.data.unwrap();

        let req = test::TestRequest::post()
            .uri(&format!("/api/godowns/{}/stock", godown.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateStockPayload {
                volume_in_size: 1,
                quantity: 100,
                description: "Widgets".to_string(),
                reorder_threshold: None,
            })
            .to_request();
        test::call_service(&app, req).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/customers", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateCustomerPayload {
                name: "ND Customer".to_string(),
                address: "2 ND Lane".to_string(),
            })
            .to_request();
        let customer: ApiResponse<Customer> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let customer = customer.data.unwrap();

        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&LocationPayload {
                latitude: 19.07,
                longitude: 72.87,
                address: Some("Mumbai".to_string()),
            })
            .to_request();
        test::call_service(&app, req).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org.id))
            .insert_header(("Authorization", auth))
            .set_json(&DispatchRequestPayload {
                customer_id: customer.id,
                line_items: vec![DispatchLineItemPayload {
                    stock_description: "Widgets".to_string(),
                    requested_quantity: 5,
                }],
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.message.contains("active assigned driver"), "{}", body.message);
    }

    // ── Vehicle compliance documents ────────────────────────────────────────

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

    #[actix_web::test]
    async fn test_vehicle_document_crud_via_api() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Compliance Co").await;
        add_vehicle_via_api(&app, org.id, &auth, "KA05-M9-4321").await;

        // Record an insurance policy expiring in 10 days — "expiring soon".
        let req = test::TestRequest::post()
            .uri("/api/vehicles/KA05-M9-4321/documents")
            .insert_header(("Authorization", auth.clone()))
            .set_json(&VehicleDocumentPayload {
                doc_type: "Insurance".to_string(),
                document_number: "POL-778".to_string(),
                issued_on: Some(iso_date_offset(-355)),
                expires_on: iso_date_offset(10),
                notes: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let created: ApiResponse<VehicleDocument> = test::read_body_json(resp).await;
        let doc = created.data.unwrap();
        assert_eq!(doc.status, ComplianceStatus::ExpiringSoon);
        assert_eq!(doc.doc_type, ComplianceDocType::Insurance);

        // It shows up in the per-vehicle list.
        let req = test::TestRequest::get()
            .uri("/api/vehicles/KA05-M9-4321/documents")
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        let list: ApiResponse<Vec<VehicleDocument>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(list.data.unwrap().len(), 1);

        // Renew it: push the expiry a year out.
        let req = test::TestRequest::put()
            .uri(&format!("/api/vehicle-documents/{}", doc.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&VehicleDocumentPayload {
                doc_type: "Insurance".to_string(),
                document_number: "POL-902".to_string(),
                issued_on: Some(iso_date_offset(0)),
                expires_on: iso_date_offset(365),
                notes: Some("renewed".to_string()),
            })
            .to_request();
        let renewed: ApiResponse<VehicleDocument> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let renewed = renewed.data.unwrap();
        assert_eq!(renewed.status, ComplianceStatus::Valid);
        assert_eq!(renewed.document_number, "POL-902");

        // The org-wide compliance list sees it too.
        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/vehicle-documents", org.id))
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        let org_list: ApiResponse<Vec<VehicleDocument>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert_eq!(org_list.data.unwrap().len(), 1);

        // Delete it.
        let req = test::TestRequest::delete()
            .uri(&format!("/api/vehicle-documents/{}", doc.id))
            .insert_header(("Authorization", auth.clone()))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        let req = test::TestRequest::get()
            .uri("/api/vehicles/KA05-M9-4321/documents")
            .insert_header(("Authorization", auth))
            .to_request();
        let list: ApiResponse<Vec<VehicleDocument>> =
            test::read_body_json(test::call_service(&app, req).await).await;
        assert!(list.data.unwrap().is_empty());
    }

    #[actix_web::test]
    async fn test_add_vehicle_document_rejects_a_bad_date_with_400() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org, auth) = setup_org(&app, "Bad Date Co").await;
        add_vehicle_via_api(&app, org.id, &auth, "BD-VH-1").await;

        let req = test::TestRequest::post()
            .uri("/api/vehicles/BD-VH-1/documents")
            .insert_header(("Authorization", auth))
            .set_json(&VehicleDocumentPayload {
                doc_type: "Permit".to_string(),
                document_number: "PMT-1".to_string(),
                issued_on: None,
                expires_on: "31-12-2026".to_string(),
                notes: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_add_vehicle_document_404_for_a_vehicle_in_another_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org_a, auth_a) = setup_org(&app, "Owner Org").await;
        let (_org_b, auth_b) = setup_org(&app, "Other Org").await;
        add_vehicle_via_api(&app, org_a.id, &auth_a, "OWN-VH-1").await;

        let req = test::TestRequest::post()
            .uri("/api/vehicles/OWN-VH-1/documents")
            .insert_header(("Authorization", auth_b))
            .set_json(&VehicleDocumentPayload {
                doc_type: "Insurance".to_string(),
                document_number: "SNEAKY".to_string(),
                issued_on: None,
                expires_on: iso_date_offset(200),
                notes: None,
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_update_vehicle_document_403_for_a_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org_a, auth_a) = setup_org(&app, "Doc Owner").await;
        let (_org_b, auth_b) = setup_org(&app, "Doc Intruder").await;
        add_vehicle_via_api(&app, org_a.id, &auth_a, "DOC-VH-1").await;

        let req = test::TestRequest::post()
            .uri("/api/vehicles/DOC-VH-1/documents")
            .insert_header(("Authorization", auth_a))
            .set_json(&VehicleDocumentPayload {
                doc_type: "FitnessCertificate".to_string(),
                document_number: "FC-1".to_string(),
                issued_on: None,
                expires_on: iso_date_offset(90),
                notes: None,
            })
            .to_request();
        let created: ApiResponse<VehicleDocument> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let doc_id = created.data.unwrap().id;

        let req = test::TestRequest::put()
            .uri(&format!("/api/vehicle-documents/{doc_id}"))
            .insert_header(("Authorization", auth_b))
            .set_json(&VehicleDocumentPayload {
                doc_type: "FitnessCertificate".to_string(),
                document_number: "HIJACK".to_string(),
                issued_on: None,
                expires_on: iso_date_offset(90),
                notes: None,
            })
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    }

    #[actix_web::test]
    async fn test_list_org_vehicle_documents_403_for_a_different_org() {
        let _db = TestDb::create();
        let app = test::init_service(App::new().configure(config_routes)).await;
        let (org_a, _auth_a) = setup_org(&app, "Fleet A").await;
        let (_org_b, auth_b) = setup_org(&app, "Fleet B").await;

        let req = test::TestRequest::get()
            .uri(&format!("/api/orgs/{}/vehicle-documents", org_a.id))
            .insert_header(("Authorization", auth_b))
            .to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
    }
}
