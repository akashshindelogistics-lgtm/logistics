//! Customers (`/api/customers`, `/api/orgs/{id}/customers`).

use crate::logistics::customer::customer::Customer;
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateCustomerPayload {
    pub name: String,
    pub address: String,
    /// Optional initial delivery location. When both `latitude` and
    /// `longitude` are supplied the customer is created and then immediately
    /// located, exactly as a follow-up `PUT /api/customers/{id}/location`
    /// would. `location_address` is an optional human-readable label for the
    /// pin; when omitted the customer's `address` is used.
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub location_address: Option<String>,
    /// Optional contact details. Dispatch notifications go to the email if
    /// set, otherwise the phone. Either can be left out.
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Customer>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Customer>>,
}

/// Load a customer by id and verify it belongs to `auth_org_id`, or build the
/// 403/404/500 response a handler should return early with. Mirrors
/// `load_owned_driver`.
pub(super) fn load_owned_customer(customer_id: Uuid, auth_org_id: Uuid) -> Result<Customer, HttpResponse> {
    match Customer::get_by_id(customer_id) {
        Ok(Some(customer)) if customer.org_id == auth_org_id => Ok(customer),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: customer belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Customer not found".to_string(),
            data: None,
        })),
        Err(err) => Err(HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch customer: {}", err),
            data: None,
        })),
    }
}

#[utoipa::path(
    get,
    path = "/api/customers",
    tag = "Customers",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Customers for the authenticated organization", body = CustomerListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/customers")]
pub async fn list_customers(auth: AuthenticatedOrg) -> impl Responder {
    match Customer::list_by_org(auth.org_id) {
        Ok(customers) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} customers", customers.len()),
            data: Some(customers),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list customers: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/customers",
    tag = "Customers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateCustomerPayload,
    responses(
        (status = 201, description = "Customer created successfully", body = CustomerResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/customers")]
pub async fn add_customer(
    path: web::Path<Uuid>,
    payload: web::Json<CreateCustomerPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only add customers to your own organization"
                .to_string(),
            data: None,
        });
    }
    match Customer::create_customer(org_id, &payload.name, &payload.address) {
        Ok(mut customer) => {
            if let (Some(latitude), Some(longitude)) = (payload.latitude, payload.longitude) {
                let label = payload
                    .location_address
                    .clone()
                    .unwrap_or_else(|| payload.address.clone());
                if let Err(err) = customer.update_location(latitude, longitude, Some(label)) {
                    return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                        success: false,
                        message: format!("Customer created but locating it failed: {}", err),
                        data: None,
                    });
                }
            }
            if payload.phone.is_some() || payload.email.is_some() {
                if let Err(err) = customer.set_contact(payload.phone.clone(), payload.email.clone()) {
                    return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                        success: false,
                        message: format!("Customer created but saving contact details failed: {}", err),
                        data: None,
                    });
                }
            }
            HttpResponse::Created().json(ApiResponse {
                success: true,
                message: "Customer created successfully".to_string(),
                data: Some(customer),
            })
        }
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create customer: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/customers/{id}/location",
    tag = "Customers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Customer UUID")),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Customer location updated successfully", body = LocationResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Customer not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/customers/{id}/location")]
pub async fn update_customer_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    let mut customer = match load_owned_customer(path.into_inner(), auth.org_id) {
        Ok(c) => c,
        Err(resp) => return resp,
    };

    match customer.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Customer location updated successfully".to_string(),
            data: customer.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update customer location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/customers/{id}",
    tag = "Customers",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Customer UUID")),
    responses(
        (status = 200, description = "Customer deleted successfully", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Customer not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[delete("/customers/{id}")]
pub async fn delete_customer(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let customer = match load_owned_customer(path.into_inner(), auth.org_id) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    match customer.delete() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Customer deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete customer: {}", err),
            data: None,
        }),
    }
}
