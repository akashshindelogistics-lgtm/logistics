//! File uploads (`/api/orgs/{id}/uploads`) and serving them back.

use crate::logistics::upload::upload::{UploadedFile, UploadError, MAX_UPLOAD_BYTES};
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse, Responder};
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

/// The minimal info handed back after a successful upload: the new file's id
/// and the URL it can be fetched back from (`GET /api/uploads/{id}`).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadedFileRef {
    pub id: Uuid,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadedFileResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<UploadedFileRef>,
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/uploads",
    tag = "Uploads",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body(
        content = String,
        description = "multipart/form-data with one `file` field — image/jpeg, image/png or image/webp, up to 5 MiB",
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "File stored", body = UploadedFileResponse),
        (status = 400, description = "No file, an unsupported content type, or too large", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/uploads")]
pub async fn upload_file(
    path: web::Path<Uuid>,
    mut payload: Multipart,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only upload files for your own organization"
                .to_string(),
            data: None,
        });
    }

    let mut content_type: Option<String> = None;
    let mut bytes: Vec<u8> = Vec::new();
    let mut found_field = false;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(err) => {
                return HttpResponse::BadRequest().json(ApiResponse::<String> {
                    success: false,
                    message: format!("Malformed multipart upload: {err}"),
                    data: None,
                })
            }
        };
        found_field = true;
        content_type = field.content_type().map(|m| m.to_string());

        while let Some(chunk) = field.next().await {
            let data = match chunk {
                Ok(d) => d,
                Err(err) => {
                    return HttpResponse::BadRequest().json(ApiResponse::<String> {
                        success: false,
                        message: format!("Malformed multipart upload: {err}"),
                        data: None,
                    })
                }
            };
            if bytes.len() + data.len() > MAX_UPLOAD_BYTES {
                return HttpResponse::BadRequest().json(ApiResponse::<String> {
                    success: false,
                    message: format!(
                        "File exceeds the {MAX_UPLOAD_BYTES}-byte limit"
                    ),
                    data: None,
                });
            }
            bytes.extend_from_slice(&data);
        }
        break; // exactly one file field is expected
    }

    if !found_field {
        return HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: "No file was uploaded".to_string(),
            data: None,
        });
    }
    let Some(content_type) = content_type else {
        return HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: "Uploaded file is missing a content type".to_string(),
            data: None,
        });
    };

    match UploadedFile::store(org_id, &content_type, &bytes) {
        Ok(file) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "File uploaded".to_string(),
            data: Some(UploadedFileRef {
                id: file.id,
                url: format!("/api/uploads/{}", file.id),
            }),
        }),
        Err(err @ (UploadError::UnsupportedContentType(_) | UploadError::Empty | UploadError::TooLarge(_))) => {
            HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: err.to_string(),
                data: None,
            })
        }
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: err.to_string(),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/uploads/{id}",
    tag = "Uploads",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Uploaded file UUID")),
    responses(
        (status = 200, description = "The file's raw bytes, served back with its stored content type"),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/uploads/{id}")]
pub async fn get_uploaded_file(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let id = path.into_inner();
    match UploadedFile::get_by_id(id) {
        Ok(Some(file)) => {
            if file.org_id != auth.org_id {
                return HttpResponse::Forbidden().json(ApiResponse::<String> {
                    success: false,
                    message: "Access denied: this file belongs to a different organization"
                        .to_string(),
                    data: None,
                });
            }
            match file.read_bytes() {
                Ok(bytes) => HttpResponse::Ok().content_type(file.content_type.clone()).body(bytes),
                Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
                    success: false,
                    message: format!("Failed to read stored file: {err}"),
                    data: None,
                }),
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "File not found".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch file: {}", err),
            data: None,
        }),
    }
}
