//! Notification history for a dispatch or a whole org.

use crate::logistics::notification::notification::Notification;
use actix_web::{get, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;
use super::dispatches::load_owned_dispatch;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NotificationListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Notification>>,
}

#[utoipa::path(
    get,
    path = "/api/dispatches/{id}/notifications",
    tag = "Notifications",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Dispatch order UUID")),
    responses(
        (status = 200, description = "Notifications recorded for this dispatch", body = NotificationListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Dispatch not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/dispatches/{id}/notifications")]
pub async fn list_dispatch_notifications(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let dispatch = match load_owned_dispatch(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match Notification::list_by_dispatch(dispatch.id) {
        Ok(notifications) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} notifications", notifications.len()),
            data: Some(notifications),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list notifications: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/notifications",
    tag = "Notifications",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The org's 100 most recent notifications", body = NotificationListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/notifications")]
pub async fn list_org_notifications(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match Notification::list_by_org(org_id, 100) {
        Ok(notifications) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} notifications", notifications.len()),
            data: Some(notifications),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list notifications: {}", err),
            data: None,
        }),
    }
}
