use super::*;

#[actix_web::test]
async fn test_health_check_endpoint() {
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: ApiResponse<String> = test::read_body_json(resp).await;
    assert!(body.success);
    assert_eq!(body.message, "Logistics system REST API operational");
}

#[actix_web::test]
async fn test_swagger_ui_endpoint() {
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/swagger-ui/").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success() || resp.status().is_redirection());
}

#[actix_web::test]
async fn test_openapi_json_spec_endpoint() {
    let app = test::init_service(App::new().configure(config_routes)).await;
    let req = test::TestRequest::get().uri("/api-docs/openapi.json").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
