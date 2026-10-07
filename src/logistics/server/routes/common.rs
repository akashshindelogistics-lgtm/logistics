//! Shared plumbing for every route module: the `AuthenticatedOrg` bearer-token
//! extractor and role gates, the `ApiResponse` envelope, and the payload /
//! response types used by more than one domain.

use crate::logistics::auth::auth::decode_token;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::user::user::OrgRole;
use crate::logistics::vehicle::vehicle::{Location, Vehicle};
use actix_web::{dev::Payload, FromRequest, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};
use utoipa::ToSchema;
use uuid::Uuid;

pub struct AuthenticatedOrg {
    pub org_id: Uuid,
    pub org_name: String,
    /// The caller's role: `Admin` for the org-owner login, or the team
    /// member's role for a `POST /api/auth/user-login` token.
    pub role: OrgRole,
    /// The team member's id, when the token came from user-login.
    pub user_id: Option<Uuid>,
}

/// Roles allowed to run and bill dispatches, and to manage customers.
pub(super) const DISPATCH_ROLES: [OrgRole; 2] = [OrgRole::Admin, OrgRole::Dispatcher];

/// Roles allowed to manage godowns and their stock.
pub(super) const WAREHOUSE_ROLES: [OrgRole; 2] = [OrgRole::Admin, OrgRole::WarehouseStaff];

/// Admin-only.
pub(super) const ADMIN_ONLY: [OrgRole; 1] = [OrgRole::Admin];

impl AuthenticatedOrg {
    /// `Ok(())` when the caller's role is in `allowed`, otherwise the `403`
    /// response to return. Every role may read; this gates writes.
    pub(super) fn require_role(&self, allowed: &[OrgRole]) -> Result<(), HttpResponse> {
        if allowed.contains(&self.role) {
            Ok(())
        } else {
            let names: Vec<&str> = allowed.iter().map(|r| r.as_str()).collect();
            Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
                success: false,
                message: format!(
                    "Your role ({}) is not allowed to do this — needs one of: {}",
                    self.role.as_str(),
                    names.join(", ")
                ),
                data: None,
            }))
        }
    }
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
                    role: OrgRole::from_str(&claims.role),
                    user_id: claims.user_id.as_deref().and_then(|u| Uuid::parse_str(u).ok()),
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

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LocationPayload {
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
}

/// One line on a dispatch request: a stock description and how many units of
/// it to send.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DispatchLineItemPayload {
    pub stock_description: String,
    pub requested_quantity: i64,
}

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
