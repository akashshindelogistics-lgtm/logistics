//! Multi-stop trips that group several dispatches onto one vehicle run.

use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::{DispatchLineItemInput, VehicleSource};
use crate::logistics::dispatch::trip::Trip;
use crate::logistics::orgs::orgs::Organization;
use actix_web::{get, post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use super::common::*;
use super::dispatches::notify_dispatch_created;

/// One stop on a multi-stop trip: a customer and the stock lines to drop there.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct TripStopPayload {
    pub customer_id: Uuid,
    pub line_items: Vec<DispatchLineItemPayload>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateTripPayload {
    /// The stops in the order the vehicle should visit them. At least two, and
    /// no customer twice.
    pub stops: Vec<TripStopPayload>,
    /// When true, stops 2..N are reordered by nearest-neighbour geography
    /// before the trip is created — `stops[0]` is always kept as the route's
    /// fixed starting point. Defaults to false (visit `stops` in the exact
    /// order given).
    #[serde(default)]
    pub optimize_route: bool,
    /// As on a single dispatch: `HIRED` puts the whole trip on one truck
    /// hired from `vendor_id`.
    #[serde(default)]
    pub vehicle_source: VehicleSource,
    #[serde(default)]
    pub vendor_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TripResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Trip>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TripListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Trip>>,
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/trips",
    tag = "Trips",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    request_body = CreateTripPayload,
    responses(
        (status = 200, description = "Trip created; one dispatch per stop", body = TripResponse),
        (status = 400, description = "Trip could not be planned", body = EmptyResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/trips")]
pub async fn create_trip(
    path: web::Path<Uuid>,
    payload: web::Json<CreateTripPayload>,
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

    // Resolve every stop's customer up front, checking org ownership.
    let mut resolved: Vec<(Customer, Vec<DispatchLineItemInput>)> =
        Vec::with_capacity(payload.stops.len());
    for (i, stop) in payload.stops.iter().enumerate() {
        let customer = match Customer::get_by_id(stop.customer_id) {
            Ok(Some(c)) if c.org_id == org_id => c,
            Ok(Some(_)) => {
                return HttpResponse::BadRequest().json(ApiResponse::<String> {
                    success: false,
                    message: format!("Stop {}: customer belongs to a different organization", i + 1),
                    data: None,
                })
            }
            Ok(None) => {
                return HttpResponse::BadRequest().json(ApiResponse::<String> {
                    success: false,
                    message: format!("Stop {}: customer not found", i + 1),
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
        let lines = stop
            .line_items
            .iter()
            .map(|li| DispatchLineItemInput {
                stock_description: li.stock_description.clone(),
                requested_quantity: li.requested_quantity,
            })
            .collect();
        resolved.push((customer, lines));
    }

    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        godowns: Vec::new(),
        location: None,
    };
    let stops_ref: Vec<(&Customer, &[DispatchLineItemInput])> =
        resolved.iter().map(|(c, l)| (c, l.as_slice())).collect();

    let result = match (payload.vehicle_source, payload.vendor_id) {
        (VehicleSource::Own, _) => org.dispatch_trip_to_customers(&stops_ref, payload.optimize_route),
        (VehicleSource::Hired, Some(vendor_id)) => {
            org.dispatch_trip_on_hired_vehicle(&stops_ref, payload.optimize_route, vendor_id)
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
        Ok(trip) => {
            // Best-effort per-stop notifications; a hired trip's go out when
            // its truck is assigned.
            if trip.vehicle_source == VehicleSource::Own {
                for stop in &trip.stops {
                    if let Some((customer, _)) = resolved.iter().find(|(c, _)| c.id == stop.customer_id) {
                        notify_dispatch_created(org_id, stop, customer).await;
                    }
                }
            }
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                message: format!("Trip created with {} stops", trip.stops.len()),
                data: Some(trip),
            })
        }
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: format!("Trip failed: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}/trips",
    tag = "Trips",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Organization UUID")),
    responses(
        (status = 200, description = "The org's trips, newest first", body = TripListResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}/trips")]
pub async fn list_org_trips(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    let org_id = path.into_inner();
    if org_id != auth.org_id {
        return HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "Access denied".to_string(),
            data: None,
        });
    }
    match Trip::list_by_org(org_id) {
        Ok(trips) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} trips", trips.len()),
            data: Some(trips),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list trips: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/trips/{id}",
    tag = "Trips",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "Trip UUID")),
    responses(
        (status = 200, description = "The trip and its stops", body = TripResponse),
        (status = 403, description = "Forbidden", body = EmptyResponse),
        (status = 404, description = "Trip not found", body = EmptyResponse),
        (status = 401, description = "Unauthorized", body = EmptyResponse)
    )
)]
#[get("/trips/{id}")]
pub async fn get_trip(path: web::Path<Uuid>, auth: AuthenticatedOrg) -> impl Responder {
    match Trip::get_by_id(path.into_inner()) {
        Ok(Some(trip)) if trip.org_id == auth.org_id => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Trip retrieved".to_string(),
            data: Some(trip),
        }),
        Ok(Some(_)) => HttpResponse::Forbidden().json(ApiResponse::<String> {
            success: false,
            message: "That trip belongs to a different organization".to_string(),
            data: None,
        }),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Trip not found".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to fetch trip: {}", err),
            data: None,
        }),
    }
}
