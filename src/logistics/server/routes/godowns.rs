//! Godowns (warehouses), the stock they hold, and stock transfers between them.

use crate::logistics::godown::godown::Godown;
use crate::logistics::godown::transfer::{StockTransfer, TransferError};
use crate::logistics::stock::stock::Stock;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
    /// Free-text, org-defined category (e.g. "Cement", "Electronics").
    /// Defaults to `"General"` when omitted.
    #[serde(default = "crate::logistics::stock::stock::default_category")]
    pub category: String,
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
    /// Free-text, org-defined category (e.g. "Cement", "Electronics").
    /// Defaults to `"General"` when omitted.
    #[serde(default = "crate::logistics::stock::stock::default_category")]
    pub category: String,
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
    .with_category(&payload.category)
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
        &payload.category,
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
    if let Err(resp) = auth.require_role(&WAREHOUSE_ROLES) {
        return resp;
    }
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
