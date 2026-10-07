//! Team members (role-scoped users) of an organization.

use crate::logistics::user::user::{OrgRole, OrgUser, UserError};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateUserPayload {
    pub name: String,
    pub email: String,
    pub password: String,
    pub role: OrgRole,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateUserPayload {
    pub name: String,
    pub role: OrgRole,
    pub is_active: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<OrgUser>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<OrgUser>>,
}

/// Load a user and confirm it belongs to `auth_org_id`, or return the
/// 403/404/500 response to bail out with.
fn load_owned_user(user_id: Uuid, auth_org_id: Uuid) -> Result<OrgUser, HttpResponse> {
    match OrgUser::get_by_id(user_id) {
        Ok(Some(u)) if u.org_id == auth_org_id => Ok(u),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "That user belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "User not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch user: {}", err),
            data: None,
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/users",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The org's team members", body = UserListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/users")]
pub async fn list_org_users(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    if let Err(resp) = auth.require_role(&[OrgRole::Admin]) {
        return resp;
    }
    match OrgUser::list_by_org(org_id) {
        Ok(users) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} team members", users.len()),
            data: Some(users),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list team members: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/users",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateUserPayload,
    responses(
        (status = 201, description = "Team member created", body = UserResponse),
        (status = 409, description = "Email already registered", body = EmptyResponse),
        (status = 400, description = "Invalid email or password", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/users")]
pub async fn add_org_user(
    path: web::Path<Uuid>,
    payload: web::Json<CreateUserPayload>,
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
    if let Err(resp) = auth.require_role(&[OrgRole::Admin]) {
        return resp;
    }

    match OrgUser::create(
        org_id,
        &payload.name,
        &payload.email,
        &payload.password,
        payload.role,
    ) {
        Ok(user) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Team member created".to_string(),
            data: Some(user),
        }),
        Err(UserError::EmailTaken) => HttpResponse::Conflict().json(ApiResponse::<String> {
            success: false,
            message: "That email address is already registered".to_string(),
            data: None,
        }),
        Err(UserError::InvalidInput(why)) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: why.to_string(),
            data: None,
        }),
        Err(UserError::Db(err)) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create team member: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "User UUID")),
    request_body = UpdateUserPayload,
    responses(
        (status = 200, description = "Team member updated", body = UserResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "User not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/users/{id}")]
pub async fn update_org_user(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateUserPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&[OrgRole::Admin]) {
        return resp;
    }
    let mut user = match load_owned_user(path.into_inner(), auth.org_id) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    match user.update(&payload.name, payload.role, payload.is_active) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Team member updated".to_string(),
            data: Some(user),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update team member: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}",
    tag = "Users",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "User UUID")),
    responses(
        (status = 200, description = "Team member removed", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "User not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/users/{id}")]
pub async fn delete_org_user(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    if let Err(resp) = auth.require_role(&[OrgRole::Admin]) {
        return resp;
    }
    let user_id = path.into_inner();
    if auth.user_id == Some(user_id) {
        return HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: "You can't delete your own account".to_string(),
            data: None,
        });
    }
    let user = match load_owned_user(user_id, auth.org_id) {
        Ok(u) => u,
        Err(resp) => return resp,
    };
    match user.delete() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Team member removed".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to remove team member: {}", err),
            data: None,
        }),
    }
}
