use crate::logistics::auth::auth::{
    decode_token, generate_token, OrgCredentials, OrgSummary,
};
use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::DispatchOrder;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::stock::stock::Stock;
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
pub struct CreateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateCustomerPayload {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DispatchRequestPayload {
    pub customer_id: Uuid,
    pub stock_description: String,
    pub requested_quantity: i64,
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
pub struct CustomerResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Customer>,
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
        stock: Vec::new(),
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
        stock: Vec::new(),
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
        stock: Vec::new(),
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
        stock: Vec::new(),
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

// ── Stock handlers (protected) ────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/stock",
    tag = "Stock",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateStockPayload,
    responses(
        (status = 201, description = "Stock added successfully", body = StockResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/stock")]
pub async fn add_stock(
    path: web::Path<Uuid>,
    payload: web::Json<CreateStockPayload>,
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
        stock: Vec::new(),
        location: None,
    };

    let stock = Stock::new(
        payload.volume_in_size,
        payload.quantity,
        &payload.description,
    );

    match stock.add_new_stock(&org) {
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
    path = "/api/orgs/{id}/stock",
    tag = "Stock",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = UpdateStockPayload,
    responses(
        (status = 200, description = "Stock updated successfully", body = StockResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}/stock")]
pub async fn update_stock(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateStockPayload>,
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
        stock: Vec::new(),
        location: None,
    };

    let mut stock = Stock::new(0, 0, &payload.description);
    match stock.update_stock(&org, payload.volume_in_size, payload.quantity) {
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
    path = "/api/orgs/{id}/stock/{desc}",
    tag = "Stock",
    security(("bearer_auth" = [])),
    params(
        ("id" = Uuid, Path, description = "Organization UUID"),
        ("desc" = String, Path, description = "Stock item description")
    ),
    responses(
        (status = 200, description = "Stock removed successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/orgs/{id}/stock/{desc}")]
pub async fn delete_stock(
    path: web::Path<(Uuid, String)>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let (org_id, desc) = path.into_inner();
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
        stock: Vec::new(),
        location: None,
    };

    let stock = Stock::new(0, 0, &desc);
    match stock.remove_stock(&org) {
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

// ── Customer handlers (protected) ─────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/customers",
    tag = "Customers",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of all customers", body = CustomerListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/customers")]
pub async fn list_customers(_auth: AuthenticatedOrg) -> impl Responder {
    match Customer::list_all() {
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
    path = "/api/customers",
    tag = "Customers",
    security(("bearer_auth" = [])),
    request_body = CreateCustomerPayload,
    responses(
        (status = 201, description = "Customer created successfully", body = CustomerResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/customers")]
pub async fn create_customer(
    payload: web::Json<CreateCustomerPayload>,
    _auth: AuthenticatedOrg,
) -> impl Responder {
    match Customer::create_customer(&payload.name, &payload.address) {
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
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/customers/{id}/location")]
pub async fn update_customer_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
    _auth: AuthenticatedOrg,
) -> impl Responder {
    let customer_id = path.into_inner();
    let mut customer = Customer {
        id: customer_id,
        name: String::new(),
        address: String::new(),
        location: None,
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

// ── Dispatch handlers (protected) ─────────────────────────────────────────────

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
        stock: Vec::new(),
        location: None,
    };

    let customer = match Customer::get_by_id(payload.customer_id) {
        Ok(Some(c)) => c,
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

    match org.dispatch_stock_to_customer(
        &customer,
        &payload.stock_description,
        payload.requested_quantity,
    ) {
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
    let dispatch_id = path.into_inner();

    let dispatch = match DispatchOrder::get_by_id(dispatch_id) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<String> {
                success: false,
                message: "Dispatch order not found".to_string(),
                data: None,
            })
        }
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch dispatch: {}", err),
                data: None,
            })
        }
    };

    if dispatch.org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: dispatch belongs to a different organization".to_string(),
            data: None,
        });
    }

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
        update_vehicle_location,
        delete_vehicle,
        add_stock,
        update_stock,
        delete_stock,
        list_customers,
        create_customer,
        update_customer_location,
        list_dispatches,
        dispatch_stock,
        get_dispatch_summary,
    ),
    components(
        schemas(
            LoginPayload, LoginData,
            CreateOrgPayload, UpdateOrgPayload, LocationPayload,
            CreateVehiclePayload, CreateStockPayload, UpdateStockPayload,
            CreateCustomerPayload, DispatchRequestPayload,
            Organization, Vehicle, Unit, Location, Stock, Customer, DispatchOrder,
            OrgSummary,
            OrgResponse, OrgListResponse, VehicleResponse, VehicleListResponse,
            StockResponse, CustomerResponse, CustomerListResponse,
            DispatchOrderResponse, DispatchOrderListResponse,
            LocationResponse, OrgSummaryListResponse, EmptyResponse,
        )
    ),
    tags(
        (name = "Health", description = "Health check"),
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Organizations", description = "Organization management"),
        (name = "Vehicles", description = "Vehicle fleet management"),
        (name = "Stock", description = "Stock inventory management"),
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
            .service(update_vehicle_location)
            .service(delete_vehicle)
            .service(add_stock)
            .service(update_stock)
            .service(delete_stock)
            .service(list_customers)
            .service(create_customer)
            .service(update_customer_location)
            .service(list_dispatches)
            .service(dispatch_stock)
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
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/auth/me").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    // ── Protected route guard ─────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_list_vehicles_without_token_returns_401() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/vehicles").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_list_dispatches_without_token_returns_401() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/dispatches").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_list_orgs_without_token_returns_401() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/orgs").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_list_customers_without_token_returns_401() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/api/customers").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    // ── Org scoping ───────────────────────────────────────────────────────────

    #[actix_web::test]
    async fn test_get_org_own_org_returns_200() {
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
    async fn test_add_stock_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/stock", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_stock_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = UpdateStockPayload {
            volume_in_size: 200,
            quantity: 75,
            description: "Nonexistent Stock Description".to_string(),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/stock", org_id))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Stock> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Stock updated successfully");
    }

    #[actix_web::test]
    async fn test_update_stock_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/stock", org_id))
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_delete_stock_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::delete()
            .uri(&format!("/api/orgs/{}/stock/nonexistent-description", org_id))
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Stock removed successfully");
    }

    #[actix_web::test]
    async fn test_create_customer_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = CreateCustomerPayload {
            name: "API Test Customer".to_string(),
            address: "100 Test Lane, Mumbai".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/customers")
            .insert_header(("Authorization", make_auth_header(org_id, "Test")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<Customer> = test::read_body_json(resp).await;
        assert!(body.success);
        let customer = body.data.unwrap();
        assert_eq!(customer.name, "API Test Customer");
    }

    #[actix_web::test]
    async fn test_create_customer_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::post()
            .uri("/api/customers")
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Test")))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_customer_location_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let customer_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let payload = LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Bandra West, Mumbai".to_string()),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer_id))
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
    async fn test_dispatch_stock_no_stock_returns_error() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let payload = DispatchRequestPayload {
            customer_id,
            stock_description: "Nonexistent Stock Description".to_string(),
            requested_quantity: 10,
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

    // ── GET /api/orgs success path ────────────────────────────────────────────

    #[actix_web::test]
    async fn test_list_orgs_with_valid_token_returns_own_org() {
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

    // ── POST /api/orgs/{id}/stock success path ────────────────────────────────

    #[actix_web::test]
    async fn test_add_stock_to_own_org_returns_201() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let create_payload = CreateOrgPayload {
            name: "Stock Test Org".to_string(),
            address: "11 Warehouse Blvd".to_string(),
            password: "stock_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        let org = body.data.unwrap();
        let auth_header = make_auth_header(org.id, &org.name);

        let stock_payload = CreateStockPayload {
            volume_in_size: 100,
            quantity: 500,
            description: "Test Widget".to_string(),
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/stock", org.id))
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

    // ── Invalid-payload tests for routes missing them ─────────────────────────

    #[actix_web::test]
    async fn test_update_org_location_invalid_payload() {
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
    async fn test_add_stock_returns_403_for_different_org() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = CreateStockPayload {
            volume_in_size: 50,
            quantity: 100,
            description: "Stolen Goods".to_string(),
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/stock", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_update_stock_returns_403_for_different_org() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = UpdateStockPayload {
            volume_in_size: 999,
            quantity: 999,
            description: "Tampered Stock".to_string(),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/stock", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_delete_stock_returns_403_for_different_org() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let req = test::TestRequest::delete()
            .uri(&format!("/api/orgs/{}/stock/some-item", target_org_id))
            .insert_header(("Authorization", make_auth_header(attacker_org_id, "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    // ── GET /api/dispatches/{id}/summary ─────────────────────────────────────

    #[actix_web::test]
    async fn test_get_dispatch_summary_without_token_returns_401() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/api/dispatches/{}/summary", Uuid::new_v4()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 401);
    }

    #[actix_web::test]
    async fn test_get_dispatch_summary_nonexistent_dispatch_returns_404() {
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
        let app = test::init_service(App::new().configure(config_routes)).await;

        // Set up an org with a vehicle, stock, and customer, then create a dispatch
        let create_payload = CreateOrgPayload {
            name: "Summary Source Org".to_string(),
            address: "1 Summary Road".to_string(),
            password: "sum_pass".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&create_payload)
            .to_request();
        let body: ApiResponse<Organization> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let org = body.data.unwrap();
        let auth = make_auth_header(org.id, &org.name);

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateVehiclePayload {
                registration_number: "SUM-VH-001".to_string(),
                capacity: 20,
                unit: "MetricTon".to_string(),
            })
            .to_request();
        test::call_service(&app, req).await;

        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/stock", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateStockPayload {
                volume_in_size: 100,
                quantity: 100,
                description: "Summary Goods".to_string(),
            })
            .to_request();
        test::call_service(&app, req).await;

        let req = test::TestRequest::post()
            .uri("/api/customers")
            .insert_header(("Authorization", auth.clone()))
            .set_json(&CreateCustomerPayload {
                name: "Summary Customer".to_string(),
                address: "2 Summary Lane".to_string(),
            })
            .to_request();
        let body: ApiResponse<Customer> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let customer = body.data.unwrap();

        // Set customer location (required for dispatch)
        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&LocationPayload {
                latitude: 19.0760,
                longitude: 72.8777,
                address: Some("Mumbai".to_string()),
            })
            .to_request();
        test::call_service(&app, req).await;

        // Create a dispatch
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org.id))
            .insert_header(("Authorization", auth.clone()))
            .set_json(&DispatchRequestPayload {
                customer_id: customer.id,
                stock_description: "Summary Goods".to_string(),
                requested_quantity: 10,
            })
            .to_request();
        let body: ApiResponse<DispatchOrder> =
            test::read_body_json(test::call_service(&app, req).await).await;
        let dispatch = body.data.unwrap();

        // Request summary with a different org's token — must be 403
        let req = test::TestRequest::get()
            .uri(&format!("/api/dispatches/{}/summary", dispatch.id))
            .insert_header(("Authorization", make_auth_header(Uuid::new_v4(), "Attacker")))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 403);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
    }

    #[actix_web::test]
    async fn test_dispatch_stock_returns_403_for_different_org() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let target_org_id = Uuid::new_v4();
        let attacker_org_id = Uuid::new_v4();
        let payload = DispatchRequestPayload {
            customer_id: Uuid::new_v4(),
            stock_description: "Stolen Stock".to_string(),
            requested_quantity: 10,
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
}
