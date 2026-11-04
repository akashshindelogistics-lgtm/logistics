//! Agent settings, the audit trail and the approval inbox. See
//! `docs/agentic-workflow.md`.

use super::common::*;
use crate::logistics::agent::model::ModelConfig;
use crate::logistics::agent::runtime::{self, DecisionError, RunError};
use crate::logistics::agent::store::{self, ActionStatus, AgentAction, AgentRun, AutonomyMode};
use actix_web::{HttpResponse, Responder, get, post, put, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

const LIST_LIMIT: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentSettings {
    pub mode: AutonomyMode,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentSettingsResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<AgentSettings>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentRunResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<AgentRun>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentRunListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<AgentRun>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentActionResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<AgentAction>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AgentActionListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<AgentAction>>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct AgentActionQuery {
    /// Only actions with this status, such as `PENDING_APPROVAL`.
    pub status: Option<ActionStatus>,
}

fn error(status: actix_web::http::StatusCode, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status).json(ApiResponse::<String> {
        success: false,
        message: message.into(),
        data: None,
    })
}

fn require_own_org(auth: &AuthenticatedOrg, org_id: Uuid) -> Result<(), HttpResponse> {
    if org_id == auth.org_id {
        Ok(())
    } else {
        Err(error(
            actix_web::http::StatusCode::FORBIDDEN,
            "Access denied",
        ))
    }
}

fn internal(message: String) -> HttpResponse {
    error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, message)
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/agents/settings",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The org's autonomy mode", body = AgentSettingsResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/agents/settings")]
pub async fn get_agent_settings(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if let Err(resp) = require_own_org(&auth, org_id) {
        return resp;
    }
    match store::get_mode(org_id) {
        Ok(mode) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Agents are {}", mode.as_str()),
            data: Some(AgentSettings { mode }),
        }),
        Err(e) => internal(e),
    }
}

#[utoipa::path(
    put,
    path = "/api/orgs/{id}/agents/settings",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = AgentSettings,
    responses(
        (status = 200, description = "Autonomy mode changed", body = AgentSettingsResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}/agents/settings")]
pub async fn update_agent_settings(
    path: web::Path<Uuid>,
    payload: web::Json<AgentSettings>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&ADMIN_ONLY) {
        return resp;
    }
    let org_id = path.into_inner();
    if let Err(resp) = require_own_org(&auth, org_id) {
        return resp;
    }
    match store::set_mode(org_id, payload.mode) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Agents are now {}", payload.mode.as_str()),
            data: Some(payload.into_inner()),
        }),
        Err(e) => internal(e),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/agents/run",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The finished run. A run the model couldn't finish still returns 200, with status FAILED", body = AgentRunResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 409, description = "Agents are off, a run is in progress, or the daily limit is used up", body = EmptyResponse),
        (status = 503, description = "No ANTHROPIC_API_KEY configured", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/agents/run")]
pub async fn run_agents_now(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let org_id = path.into_inner();
    if let Err(resp) = require_own_org(&auth, org_id) {
        return resp;
    }
    let Some(config) = ModelConfig::from_env() else {
        return error(
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
            "Agents need ANTHROPIC_API_KEY to be configured",
        );
    };
    match runtime::run_dispatch_desk(org_id, "manual", &config).await {
        Ok(run) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Agent run {}", run.status),
            data: Some(run),
        }),
        Err(RunError::Failed(e)) => internal(e),
        Err(e) => error(actix_web::http::StatusCode::CONFLICT, e.to_string()),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/agents/runs",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The org's 100 most recent agent runs, newest first", body = AgentRunListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/agents/runs")]
pub async fn list_agent_runs(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if let Err(resp) = require_own_org(&auth, org_id) {
        return resp;
    }
    match store::list_runs(org_id, LIST_LIMIT) {
        Ok(runs) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} agent runs", runs.len()),
            data: Some(runs),
        }),
        Err(e) => internal(e),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/agents/actions",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID"), AgentActionQuery),
    responses(
        (status = 200, description = "The org's 100 most recent agent actions, newest first", body = AgentActionListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/agents/actions")]
pub async fn list_agent_actions(
    path: web::Path<Uuid>,
    query: web::Query<AgentActionQuery>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let org_id = path.into_inner();
    if let Err(resp) = require_own_org(&auth, org_id) {
        return resp;
    }
    match store::list_actions(org_id, query.status, LIST_LIMIT) {
        Ok(actions) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} agent actions", actions.len()),
            data: Some(actions),
        }),
        Err(e) => internal(e),
    }
}

fn decision_response(result: Result<AgentAction, DecisionError>, message: &str) -> HttpResponse {
    match result {
        Ok(action) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: message.to_string(),
            data: Some(action),
        }),
        Err(DecisionError::NotFound) => error(
            actix_web::http::StatusCode::NOT_FOUND,
            "Agent action not found",
        ),
        Err(DecisionError::NotPending) => error(
            actix_web::http::StatusCode::CONFLICT,
            "This action isn't waiting for approval",
        ),
        Err(DecisionError::Failed(e)) => internal(e),
    }
}

#[utoipa::path(
    post,
    path = "/api/agent-actions/{id}/approve",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Agent action UUID")),
    responses(
        (status = 200, description = "Carried out, or status FAILED if it's no longer valid", body = AgentActionResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Not found in this organization", body = EmptyResponse),
        (status = 409, description = "Not waiting for approval", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/agent-actions/{id}/approve")]
pub async fn approve_agent_action(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    decision_response(
        runtime::approve_action(auth.org_id, path.into_inner(), auth.user_id),
        "Decision recorded",
    )
}

#[utoipa::path(
    post,
    path = "/api/agent-actions/{id}/reject",
    tag = "Agents",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Agent action UUID")),
    responses(
        (status = 200, description = "Rejected. Nothing happened, and the agent won't ask again", body = AgentActionResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Not found in this organization", body = EmptyResponse),
        (status = 409, description = "Not waiting for approval", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/agent-actions/{id}/reject")]
pub async fn reject_agent_action(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    decision_response(
        runtime::reject_action(auth.org_id, path.into_inner(), auth.user_id),
        "Rejected",
    )
}
