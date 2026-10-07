use super::*;

#[actix_web::test]
async fn test_org_notifications_feed_is_scoped_to_the_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, _d, auth) = setup_dispatch(&app, "Notify Feed Org").await;
    let (_other, other_auth) = setup_org(&app, "Notify Feed Intruder").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/notifications", org.id))
        .insert_header(("Authorization", auth))
        .to_request();
    let feed = test::read_body_json::<ApiResponse<Vec<Notification>>, _>(
        test::call_service(&app, req).await,
    )
    .await
    .data
    .unwrap();
    assert_eq!(feed.len(), 2);
    assert!(feed.iter().all(|n| n.org_id == org.id));

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/notifications", org.id))
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}
