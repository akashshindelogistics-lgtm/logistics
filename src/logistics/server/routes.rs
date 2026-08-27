use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::DispatchOrder;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::stock::stock::Stock;
use crate::logistics::vehicle::vehicle::{Location, Unit, Vehicle};
use actix_web::{delete, get, post, put, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateOrgPayload {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateOrgPayload {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct LocationPayload {
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateVehiclePayload {
    pub registration_number: String,
    pub capacity: i64,
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateStockPayload {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateCustomerPayload {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct DispatchRequestPayload {
    pub customer_id: Uuid,
    pub stock_description: String,
    pub requested_quantity: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiResponse<T: ToSchema> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

// Concrete response types for utoipa schema registration
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrgResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Organization>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vehicle>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct StockResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Stock>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Customer>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DispatchOrderResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<DispatchOrder>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LocationResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Location>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EmptyResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct OrgListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Organization>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct VehicleListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Vehicle>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<Customer>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DispatchOrderListResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<Vec<DispatchOrder>>,
}

#[utoipa::path(
    get,
    path = "/api/orgs",
    tag = "Organizations",
    responses(
        (status = 200, description = "List of all organizations", body = OrgListResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[get("/orgs")]
pub async fn list_orgs() -> impl Responder {
    match Organization::list_all() {
        Ok(orgs) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} organizations", orgs.len()),
            data: Some(orgs),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list organizations: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/orgs/{id}",
    tag = "Organizations",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    responses(
        (status = 200, description = "Organization detail with vehicles and stock", body = OrgResponse),
        (status = 404, description = "Organization not found", body = EmptyResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[get("/orgs/{id}")]
pub async fn get_org(path: web::Path<Uuid>) -> impl Responder {
    let org_id = path.into_inner();
    match Organization::get_by_id(org_id) {
        Ok(Some(org)) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Organization retrieved successfully".to_string(),
            data: Some(org),
        }),
        Ok(None) => HttpResponse::NotFound().json(ApiResponse::<String> {
            success: false,
            message: "Organization not found".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to retrieve organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/vehicles",
    tag = "Vehicles",
    responses(
        (status = 200, description = "List of all vehicles", body = VehicleListResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[get("/vehicles")]
pub async fn list_vehicles() -> impl Responder {
    match Vehicle::list_all() {
        Ok(vehicles) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: format!("Retrieved {} vehicles", vehicles.len()),
            data: Some(vehicles),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to list vehicles: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    get,
    path = "/api/customers",
    tag = "Customers",
    responses(
        (status = 200, description = "List of all customers", body = CustomerListResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[get("/customers")]
pub async fn list_customers() -> impl Responder {
    match Customer::list_all() {
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
    get,
    path = "/api/dispatches",
    tag = "Dispatch",
    responses(
        (status = 200, description = "List of all dispatch orders", body = DispatchOrderListResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[get("/dispatches")]
pub async fn list_dispatches() -> impl Responder {
    match DispatchOrder::list_all() {
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
    get,
    path = "/api/health",
    tag = "Health",
    responses(
        (status = 200, description = "System is operational", body = EmptyResponse)
    )
)]
#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse::<String> {
        success: true,
        message: "Logistics system REST API operational".to_string(),
        data: None,
    })
}

#[utoipa::path(
    post,
    path = "/api/orgs",
    tag = "Organizations",
    request_body = CreateOrgPayload,
    responses(
        (status = 201, description = "Organization created successfully", body = OrgResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[post("/orgs")]
pub async fn create_org(payload: web::Json<CreateOrgPayload>) -> impl Responder {
    match Organization::create_organization(&payload.name, &payload.address) {
        Ok(org) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Organization created successfully".to_string(),
            data: Some(org),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to create organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/orgs/{id}",
    tag = "Organizations",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    request_body = UpdateOrgPayload,
    responses(
        (status = 200, description = "Organization updated successfully", body = OrgResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}")]
pub async fn update_org(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateOrgPayload>,
) -> impl Responder {
    let org_id = path.into_inner();
    let mut org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    match org.update_organization(&payload.name, &payload.address) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Organization updated successfully".to_string(),
            data: Some(org),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/orgs/{id}/location",
    tag = "Organizations",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Organization location updated successfully", body = LocationResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}/location")]
pub async fn update_org_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
) -> impl Responder {
    let org_id = path.into_inner();
    let mut org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    match org.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Organization location updated successfully".to_string(),
            data: org.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update organization location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/orgs/{id}",
    tag = "Organizations",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    responses(
        (status = 200, description = "Organization deleted successfully", body = EmptyResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[delete("/orgs/{id}")]
pub async fn delete_org(path: web::Path<Uuid>) -> impl Responder {
    let org_id = path.into_inner();
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    match org.remove_organization() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Organization deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete organization: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/vehicles",
    tag = "Vehicles",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    request_body = CreateVehiclePayload,
    responses(
        (status = 201, description = "Vehicle registered successfully", body = VehicleResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/vehicles")]
pub async fn add_vehicle(
    path: web::Path<Uuid>,
    payload: web::Json<CreateVehiclePayload>,
) -> impl Responder {
    let org_id = path.into_inner();
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    let unit = Unit::from_str(&payload.unit);
    let vehicle = Vehicle::new(&payload.registration_number, payload.capacity, unit);

    match vehicle.add_new_vehicle_to_org(&org) {
        Ok(_) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Vehicle registered successfully".to_string(),
            data: Some(vehicle),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to register vehicle: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/vehicles/{reg}/location",
    tag = "Vehicles",
    params(
        ("reg" = String, Path, description = "Vehicle registration number")
    ),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Vehicle location updated successfully", body = LocationResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[put("/vehicles/{reg}/location")]
pub async fn update_vehicle_location(
    path: web::Path<String>,
    payload: web::Json<LocationPayload>,
) -> impl Responder {
    let reg_number = path.into_inner();
    let mut vehicle = Vehicle::new(&reg_number, 0, Unit::MetricTon);

    match vehicle.update_location(payload.latitude, payload.longitude, payload.address.clone()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Vehicle location updated successfully".to_string(),
            data: vehicle.location,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update vehicle location: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/vehicles/{reg}",
    tag = "Vehicles",
    params(
        ("reg" = String, Path, description = "Vehicle registration number")
    ),
    responses(
        (status = 200, description = "Vehicle deleted successfully", body = EmptyResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[delete("/vehicles/{reg}")]
pub async fn delete_vehicle(path: web::Path<String>) -> impl Responder {
    let reg_number = path.into_inner();
    let vehicle = Vehicle::new(&reg_number, 0, Unit::MetricTon);

    match vehicle.remove_vehicle() {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Vehicle deleted successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to delete vehicle: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/orgs/{id}/stock",
    tag = "Stock",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    request_body = CreateStockPayload,
    responses(
        (status = 201, description = "Stock added successfully", body = StockResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/stock")]
pub async fn add_stock(
    path: web::Path<Uuid>,
    payload: web::Json<CreateStockPayload>,
) -> impl Responder {
    let org_id = path.into_inner();
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    let stock = Stock::new(
        payload.volume_in_size,
        payload.quantity,
        &payload.description,
    );

    match stock.add_new_stock(&org) {
        Ok(_) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Stock added successfully".to_string(),
            data: Some(stock),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to add stock: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    put,
    path = "/api/orgs/{id}/stock",
    tag = "Stock",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    request_body = UpdateStockPayload,
    responses(
        (status = 200, description = "Stock updated successfully", body = StockResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[put("/orgs/{id}/stock")]
pub async fn update_stock(
    path: web::Path<Uuid>,
    payload: web::Json<UpdateStockPayload>,
) -> impl Responder {
    let org_id = path.into_inner();
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    let mut stock = Stock::new(0, 0, &payload.description);
    match stock.update_stock(&org, payload.volume_in_size, payload.quantity) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Stock updated successfully".to_string(),
            data: Some(stock),
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to update stock: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    delete,
    path = "/api/orgs/{id}/stock/{desc}",
    tag = "Stock",
    params(
        ("id" = Uuid, Path, description = "Organization UUID"),
        ("desc" = String, Path, description = "Stock item description")
    ),
    responses(
        (status = 200, description = "Stock removed successfully", body = EmptyResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[delete("/orgs/{id}/stock/{desc}")]
pub async fn delete_stock(path: web::Path<(Uuid, String)>) -> impl Responder {
    let (org_id, desc) = path.into_inner();
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    let stock = Stock::new(0, 0, &desc);
    match stock.remove_stock(&org) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse::<String> {
            success: true,
            message: "Stock removed successfully".to_string(),
            data: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(ApiResponse::<String> {
            success: false,
            message: format!("Failed to remove stock: {}", err),
            data: None,
        }),
    }
}

#[utoipa::path(
    post,
    path = "/api/customers",
    tag = "Customers",
    request_body = CreateCustomerPayload,
    responses(
        (status = 201, description = "Customer created successfully", body = CustomerResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[post("/customers")]
pub async fn create_customer(payload: web::Json<CreateCustomerPayload>) -> impl Responder {
    match Customer::create_customer(&payload.name, &payload.address) {
        Ok(customer) => HttpResponse::Created().json(ApiResponse {
            success: true,
            message: "Customer created successfully".to_string(),
            data: Some(customer),
        }),
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
    params(
        ("id" = Uuid, Path, description = "Customer UUID")
    ),
    request_body = LocationPayload,
    responses(
        (status = 200, description = "Customer location updated successfully", body = LocationResponse),
        (status = 500, description = "Internal server error", body = EmptyResponse)
    )
)]
#[put("/customers/{id}/location")]
pub async fn update_customer_location(
    path: web::Path<Uuid>,
    payload: web::Json<LocationPayload>,
) -> impl Responder {
    let customer_id = path.into_inner();
    let mut customer = Customer {
        id: customer_id,
        name: String::new(),
        address: String::new(),
        location: None,
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
    post,
    path = "/api/orgs/{id}/dispatch",
    tag = "Dispatch",
    params(
        ("id" = Uuid, Path, description = "Organization UUID")
    ),
    request_body = DispatchRequestPayload,
    responses(
        (status = 200, description = "Stock dispatched successfully", body = DispatchOrderResponse),
        (status = 400, description = "Dispatch request failed", body = EmptyResponse)
    )
)]
#[post("/orgs/{id}/dispatch")]
pub async fn dispatch_stock(
    path: web::Path<Uuid>,
    payload: web::Json<DispatchRequestPayload>,
) -> impl Responder {
    let org_id = path.into_inner();
    let org = Organization {
        id: org_id,
        name: String::new(),
        address: String::new(),
        vehicles: Vec::new(),
        stock: Vec::new(),
        location: None,
    };

    let customer = Customer {
        id: payload.customer_id,
        name: String::new(),
        address: String::new(),
        location: None,
    };

    match org.dispatch_stock_to_customer(
        &customer,
        &payload.stock_description,
        payload.requested_quantity,
    ) {
        Ok(order) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "Stock dispatched successfully".to_string(),
            data: Some(order),
        }),
        Err(err) => HttpResponse::BadRequest().json(ApiResponse::<String> {
            success: false,
            message: format!("Dispatch failed: {}", err),
            data: None,
        }),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        list_orgs,
        get_org,
        create_org,
        update_org,
        update_org_location,
        delete_org,
        list_vehicles,
        add_vehicle,
        update_vehicle_location,
        delete_vehicle,
        add_stock,
        update_stock,
        delete_stock,
        list_customers,
        create_customer,
        update_customer_location,
        list_dispatches,
        dispatch_stock,
    ),
    components(
        schemas(
            CreateOrgPayload,
            UpdateOrgPayload,
            LocationPayload,
            CreateVehiclePayload,
            CreateStockPayload,
            UpdateStockPayload,
            CreateCustomerPayload,
            DispatchRequestPayload,
            Organization,
            Vehicle,
            Unit,
            Location,
            Stock,
            Customer,
            DispatchOrder,
            OrgResponse,
            OrgListResponse,
            VehicleResponse,
            VehicleListResponse,
            StockResponse,
            CustomerResponse,
            CustomerListResponse,
            DispatchOrderResponse,
            DispatchOrderListResponse,
            LocationResponse,
            EmptyResponse,
        )
    ),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Organizations", description = "Organization management endpoints"),
        (name = "Vehicles", description = "Vehicle fleet management endpoints"),
        (name = "Stock", description = "Stock inventory management endpoints"),
        (name = "Customers", description = "Customer management endpoints"),
        (name = "Dispatch", description = "Automated stock dispatch endpoints"),
    )
)]
pub struct ApiDoc;

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(health_check)
            .service(list_orgs)
            .service(get_org)
            .service(create_org)
            .service(update_org)
            .service(update_org_location)
            .service(delete_org)
            .service(list_vehicles)
            .service(add_vehicle)
            .service(update_vehicle_location)
            .service(delete_vehicle)
            .service(add_stock)
            .service(update_stock)
            .service(delete_stock)
            .service(list_customers)
            .service(create_customer)
            .service(update_customer_location)
            .service(list_dispatches)
            .service(dispatch_stock),
    )
    .service(
        SwaggerUi::new("/swagger-ui/{_:.*}")
            .url("/api-docs/openapi.json", ApiDoc::openapi()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

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
        // Swagger UI returns 200 or a redirect to index.html
        assert!(resp.status().is_success() || resp.status().is_redirection());
    }

    #[actix_web::test]
    async fn test_openapi_json_spec_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get()
            .uri("/api-docs/openapi.json")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_create_org_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let payload = CreateOrgPayload {
            name: "API Test Express Org".to_string(),
            address: "100 Server Hub, Cyber City".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .set_json(&payload)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);

        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        assert!(body.success);
        assert!(body.data.is_some());
        let org = body.data.unwrap();
        assert_eq!(org.name, "API Test Express Org");
    }

    #[actix_web::test]
    async fn test_create_org_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::post()
            .uri("/api/orgs")
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_org_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = UpdateOrgPayload {
            name: "Updated Org Name".to_string(),
            address: "456 Updated Ave, New City".to_string(),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}", org_id))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        // UPDATE with non-existent UUID succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Organization> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.data.unwrap().name, "Updated Org Name");
    }

    #[actix_web::test]
    async fn test_update_org_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}", org_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_org_location_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = LocationPayload {
            latitude: 28.6139,
            longitude: 77.2090,
            address: Some("New Delhi, India".to_string()),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/location", org_id))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        assert!(body.success);
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 28.6139);
        assert_eq!(loc.longitude, 77.2090);
        assert_eq!(loc.address.as_deref(), Some("New Delhi, India"));
    }

    #[actix_web::test]
    async fn test_update_org_location_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/location", org_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_delete_org_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::delete()
            .uri(&format!("/api/orgs/{}", org_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        // DELETE with non-existent UUID succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Organization deleted successfully");
    }

    #[actix_web::test]
    async fn test_add_vehicle_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_add_vehicle_to_nonexistent_org_returns_error() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = CreateVehiclePayload {
            registration_number: "ZZ01 XX 0001".to_string(),
            capacity: 10,
            unit: "MetricTon".to_string(),
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/vehicles", org_id))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        // FK constraint violation (org doesn't exist) → 500
        assert_eq!(resp.status().as_u16(), 500);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.message.contains("Failed to register vehicle"));
    }

    #[actix_web::test]
    async fn test_update_vehicle_location_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let payload = LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Mumbai, Maharashtra".to_string()),
        };
        let req = test::TestRequest::put()
            .uri("/api/vehicles/NONEXISTENT-REG-001/location")
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        // UPDATE with non-existent reg succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        assert!(body.success);
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 19.0760);
        assert_eq!(loc.longitude, 72.8777);
    }

    #[actix_web::test]
    async fn test_update_vehicle_location_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::put()
            .uri("/api/vehicles/MH12EN3502/location")
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_delete_vehicle_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::delete()
            .uri("/api/vehicles/NONEXISTENT-REG-002")
            .to_request();
        let resp = test::call_service(&app, req).await;
        // DELETE with non-existent reg succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Vehicle deleted successfully");
    }

    #[actix_web::test]
    async fn test_add_stock_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/stock", org_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_stock_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let payload = UpdateStockPayload {
            volume_in_size: 200,
            quantity: 75,
            description: "Nonexistent Stock Description".to_string(),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/stock", org_id))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        // UPDATE with non-existent org/desc succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Stock> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Stock updated successfully");
    }

    #[actix_web::test]
    async fn test_update_stock_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/orgs/{}/stock", org_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_delete_stock_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::delete()
            .uri(&format!("/api/orgs/{}/stock/nonexistent-description", org_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        // DELETE with non-existent org/desc succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(body.success);
        assert_eq!(body.message, "Stock removed successfully");
    }

    #[actix_web::test]
    async fn test_create_customer_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let payload = CreateCustomerPayload {
            name: "API Test Customer".to_string(),
            address: "100 Test Lane, Mumbai".to_string(),
        };
        let req = test::TestRequest::post()
            .uri("/api/customers")
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 201);
        let body: ApiResponse<Customer> = test::read_body_json(resp).await;
        assert!(body.success);
        let customer = body.data.unwrap();
        assert_eq!(customer.name, "API Test Customer");
        assert_eq!(customer.address, "100 Test Lane, Mumbai");
    }

    #[actix_web::test]
    async fn test_create_customer_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::post()
            .uri("/api/customers")
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_customer_location_endpoint() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let customer_id = Uuid::new_v4();
        let payload = LocationPayload {
            latitude: 19.0760,
            longitude: 72.8777,
            address: Some("Bandra West, Mumbai".to_string()),
        };
        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer_id))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        // UPDATE with non-existent customer succeeds (0 rows affected, no error)
        assert_eq!(resp.status().as_u16(), 200);
        let body: ApiResponse<Location> = test::read_body_json(resp).await;
        assert!(body.success);
        let loc = body.data.unwrap();
        assert_eq!(loc.latitude, 19.0760);
        assert_eq!(loc.longitude, 72.8777);
        assert_eq!(loc.address.as_deref(), Some("Bandra West, Mumbai"));
    }

    #[actix_web::test]
    async fn test_update_customer_location_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let customer_id = Uuid::new_v4();
        let req = test::TestRequest::put()
            .uri(&format!("/api/customers/{}/location", customer_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_dispatch_stock_no_stock_returns_error() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let payload = DispatchRequestPayload {
            customer_id,
            stock_description: "Nonexistent Stock Description".to_string(),
            requested_quantity: 10,
        };
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org_id))
            .set_json(&payload)
            .to_request();
        let resp = test::call_service(&app, req).await;
        // Stock not found (or DB error) → 400 Bad Request
        assert_eq!(resp.status().as_u16(), 400);
        let body: ApiResponse<String> = test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.message.contains("Dispatch failed"));
    }

    #[actix_web::test]
    async fn test_dispatch_stock_invalid_payload() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let org_id = Uuid::new_v4();
        let req = test::TestRequest::post()
            .uri(&format!("/api/orgs/{}/dispatch", org_id))
            .insert_header(("Content-Type", "application/json"))
            .set_payload("{bad json}")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }
}
