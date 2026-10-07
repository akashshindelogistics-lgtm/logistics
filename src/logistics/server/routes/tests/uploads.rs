use super::*;

fn upload_request(
    org_id: Uuid,
    auth: Option<&str>,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> actix_http::Request {
    let boundary = "TestBoundary1234";
    let body = multipart_body(boundary, filename, content_type, bytes);
    let mut req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/uploads", org_id))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body);
    if let Some(auth) = auth {
        req = req.insert_header(("Authorization", auth.to_string()));
    }
    req.to_request()
}

#[actix_web::test]
async fn test_upload_file_stores_it_and_serves_it_back() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _, auth) = setup_dispatch(&app, "Upload Org").await;

    let bytes = b"fake png bytes for a test".to_vec();
    let req = upload_request(org.id, Some(&auth), "pod.png", "image/png", &bytes);
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let uploaded = test::read_body_json::<ApiResponse<UploadedFileRef>, _>(resp)
        .await
        .data
        .unwrap();
    assert_eq!(uploaded.url, format!("/api/uploads/{}", uploaded.id));

    // Serve it back — same bytes, same content type, via GET /api/uploads/{id}.
    let req = test::TestRequest::get()
        .uri(&uploaded.url)
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/png"
    );
    let body = test::read_body(resp).await;
    assert_eq!(body.as_ref(), bytes.as_slice());
}

#[actix_web::test]
async fn test_upload_file_requires_auth() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _, _) = setup_dispatch(&app, "Upload Auth Org").await;

    let req = upload_request(org.id, None, "pod.png", "image/png", b"bytes");
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_upload_file_rejects_a_different_orgs_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org1, _, _) = setup_dispatch(&app, "Upload Org One").await;
    let (_, auth2) = setup_org(&app, "Upload Org Two").await;

    let req = upload_request(org1.id, Some(&auth2), "pod.png", "image/png", b"bytes");
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_upload_file_requires_dispatch_role() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _, auth) = setup_dispatch(&app, "Upload Role Org").await;
    let wh = add_user_and_login(&app, org.id, &auth, "upload-wh@example.com", OrgRole::WarehouseStaff).await;

    let req = upload_request(org.id, Some(&wh), "pod.png", "image/png", b"bytes");
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_upload_file_rejects_unsupported_content_type() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _, auth) = setup_dispatch(&app, "Upload CT Org").await;

    let req = upload_request(org.id, Some(&auth), "notes.pdf", "application/pdf", b"%PDF-1.4");
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_upload_file_rejects_a_file_over_the_size_limit() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _, auth) = setup_dispatch(&app, "Upload Size Org").await;

    let too_big = vec![0u8; MAX_UPLOAD_BYTES + 1];
    let req = upload_request(org.id, Some(&auth), "big.png", "image/png", &too_big);
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_get_uploaded_file_404s_for_an_unknown_id() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_, _, auth) = setup_dispatch(&app, "Upload 404 Org").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/uploads/{}", Uuid::new_v4()))
        .insert_header(("Authorization", auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_get_uploaded_file_403s_for_a_different_orgs_token() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org1, _, auth1) = setup_dispatch(&app, "Upload Get Org One").await;
    let (_, auth2) = setup_org(&app, "Upload Get Org Two").await;

    let req = upload_request(org1.id, Some(&auth1), "pod.png", "image/png", b"bytes");
    let uploaded = test::read_body_json::<ApiResponse<UploadedFileRef>, _>(
        test::call_service(&app, req).await,
    )
    .await
    .data
    .unwrap();

    let req = test::TestRequest::get()
        .uri(&uploaded.url)
        .insert_header(("Authorization", auth2))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}
