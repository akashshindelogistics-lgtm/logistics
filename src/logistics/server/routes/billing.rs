//! Freight billing: dispatch invoices, payments and per-customer billing
//! summaries.

use crate::logistics::billing::invoice::{CustomerBillingSummary, Invoice, InvoiceError};
use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;
use super::customers::load_owned_customer;
use super::dispatches::load_owned_dispatch;

/// Raise or amend a freight invoice.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct InvoicePayload {
    /// Freight charge, in whole currency units. Must be greater than zero.
    pub amount: i64,
    /// Payment due date as ISO `YYYY-MM-DD`.
    pub due_on: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InvoiceResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Invoice>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InvoiceListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Invoice>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerBillingResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<CustomerBillingSummary>,
}

//
// One invoice per dispatch. Every route checks the underlying dispatch (or the
// invoice's stored org) belongs to the authenticated org. See docs/billing.md.

/// Load an invoice by id and verify it belongs to `auth_org_id`, or build the
/// 403/404/500 response to return early with.
fn load_owned_invoice(invoice_id: Uuid, auth_org_id: Uuid) -> Result<Invoice, HttpResponse> {
    match Invoice::get_by_id(invoice_id) {
        Ok(Some(inv)) if inv.org_id == auth_org_id => Ok(inv),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: invoice belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Invoice not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch invoice: {}", err),
            data: None,
        })),
    }
}

/// Map an `InvoiceError` to a caller-facing response: a validation problem is
/// a `400`, a duplicate/paid conflict a `409`, anything else a `500`.
fn invoice_error_response(err: InvoiceError) -> HttpResponse {
    match err {
        InvoiceError::InvalidDate(_) | InvoiceError::NonPositiveAmount => {
            HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: err.to_string(),
                data: None,
            })
        }
        InvoiceError::AlreadyInvoiced | InvoiceError::AlreadyPaid => {
            HttpResponse::Conflict().json(ApiResponse::<String> {
                success: false,
                message: err.to_string(),
                data: None,
            })
        }
        InvoiceError::Db(_) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to save invoice: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/dispatches/{id}/invoice",
    tag = "Billing",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Dispatch UUID")),
    request_body = InvoicePayload,
    responses(
        (status = 201, description = "Invoice raised", body = InvoiceResponse),
        (status = 400, description = "Amount not positive or due date invalid", body = EmptyResponse),
        (status = 409, description = "The dispatch already has an invoice", body = EmptyResponse),
        (status = 403, description = "Dispatch belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Dispatch not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/dispatches/{id}/invoice")]
pub async fn create_dispatch_invoice(
    path: web::Path<Uuid>,
    payload: web::Json<InvoicePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let dispatch = match load_owned_dispatch(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match Invoice::create(
        dispatch.org_id,
        dispatch.id,
        dispatch.customer_id,
        payload.amount,
        payload.due_on.clone(),
    ) {
        Ok(inv) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Invoice raised".to_string(),
            data: Some(inv),
        }),
        Err(err) => invoice_error_response(err),
    }
}

#[utoipa::path(
    get,
    path = "/api/dispatches/{id}/invoice",
    tag = "Billing",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Dispatch UUID")),
    responses(
        (status = 200, description = "The dispatch's invoice", body = InvoiceResponse),
        (status = 403, description = "Dispatch belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Dispatch or invoice not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/dispatches/{id}/invoice")]
pub async fn get_dispatch_invoice(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let dispatch = match load_owned_dispatch(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    match Invoice::get_by_dispatch(dispatch.id) {
        Ok(Some(inv)) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Invoice retrieved".to_string(),
            data: Some(inv),
        }),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "This dispatch has not been invoiced yet".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch invoice: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/invoices/{id}",
    tag = "Billing",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Invoice UUID")),
    request_body = InvoicePayload,
    responses(
        (status = 200, description = "Invoice updated", body = InvoiceResponse),
        (status = 400, description = "Amount not positive or due date invalid", body = EmptyResponse),
        (status = 409, description = "A paid invoice cannot be changed", body = EmptyResponse),
        (status = 403, description = "Invoice belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Invoice not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/invoices/{id}")]
pub async fn update_invoice(
    path: web::Path<Uuid>,
    payload: web::Json<InvoicePayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let mut inv = match load_owned_invoice(path.into_inner(), auth.org_id) {
        Ok(i) => i,
        Err(resp) => return resp,
    };
    match inv.update(payload.amount, payload.due_on.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Invoice updated".to_string(),
            data: Some(inv),
        }),
        Err(err) => invoice_error_response(err),
    }
}

#[utoipa::path(
    post,
    path = "/api/invoices/{id}/pay",
    tag = "Billing",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Invoice UUID")),
    responses(
        (status = 200, description = "Invoice marked paid (idempotent)", body = InvoiceResponse),
        (status = 403, description = "Invoice belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Invoice not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/invoices/{id}/pay")]
pub async fn pay_invoice(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let mut inv = match load_owned_invoice(path.into_inner(), auth.org_id) {
        Ok(i) => i,
        Err(resp) => return resp,
    };
    match inv.mark_paid() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Invoice marked paid".to_string(),
            data: Some(inv),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to mark invoice paid: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/invoices",
    tag = "Billing",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "Every invoice for the org, most recently issued first", body = InvoiceListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/invoices")]
pub async fn list_org_invoices(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match Invoice::list_by_org(org_id) {
        Ok(invoices) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} invoices", invoices.len()),
            data: Some(invoices),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list invoices: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/customers/{id}/billing",
    tag = "Billing",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Customer UUID")),
    responses(
        (status = 200, description = "The customer's payment standing", body = CustomerBillingResponse),
        (status = 403, description = "Customer belongs to a different organization", body = EmptyResponse),
        (status = 404, description = "Customer not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/customers/{id}/billing")]
pub async fn get_customer_billing(
    path: web::Path<Uuid>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let customer = match load_owned_customer(path.into_inner(), auth.org_id) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    match Invoice::customer_summary(customer.id) {
        Ok(summary) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Billing summary retrieved".to_string(),
            data: Some(summary),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to build billing summary: {}", err),
            data: None,
        }),
    }
}
