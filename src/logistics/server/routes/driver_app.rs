//! `GET /api/driver-app/latest` and `GET /api/driver-app/download`: the
//! Android driver app, published on this server for drivers to install and
//! for the app to check for updates. No auth: a driver downloads the app
//! before it's paired, and the APK holds no secrets.

use super::common::*;
use crate::logistics::driver_app::driver_app::{DriverAppDir, DriverAppRelease};
use actix_web::{HttpResponse, Responder, get, web};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DriverAppReleaseResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DriverAppRelease>,
}

fn not_published() -> HttpResponse {
    HttpResponse::NotFound().json(ApiResponse::<String> {
        success: false,
        message: "No driver app release has been published on this server".to_string(),
        data: None,
    })
}

fn release_error(err: impl std::fmt::Display) -> HttpResponse {
    HttpResponse::InternalServerError().json(ApiResponse::<String> {
        success: false,
        message: err.to_string(),
        data: None,
    })
}

#[utoipa::path(
    get,
    path = "/api/driver-app/latest",
    tag = "Driver App",
    responses(
        (status = 200, description = "The current release of the Android driver app", body = DriverAppReleaseResponse),
        (status = 404, description = "No release published on this server", body = EmptyResponse),
        (status = 500, description = "The published release is invalid or unreadable", body = EmptyResponse)
    )
)]
#[get("/driver-app/latest")]
pub async fn latest_driver_app(dir: web::Data<DriverAppDir>) -> impl Responder {
    match dir.latest() {
        Ok(Some(release)) => HttpResponse::Ok()
            // Checked by every phone on the status screen; a stale answer
            // would hide an update.
            .insert_header(("Cache-Control", "no-cache"))
            .json(ApiResponse {
                success: true,
                message: format!("Driver app {}", release.version_name),
                data: Some(release),
            }),
        Ok(None) => not_published(),
        Err(err) => release_error(err),
    }
}

#[utoipa::path(
    get,
    path = "/api/driver-app/download",
    tag = "Driver App",
    responses(
        (status = 200, description = "The current release's APK", content_type = "application/vnd.android.package-archive"),
        (status = 404, description = "No release published on this server", body = EmptyResponse),
        (status = 500, description = "The published release is invalid or unreadable", body = EmptyResponse)
    )
)]
#[get("/driver-app/download")]
pub async fn download_driver_app(dir: web::Data<DriverAppDir>) -> impl Responder {
    let (release, path) = match dir.latest_apk() {
        Ok(Some(found)) => found,
        Ok(None) => return not_published(),
        Err(err) => return release_error(err),
    };
    match std::fs::read(&path) {
        Ok(bytes) => HttpResponse::Ok()
            .content_type("application/vnd.android.package-archive")
            .insert_header((
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", release.file_name),
            ))
            .insert_header(("Cache-Control", "no-cache"))
            .body(bytes),
        Err(err) => release_error(format!("couldn't read {}: {err}", release.file_name)),
    }
}
