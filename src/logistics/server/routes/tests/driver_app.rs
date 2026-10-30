use super::*;
use crate::logistics::driver_app::driver_app::{DriverAppDir, MANIFEST_FILE, Manifest};
use crate::logistics::test_support::TempDir;
use actix_web::web;

/// An app whose driver app releases live in `dir` (no database needed).
macro_rules! app_with_releases {
    ($dir:expr) => {
        test::init_service(
            App::new()
                .configure(config_routes)
                .app_data(web::Data::new(DriverAppDir($dir.clone()))),
        )
        .await
    };
}

fn publish(dir: &std::path::Path, version_code: i64, apk: &[u8]) -> Manifest {
    let manifest = Manifest {
        version_code,
        version_name: format!("0.{version_code}.0"),
        file: format!("logistics-driver-0.{version_code}.0.apk"),
        sha256: "0f".repeat(32),
        released_at: 1_793_000_000,
    };
    std::fs::write(dir.join(&manifest.file), apk).unwrap();
    std::fs::write(
        dir.join(MANIFEST_FILE),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    manifest
}

#[actix_web::test]
async fn test_latest_driver_app_with_nothing_published_is_404() {
    let tmp = TempDir::create();
    let app = app_with_releases!(tmp.0);

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/driver-app/latest")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 404);
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/driver-app/download")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_latest_driver_app_describes_the_published_release_without_auth() {
    let tmp = TempDir::create();
    publish(&tmp.0, 2, b"apk bytes");
    let app = app_with_releases!(tmp.0);

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/driver-app/latest")
            .to_request(),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.headers().get("Cache-Control").unwrap(), "no-cache");
    let body: DriverAppReleaseResponse = test::read_body_json(resp).await;
    let release = body.data.unwrap();
    assert_eq!(release.version_code, 2);
    assert_eq!(release.version_name, "0.2.0");
    assert_eq!(release.size_bytes, 9);
    assert_eq!(release.sha256, "0f".repeat(32));
    assert_eq!(release.download_path, "/api/driver-app/download");
}

#[actix_web::test]
async fn test_download_driver_app_serves_the_latest_apk() {
    let tmp = TempDir::create();
    publish(&tmp.0, 1, b"old release");
    publish(&tmp.0, 3, b"new release");
    let app = app_with_releases!(tmp.0);

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/driver-app/download")
            .to_request(),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(
        resp.headers().get("Content-Type").unwrap(),
        "application/vnd.android.package-archive"
    );
    assert_eq!(
        resp.headers().get("Content-Disposition").unwrap(),
        "attachment; filename=\"logistics-driver-0.3.0.apk\""
    );
    assert_eq!(test::read_body(resp).await.as_ref(), b"new release");
}

#[actix_web::test]
async fn test_driver_app_with_a_broken_manifest_is_500_and_serves_nothing() {
    let tmp = TempDir::create();
    let manifest = publish(&tmp.0, 1, b"apk");
    std::fs::remove_file(tmp.0.join(&manifest.file)).unwrap();
    // A file outside the directory must never be served.
    let escaping = Manifest {
        file: "../secret.apk".to_string(),
        ..manifest
    };
    let app = app_with_releases!(tmp.0);

    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/driver-app/latest")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 500);

    std::fs::write(
        tmp.0.join(MANIFEST_FILE),
        serde_json::to_string(&escaping).unwrap(),
    )
    .unwrap();
    let resp = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/driver-app/download")
            .to_request(),
    )
    .await;
    assert_eq!(resp.status().as_u16(), 500);
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(
        body.message.contains("file must be the name of an .apk"),
        "{}",
        body.message
    );
}
