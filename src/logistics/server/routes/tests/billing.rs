use super::*;

/// An ISO `YYYY-MM-DD` string `offset` days from today (UTC).
fn billing_date_offset(offset: i64) -> String {
    let days = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64)
        .div_euclid(86_400)
        + offset;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[actix_web::test]
async fn test_invoice_lifecycle_via_api() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (org, dispatch, auth) = setup_dispatch(&app, "Billing Flow Org").await;

    // Raise an invoice due in 15 days -> PENDING.
    let req = test::TestRequest::post()
        .uri(&format!("/api/dispatches/{}/invoice", dispatch.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&InvoicePayload { amount: 4500, due_on: billing_date_offset(15) })
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 201);
    let inv: ApiResponse<Invoice> = test::read_body_json(resp).await;
    let inv = inv.data.unwrap();
    assert_eq!(inv.amount, 4500);
    assert_eq!(inv.status, PaymentStatus::Pending);

    // A second invoice for the same dispatch -> 409.
    let req = test::TestRequest::post()
        .uri(&format!("/api/dispatches/{}/invoice", dispatch.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&InvoicePayload { amount: 1, due_on: billing_date_offset(1) })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 409);

    // Amend the amount.
    let req = test::TestRequest::put()
        .uri(&format!("/api/invoices/{}", inv.id))
        .insert_header(("Authorization", auth.clone()))
        .set_json(&InvoicePayload { amount: 5200, due_on: billing_date_offset(15) })
        .to_request();
    let amended: ApiResponse<Invoice> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(amended.data.unwrap().amount, 5200);

    // It shows in the org-wide list and the customer billing summary.
    let req = test::TestRequest::get()
        .uri(&format!("/api/orgs/{}/invoices", org.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let list: ApiResponse<Vec<Invoice>> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(list.data.unwrap().len(), 1);

    let req = test::TestRequest::get()
        .uri(&format!("/api/customers/{}/billing", dispatch.customer_id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let summary: ApiResponse<CustomerBillingSummary> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let summary = summary.data.unwrap();
    assert_eq!(summary.total_outstanding, 5200);
    assert_eq!(summary.overdue_count, 0);

    // Pay it -> PAID, and outstanding drops to zero.
    let req = test::TestRequest::post()
        .uri(&format!("/api/invoices/{}/pay", inv.id))
        .insert_header(("Authorization", auth.clone()))
        .to_request();
    let paid: ApiResponse<Invoice> =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(paid.data.unwrap().status, PaymentStatus::Paid);

    // Editing a paid invoice -> 409.
    let req = test::TestRequest::put()
        .uri(&format!("/api/invoices/{}", inv.id))
        .insert_header(("Authorization", auth))
        .set_json(&InvoicePayload { amount: 1, due_on: billing_date_offset(30) })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 409);
}

#[actix_web::test]
async fn test_create_invoice_rejects_bad_amount_with_400() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, auth) = setup_dispatch(&app, "Bad Amount Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/dispatches/{}/invoice", dispatch.id))
        .insert_header(("Authorization", auth))
        .set_json(&InvoicePayload { amount: 0, due_on: billing_date_offset(10) })
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_invoice_routes_403_for_a_different_org() {
    let _db = TestDb::create();
    let app = test::init_service(App::new().configure(config_routes)).await;
    let (_org, dispatch, auth) = setup_dispatch(&app, "Invoice Owner Org").await;
    let (_other, other_auth) = setup_org(&app, "Invoice Intruder Org").await;

    let req = test::TestRequest::post()
        .uri(&format!("/api/dispatches/{}/invoice", dispatch.id))
        .insert_header(("Authorization", auth))
        .set_json(&InvoicePayload { amount: 100, due_on: billing_date_offset(10) })
        .to_request();
    let created: ApiResponse<Invoice> =
        test::read_body_json(test::call_service(&app, req).await).await;
    let inv_id = created.data.unwrap().id;

    let req = test::TestRequest::post()
        .uri(&format!("/api/invoices/{}/pay", inv_id))
        .insert_header(("Authorization", other_auth))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status().as_u16(), 403);
}
