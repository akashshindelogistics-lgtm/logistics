//! Login endpoints (`/api/auth/*`): the public org list, org-owner and
//! team-member login, and `GET /api/auth/me`.

use crate::logistics::auth::auth::{generate_token, generate_user_token, OrgCredentials, OrgSummary};
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::user::user::OrgUser;
use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LoginPayload {
    pub org_id: Uuid,
    pub password: String,
}

/// Body for `POST /api/auth/user-login` — a team member signs in with the
/// email + password an Admin set for them.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UserLoginPayload {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginData {
    pub token: String,
    pub org_id: String,
    pub org_name: String,
    /// `ADMIN` for the org-owner login, else the team member's role.
    #[serde(default = "default_admin_role_string")]
    pub role: String,
    /// The team member's display name, when this was a user-login.
    #[serde(default)]
    pub user_name: Option<String>,
}

fn default_admin_role_string() -> String {
    "ADMIN".to_string()
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrgSummaryListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<OrgSummary>>,
}

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
                        role: "ADMIN".to_string(),
                        user_name: None,
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
    post,
    path = "/api/auth/user-login",
    tag = "Auth",
    request_body = UserLoginPayload,
    responses(
        (status = 200, description = "Login successful, returns a role-scoped JWT", body = OrgResponse),
        (status = 401, description = "Invalid credentials or inactive account", body = EmptyResponse)
    )
)]
#[post("/auth/user-login")]
pub async fn user_login(payload: web::Json<UserLoginPayload>) -> impl Responder {
    let user = match OrgUser::verify_login(&payload.email, &payload.password) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::Unauthorized().json(ApiResponse::<String> {
                success: false,
                message: "Invalid email or password, or the account is inactive".to_string(),
                data: None,
            })
        }
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Authentication error: {}", err),
                data: None,
            })
        }
    };

    let org_name = Organization::get_by_id(user.org_id)
        .ok()
        .flatten()
        .map(|o| o.name)
        .unwrap_or_default();

    match generate_user_token(user.org_id, &org_name, Some(user.id), user.role.as_str()) {
        Ok(token) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Login successful".to_string(),
            data: Some(LoginData {
                token,
                org_id: user.org_id.to_string(),
                org_name,
                role: user.role.as_str().to_string(),
                user_name: Some(user.name),
            }),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to generate token: {}", err),
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
