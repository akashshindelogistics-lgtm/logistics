use super::*;

#[actix_web::test]
async fn test_vehicle_document_crud_via_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Compliance Co").await;
    add_vehicle_via_api(&app, org.id, &auth, "KA05-M9-4321").await;

    // Record an insurance policy expiring in 10 days — "expiring soon".
    let req = test::TestRequest::post()
        .uri("/api/vehicles/KA05-M9-4321/documents")
        .insert_header(("Authorization", auth.clone()))
        .set_json(&VehicleDocumentPayload {
            doc_type: "Insurance".to_string(),
            document_number: "POL-778".to_string(),
            issued_on: Some(iso_date_offset(-355)),
            expires_on: iso_date_offset(10),
            notes: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let created: ApiResponse<VehicleDocument> = test::read_body_json(resp).await;
    let doc = created.data.unwrap();
    assert_eq!(doc.status, ComplianceStatus::ExpiringSoon);
    assert_eq!(doc.doc_type, ComplianceDocType::Insurance);

    // It shows up in the per-vehicle list.
    let req = test::TestRequest::get()
        .uri("/api/vehicles/KA05-M9-4321/documents")
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let list: ApiResponse<Vec<VehicleDocument>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(list.data.unwrap().len(), 1);

    // Renew it: push the expiry a year out.
    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-documents/{}", doc.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&VehicleDocumentPayload {
            doc_type: "Insurance".to_string(),
            document_number: "POL-902".to_string(),
            issued_on: Some(iso_date_offset(0)),
            expires_on: iso_date_offset(365),
            notes: Some("renewed".to_string()),
        })
        .to_request();
    let renewed: ApiResponse<VehicleDocument> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let renewed = renewed.data.unwrap();
    assert_eq!(renewed.status, ComplianceStatus::Valid);
    assert_eq!(renewed.document_number, "POL-902");

    // The org-wide compliance list sees it too.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vehicle-documents", org.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let org_list: ApiResponse<Vec<VehicleDocument>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(org_list.data.unwrap().len(), 1);

    // Delete it.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/vehicle-documents/{}", doc.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

    let req = test::TestRequest::get()
        .uri("/api/vehicles/KA05-M9-4321/documents")
        .insert_header(("Authorization", auth))
        .to_request();
    let list: ApiResponse<Vec<VehicleDocument>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert!(list.data.unwrap().is_empty());
}

#[actix_web::test]
async fn test_add_vehicle_document_rejects_a_bad_date_with_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, auth) = setup_org(&app, "Bad Date Co").await;
    add_vehicle_via_api(&app, org.id, &auth, "BD-VH-1").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/BD-VH-1/documents")
        .insert_header(("Authorization", auth))
        .set_json(&VehicleDocumentPayload {
            doc_type: "Permit".to_string(),
            document_number: "PMT-1".to_string(),
            issued_on: None,
            expires_on: "31-12-2026".to_string(),
            notes: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_add_vehicle_document_404_for_a_vehicle_in_another_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Owner Org").await;
    let (_org_b, auth_b) = setup_org(&app, "Other Org").await;
    add_vehicle_via_api(&app, org_a.id, &auth_a, "OWN-VH-1").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/OWN-VH-1/documents")
        .insert_header(("Authorization", auth_b))
        .set_json(&VehicleDocumentPayload {
            doc_type: "Insurance".to_string(),
            document_number: "SNEAKY".to_string(),
            issued_on: None,
            expires_on: iso_date_offset(200),
            notes: None,
        })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 404);
}

#[actix_web::test]
async fn test_update_vehicle_document_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, auth_a) = setup_org(&app, "Doc Owner").await;
    let (_org_b, auth_b) = setup_org(&app, "Doc Intruder").await;
    add_vehicle_via_api(&app, org_a.id, &auth_a, "DOC-VH-1").await;

    let req = test::TestRequest::post()
        .uri("/api/vehicles/DOC-VH-1/documents")
        .insert_header(("Authorization", auth_a))
        .set_json(&VehicleDocumentPayload {
            doc_type: "FitnessCertificate".to_string(),
            document_number: "FC-1".to_string(),
            issued_on: None,
            expires_on: iso_date_offset(90),
            notes: None,
        })
        .to_request();
    let created: ApiResponse<VehicleDocument> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let doc_id = created.data.unwrap().id;

    let req = test::TestRequest::put()
        .uri(&format!("/api/vehicle-documents/{doc_id}"))
        .insert_header(("Authorization", auth_b))
        .set_json(&VehicleDocumentPayload {
            doc_type: "FitnessCertificate".to_string(),
            document_number: "HIJACK".to_string(),
            issued_on: None,
            expires_on: iso_date_offset(90),
            notes: None,
        })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}

#[actix_web::test]
async fn test_list_org_vehicle_documents_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org_a, _auth_a) = setup_org(&app, "Fleet A").await;
    let (_org_b, auth_b) = setup_org(&app, "Fleet B").await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/vehicle-documents", org_a.id))
        .insert_header(("Authorization", auth_b))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}
