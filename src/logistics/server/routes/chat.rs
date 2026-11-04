//! The chat inbox. Conversations are private to the person who started them.
//! See `docs/chat-inbox.md`.

use super::common::*;
use crate::logistics::chat::runtime::{self, Caller, ChatError, ChatThread};
use crate::logistics::chat::store::{self, ChatConversation, ChatProposal, Owner};
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, web};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

const LIST_LIMIT: u32 = 100;
const NOT_SET_UP: &str = "Chat isn't set up on this server yet: it needs ANTHROPIC_API_KEY.";

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ChatMessagePayload {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChatConversationResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<ChatConversation>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChatConversationListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<ChatConversation>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChatThreadResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<ChatThread>,
}

fn error(status: StatusCode, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status).json(ApiResponse::<String> {
        success: false,
        message: message.into(),
        data: None,
    })
}

fn owner(auth: &AuthenticatedOrg) -> Owner {
    Owner {
        org_id: auth.org_id,
        user_id: auth.user_id,
    }
}

fn caller(auth: &AuthenticatedOrg, req: &HttpRequest) -> Caller {
    Caller {
        owner: owner(auth),
        org_name: auth.org_name.clone(),
        role: auth.role.as_str().to_string(),
        // The extractor has already accepted this header.
        authorization: req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
    }
}

fn thread_response(owner: &Owner, id: Uuid, notice: Option<String>) -> HttpResponse {
    let conversation = match store::get_conversation(owner, id) {
        Ok(Some(c)) => c,
        Ok(None) => return error(StatusCode::NOT_FOUND, "Conversation not found"),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    match runtime::build_items(id) {
        Ok(items) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Retrieved conversation".to_string(),
            data: Some(ChatThread {
                conversation,
                items,
                notice,
            }),
        }),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

/// `Ok` carries a problem to show beside the conversation, for failures that
/// leave what's already stored intact.
fn turn_outcome(result: Result<(), ChatError>) -> Result<Option<String>, HttpResponse> {
    match result {
        Ok(()) => Ok(None),
        Err(ChatError::NotFound) => Err(error(StatusCode::NOT_FOUND, "Not found")),
        Err(e @ ChatError::NotPending) => Err(error(StatusCode::CONFLICT, e.to_string())),
        Err(e @ ChatError::Invalid(_)) => Err(error(StatusCode::BAD_REQUEST, e.to_string())),
        Err(e @ (ChatError::OverDailyLimit | ChatError::TooLong | ChatError::Failed(_))) => {
            Ok(Some(e.to_string()))
        }
    }
}

/// The card, if it's in one of this person's own conversations.
fn owned_proposal(owner: &Owner, id: Uuid) -> Result<ChatProposal, HttpResponse> {
    let not_found = || error(StatusCode::NOT_FOUND, "Not found");
    let proposal = store::get_proposal(id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(not_found)?;
    store::get_conversation(owner, proposal.conversation_id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .ok_or_else(not_found)?;
    Ok(proposal)
}

#[utoipa::path(
    get,
    path = "/api/chat/conversations",
    tag = "Chat",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "This person's conversations, most recent first", body = ChatConversationListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/chat/conversations")]
pub async fn list_chat_conversations(auth: AuthenticatedOrg) -> impl Responder {
    match store::list_conversations(&owner(&auth), LIST_LIMIT) {
        Ok(conversations) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Retrieved conversations".to_string(),
            data: Some(conversations),
        }),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[utoipa::path(
    post,
    path = "/api/chat/conversations",
    tag = "Chat",
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "New empty conversation", body = ChatConversationResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/chat/conversations")]
pub async fn create_chat_conversation(auth: AuthenticatedOrg) -> impl Responder {
    match store::create_conversation(&owner(&auth)) {
        Ok(conversation) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Conversation created".to_string(),
            data: Some(conversation),
        }),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[utoipa::path(
    get,
    path = "/api/chat/conversations/{id}",
    tag = "Chat",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Conversation UUID")),
    responses(
        (status = 200, description = "The conversation, ready to draw", body = ChatThreadResponse),
        (status = 404, description = "Not found, or someone else's", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/chat/conversations/{id}")]
pub async fn get_chat_conversation(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    thread_response(&owner(&auth), path.into_inner(), None)
}

#[utoipa::path(
    delete,
    path = "/api/chat/conversations/{id}",
    tag = "Chat",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Conversation UUID")),
    responses(
        (status = 200, description = "Deleted. Anything it already did stays done", body = EmptyResponse),
        (status = 404, description = "Not found, or someone else's", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/chat/conversations/{id}")]
pub async fn delete_chat_conversation(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    match store::delete_conversation(&owner(&auth), path.into_inner()) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Conversation deleted".to_string(),
            data: None,
        }),
        Ok(false) => error(StatusCode::NOT_FOUND, "Conversation not found"),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

#[utoipa::path(
    post,
    path = "/api/chat/conversations/{id}/messages",
    tag = "Chat",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Conversation UUID")),
    request_body = ChatMessagePayload,
    responses(
        (status = 200, description = "The conversation after the model has answered. `notice` says if it couldn't", body = ChatThreadResponse),
        (status = 400, description = "Empty or too long", body = EmptyResponse),
        (status = 404, description = "Not found, or someone else's", body = EmptyResponse),
        (status = 503, description = "No API key on this server", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/chat/conversations/{id}/messages")]
pub async fn send_chat_message(
    req: HttpRequest,
    path: web::Path<Uuid>,
    payload: web::Json<ChatMessagePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let Some(config) = runtime::model_config() else {
        return error(StatusCode::SERVICE_UNAVAILABLE, NOT_SET_UP);
    };
    let id = path.into_inner();
    let caller = caller(&auth, &req);
    let result = runtime::send_message(&caller, id, &payload.text, &config).await;
    match turn_outcome(result) {
        Ok(notice) => thread_response(&caller.owner, id, notice),
        Err(response) => response,
    }
}

#[utoipa::path(
    post,
    path = "/api/chat/proposals/{id}/confirm",
    tag = "Chat",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Proposal UUID")),
    responses(
        (status = 200, description = "It was attempted, as this person. The card holds the result, including a refusal by the API", body = ChatThreadResponse),
        (status = 404, description = "Not found, or someone else's", body = EmptyResponse),
        (status = 409, description = "Already answered", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/chat/proposals/{id}/confirm")]
pub async fn confirm_chat_proposal(
    req: HttpRequest,
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let caller = caller(&auth, &req);
    let proposal = match owned_proposal(&caller.owner, path.into_inner()) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let config = runtime::model_config();
    let result = runtime::confirm(&caller, &proposal, config.as_ref()).await;
    match turn_outcome(result) {
        Ok(notice) => thread_response(&caller.owner, proposal.conversation_id, notice),
        Err(response) => response,
    }
}

#[utoipa::path(
    post,
    path = "/api/chat/proposals/{id}/cancel",
    tag = "Chat",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Proposal UUID")),
    responses(
        (status = 200, description = "Cancelled. Nothing ran", body = ChatThreadResponse),
        (status = 404, description = "Not found, or someone else's", body = EmptyResponse),
        (status = 409, description = "Already answered", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/chat/proposals/{id}/cancel")]
pub async fn cancel_chat_proposal(
    req: HttpRequest,
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let caller = caller(&auth, &req);
    let proposal = match owned_proposal(&caller.owner, path.into_inner()) {
        Ok(p) => p,
        Err(response) => return response,
    };
    match turn_outcome(runtime::cancel(&caller, &proposal)) {
        Ok(notice) => thread_response(&caller.owner, proposal.conversation_id, notice),
        Err(response) => response,
    }
}
