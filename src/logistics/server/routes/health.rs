//! `GET /api/health` — the unauthenticated liveness probe.

use actix_web::{get, HttpResponse, Responder};
use super::common::*;

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
