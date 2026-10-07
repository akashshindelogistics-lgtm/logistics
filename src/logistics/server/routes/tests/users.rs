use super::*;

#[actix_web::test]
async fn test_only_admin_manages_users_and_email_is_unique() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, admin) = setup_org(&app, "User Mgmt Org").await;

    let dispatcher = add_user_and_login(&app, org.id, &admin, "disp@example.com", OrgRole::Dispatcher).await;

    // A dispatcher cannot list or create users.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/users", org.id))
        .insert_header(("Authorization", dispatcher.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/users", org.id))
        .insert_header(("Authorization", dispatcher))
        .set_json(&CreateUserPayload {
            name: "Nope".into(), email: "nope@example.com".into(),
            password: "password1".into(), role: OrgRole::Admin,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);

    // Admin listing sees the dispatcher; a duplicate email is 409.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/users", org.id))
        .insert_header(("Authorization", admin.clone()))
        .to_request();
    let users = test::read_body_json::<ApiResponse<Vec<OrgUser>>, _>(test::call_service(&app, req).await)
        .await.data.unwrap();
    assert_eq!(users.len(), 1);

    let req = test::TestRequest::post()
        .uri(&format!("/api/orgs/{}/users", org.id))
        .insert_header(("Authorization", admin))
        .set_json(&CreateUserPayload {
            name: "Dup".into(), email: "DISP@example.com".into(),
            password: "password1".into(), role: OrgRole::WarehouseStaff,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 409);
}
