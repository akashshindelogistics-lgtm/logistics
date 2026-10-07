//! Operational reports (`/api/orgs/{id}/reports/*`).

use crate::logistics::reports::OpsReport;
use actix_web::{get, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OpsReportResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<OpsReport>,
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/reports",
    tag = "Reports",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Operational report for the organization", body = OpsReportResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/reports")]
pub async fn get_ops_report(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only view reports for your own organization"
                .to_string(),
            data: None,
        });
    }
    match OpsReport::for_org(org_id) {
        Ok(report) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Report generated".to_string(),
            data: Some(report),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to build report: {}", err),
            data: None,
        }),
    }
}

/// "Explain this report" - an AI-narrated summary of the org's operational
/// report, same shape as `GET /api/dispatches/{id}/summary` but pointed at
/// [`OpsReport`] instead of one dispatch. Not RAG - straight
/// prompt-over-structured-data, exactly like the dispatch summary already is.
#[utoipa::path(
    get,
    path = "/api/orgs/{id}/reports/summary",
    tag = "Reports",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "AI-generated plain-English report briefing", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/reports/summary")]
pub async fn get_ops_report_summary(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only summarize reports for your own organization"
                .to_string(),
            data: None,
        });
    }

    let report = match OpsReport::for_org(org_id) {
        Ok(report) => report,
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to build report: {}", err),
                data: None,
            })
        }
    };

    match crate::logistics::ai::report::generate_report_summary(&report).await {
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
