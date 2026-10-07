//! Dispatch orders: creating one from godown stock, listing, and moving it
//! through its status lifecycle (with proof of delivery).

use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::{
    DispatchLineItemInput, DispatchOrder, DispatchStatus, ProofOfDeliveryInput, VehicleSource,
};
use crate::logistics::driver::driver::Driver;
use crate::logistics::notification::notification::Notification;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::vendor::hire::VehicleHire;
use crate::logistics::vehicle::vehicle::Vehicle;
use actix_web::{get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DispatchRequestPayload {
    pub customer_id: Uuid,
    /// The stock lines this shipment carries. Must contain at least one item;
    /// a description may not be repeated.
    pub line_items: Vec<DispatchLineItemPayload>,
    /// `OWN` (default) picks a free vehicle from the org's fleet. `HIRED`
    /// reserves the stock and asks `vendor_id` for a truck: the dispatch
    /// starts `AWAITING_VEHICLE` until `PUT /api/vehicle-hires/{id}/assign`.
    #[serde(default)]
    pub vehicle_source: VehicleSource,
    /// Required when `vehicle_source` is `HIRED`: an active vendor of this org.
    #[serde(default)]
    pub vendor_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateDispatchStatusPayload {
    pub status: DispatchStatus,
    /// Required when `status` is `DELIVERED`; rejected with `400` if
    /// missing. Ignored for every other status.
    #[serde(default)]
    pub proof_of_delivery: Option<ProofOfDeliveryPayload>,
    /// Only used when `status` is `RETURNED`: the godown that should receive
    /// the returned stock. Optional — the server falls back to a godown that
    /// already holds one of the returned items, or the org's first godown.
    #[serde(default)]
    pub return_to_godown_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ProofOfDeliveryPayload {
    pub receiver_name: String,
    pub signature_or_photo_url: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DispatchOrderResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DispatchOrder>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DispatchOrderListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<DispatchOrder>>,
}

/// Load a dispatch by id and verify it belongs to `auth_org_id`, or build the
/// 403/404/500 response a handler should return early with.
pub(super) fn load_owned_dispatch(
    dispatch_id: Uuid,
    auth_org_id: Uuid,
) -> Result<DispatchOrder, HttpResponse> {
    match DispatchOrder::get_by_id(dispatch_id) {
        Ok(Some(dispatch)) if dispatch.org_id == auth_org_id => Ok(dispatch),
        Ok(Some(_)) => Err(HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: dispatch belongs to a different organization".to_string(),
            data: None,
        })),
        Ok(None) => Err(HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Dispatch order not found".to_string(),
            data: None,
        })),
        Err(err) => Err(
            HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch dispatch: {}", err),
                data: None,
            }),
        ),
    }
}

#[utoipa::path(
    get,
    path = "/api/dispatches",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of dispatch orders for authenticated organization", body = DispatchOrderListResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/dispatches")]
pub async fn list_dispatches(auth: AuthenticatedOrg) -> impl Responder {
    match DispatchOrder::list_by_org(auth.org_id) {
        Ok(orders) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} dispatch orders", orders.len()),
            data: Some(orders),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list dispatch orders: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/dispatch",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = DispatchRequestPayload,
    responses(
        (status = 200, description = "Stock dispatched successfully", body = DispatchOrderResponse),
        (status = 400, description = "Dispatch request failed", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/dispatch")]
pub async fn dispatch_stock(
    path: web::Path<Uuid>,
    payload: web::Json<DispatchRequestPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied: you can only dispatch from your own organization".to_string(),
            data: None,
        });
    }
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        godowns: Vec::new(),
        location: None,
    };

    let customer = match Customer::get_by_id(payload.customer_id) {
        Ok(Some(c)) if c.org_id == auth.org_id => c,
        Ok(Some(_)) => {
            return HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: "Customer belongs to a different organization".to_string(),
                data: None,
            })
        }
        Ok(None) => {
            return HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: "Customer not found".to_string(),
                data: None,
            })
        }
        Err(err) => {
            return HttpResponse::InternalServerError().json(ApiResponse::<String> {
                success: false,
                message: format!("Failed to fetch customer: {}", err),
                data: None,
            })
        }
    };

    let line_items: Vec<DispatchLineItemInput> = payload
        .line_items
        .iter()
        .map(|li| DispatchLineItemInput {
            stock_description: li.stock_description.clone(),
            requested_quantity: li.requested_quantity,
        })
        .collect();

    let result = match (payload.vehicle_source, payload.vendor_id) {
        (VehicleSource::Own, _) => org.dispatch_stock_to_customer(&customer, &line_items),
        (VehicleSource::Hired, Some(vendor_id)) => {
            org.dispatch_stock_on_hired_vehicle(&customer, &line_items, vendor_id)
        }
        (VehicleSource::Hired, None) => {
            return HttpResponse::BadRequest().json(ApiResponse::<String> {
                success: false,
                message: "vendor_id is required when vehicle_source is HIRED".to_string(),
                data: None,
            })
        }
    };

    match result {
        Ok(order) => {
            // Best-effort: tell the customer and the assigned driver. A
            // recording failure must not fail the dispatch itself. A hired
            // dispatch has no truck or driver yet; its notifications go out
            // when the vendor's truck is assigned.
            if order.vehicle_source == VehicleSource::Own {
                notify_dispatch_created(org_id, &order, &customer).await;
            }
            let message = if order.status == DispatchStatus::AwaitingVehicle {
                "Stock reserved; awaiting the hired vehicle"
            } else {
                "Stock dispatched successfully"
            };
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                message: message.to_string(),
                data: Some(order),
            })
        }
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: format!("Dispatch failed: {}", err),
            data: None,
        }),
    }
}

/// Record (and try to send) the dispatch-created notifications for one
/// dispatch: to its customer, and to whoever drives its truck.
pub(super) async fn notify_dispatch_created(org_id: Uuid, order: &DispatchOrder, customer: &Customer) {
    let driver_phone = driver_phone_for_dispatch(order, org_id);
    if let Ok(notifs) =
        Notification::record_dispatch_created(org_id, order.id, customer, driver_phone.as_deref()).await
    {
        Notification::deliver_queued_best_effort(&notifs).await;
    }
}

/// The phone of whoever drives this dispatch's truck: the hired driver on a
/// hired dispatch, otherwise the active driver assigned to the own vehicle.
fn driver_phone_for_dispatch(order: &DispatchOrder, org_id: Uuid) -> Option<String> {
    match order.vehicle_source {
        VehicleSource::Hired => {
            VehicleHire::get_by_id(order.hire_id?).ok().flatten()?.driver_phone
        }
        VehicleSource::Own => driver_phone_for_vehicle(order.vehicle_registration_number.as_deref()?, org_id),
    }
}

/// The phone number of the active driver assigned to `reg`, if any.
fn driver_phone_for_vehicle(reg: &str, org_id: Uuid) -> Option<String> {
    let vehicle = Vehicle::list_by_org(org_id)
        .ok()?
        .into_iter()
        .find(|v| v.registration_number == reg)?;
    let driver_id = vehicle.assigned_driver_id?;
    Driver::get_by_id(driver_id).ok().flatten().map(|d| d.phone)
}

#[utoipa::path(
    put,
    path = "/api/dispatches/{id}/status",
    tag = "Dispatch",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Dispatch order UUID")),
    request_body = UpdateDispatchStatusPayload,
    responses(
        (status = 200, description = "Dispatch status updated", body = DispatchOrderResponse),
        (status = 400, description = "Illegal transition for the dispatch's current status", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Dispatch not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[put("/dispatches/{id}/status")]
pub async fn update_dispatch_status(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateDispatchStatusPayload>,
    auth: AuthenticatedOrg,
) -> impl Responder {
    if let Err(resp) = auth.require_role(&DISPATCH_ROLES) {
        return resp;
    }
    let mut dispatch = match load_owned_dispatch(path.into_inner(), auth.org_id) {
        Ok(d) => d,
        Err(resp) => return resp,
    };

    let proof = payload
        .proof_of_delivery
        .as_ref()
        .map(|p| ProofOfDeliveryInput {
            receiver_name: p.receiver_name.clone(),
            signature_or_photo_url: p.signature_or_photo_url.clone(),
        });

    match dispatch.transition_to(payload.status, proof, payload.return_to_godown_id) {
        Ok(()) => {
            if dispatch.status == DispatchStatus::Delivered {
                if let Ok(Some(customer)) = Customer::get_by_id(dispatch.customer_id) {
                    if let Ok(notifs) =
                        Notification::record_dispatch_delivered(auth.org_id, dispatch.id, &customer).await
                    {
                        Notification::deliver_queued_best_effort(&notifs).await;
                    }
                }
            }
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                message: format!("Dispatch status updated to {}", dispatch.status),
                data: Some(dispatch),
            })
        }
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: format!("Status update failed: {}", err),
            data: None,
        }),
    }
}
