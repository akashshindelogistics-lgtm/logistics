//! The HTTP layer: every `/api` handler, its request/response types, the
//! OpenAPI document and the route table, split into one module per domain.
//!
//! Each domain module holds its handlers plus the payload/response types only
//! it uses; [`common`] holds the bearer-token extractor, role gates, the
//! `ApiResponse` envelope and the types shared across domains. Everything is
//! re-exported from here, so `routes::config_routes`, `routes::ApiDoc` and
//! `routes::<Type>` paths are unchanged.

use crate::logistics::ai::assistant::{AssistantAnswer, AssistantSource};
use crate::logistics::auth::auth::OrgSummary;
use crate::logistics::billing::invoice::{CustomerBillingSummary, Invoice, PaymentStatus};
use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::{
    DispatchLineItem, DispatchOrder, DispatchStatus, DispatchStatusEvent, ProofOfDelivery,
    VehicleSource,
};
use crate::logistics::dispatch::trip::{Trip, TripStatus};
use crate::logistics::driver::driver::Driver;
use crate::logistics::godown::godown::Godown;
use crate::logistics::godown::transfer::StockTransfer;
use crate::logistics::notification::notification::{
    Notification, NotificationChannel, NotificationEvent, NotificationStatus,
};
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::reports::{
    CategoryUnits, DeliveryPerformance, DispatchVolumePoint, GodownInventory, HireMargin,
    HiredTransport, OpsReport, VendorSpend, VehicleUtilization,
};
use crate::logistics::stock::stock::Stock;
use crate::logistics::user::user::{OrgRole, OrgUser};
use crate::logistics::vendor::hire::{HireStatus, VehicleHire, VendorPayment};
use crate::logistics::vendor::vendor::VehicleVendor;
use crate::logistics::vehicle::document::{ComplianceDocType, ComplianceStatus, VehicleDocument};
use crate::logistics::vehicle::maintenance::{MaintenanceStatus, VehicleMaintenance};
use crate::logistics::vehicle::vehicle::{Location, Unit, Vehicle};
use actix_web::web;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

mod common;
mod auth;
mod health;
mod orgs;
mod vehicles;
mod drivers;
mod vendors;
mod hires;
mod vehicle_documents;
mod maintenance;
mod godowns;
mod customers;
mod dispatches;
mod trips;
mod uploads;
mod ai;
mod billing;
mod reports;
mod notifications;
mod users;

pub use common::*;
pub use auth::*;
pub use health::*;
pub use orgs::*;
pub use vehicles::*;
pub use drivers::*;
pub use vendors::*;
pub use hires::*;
pub use vehicle_documents::*;
pub use maintenance::*;
pub use godowns::*;
pub use customers::*;
pub use dispatches::*;
pub use trips::*;
pub use uploads::*;
pub use ai::*;
pub use billing::*;
pub use reports::*;
pub use notifications::*;
pub use users::*;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    servers(
        (url = "http://127.0.0.1:8080", description = "Local development server"),
    ),
    paths(
        health_check,
        auth_orgs,
        auth_login,
        user_login,
        auth_me,
        list_org_users,
        add_org_user,
        update_org_user,
        delete_org_user,
        list_orgs,
        get_org,
        create_org,
        update_org,
        update_org_location,
        delete_org,
        list_vehicles,
        add_vehicle,
        edit_vehicle,
        update_vehicle_location,
        track_vehicle_location,
        rotate_vehicle_tracker_key,
        delete_vehicle,
        list_drivers,
        add_driver,
        update_driver,
        delete_driver,
        assign_vehicle_driver,
        list_vendors,
        add_vendor,
        update_vendor,
        delete_vendor,
        list_vehicle_hires,
        assign_vehicle_hire,
        record_vendor_payment,
        list_vendor_payments,
        list_vehicle_documents,
        add_vehicle_document,
        update_vehicle_document,
        delete_vehicle_document,
        list_org_vehicle_documents,
        list_vehicle_maintenance,
        add_vehicle_maintenance,
        update_vehicle_maintenance,
        record_vehicle_maintenance_mileage,
        delete_vehicle_maintenance,
        list_org_vehicle_maintenance,
        list_godowns,
        create_godown,
        get_godown,
        update_godown,
        delete_godown,
        update_godown_location,
        add_godown_stock,
        update_godown_stock,
        delete_godown_stock,
        transfer_godown_stock,
        list_stock_transfers,
        list_customers,
        add_customer,
        update_customer_location,
        delete_customer,
        list_dispatches,
        dispatch_stock,
        update_dispatch_status,
        get_dispatch_summary,
        ask_assistant,
        reindex_assistant,
        get_daily_digest,
        get_reorder_suggestions,
        create_dispatch_invoice,
        get_dispatch_invoice,
        update_invoice,
        pay_invoice,
        list_org_invoices,
        get_customer_billing,
        get_ops_report,
        get_ops_report_summary,
        list_dispatch_notifications,
        list_org_notifications,
        create_trip,
        list_org_trips,
        get_trip,
        upload_file,
        get_uploaded_file,
    ),
    components(
        schemas(
            LoginPayload, LoginData, UserLoginPayload, CreateUserPayload, UpdateUserPayload,
            OrgUser, OrgRole,
            CreateOrgPayload, UpdateOrgPayload, LocationPayload,
            CreateVehiclePayload, UpdateVehiclePayload, TrackLocationPayload, CreateStockPayload, UpdateStockPayload,
            CreateGodownPayload, UpdateGodownPayload,
            CreateCustomerPayload, DispatchRequestPayload, DispatchLineItemPayload,
            CreateDriverPayload, UpdateDriverPayload, AssignDriverPayload,
            CreateVendorPayload, UpdateVendorPayload, VehicleVendor,
            VehicleHire, HireStatus, AssignHirePayload, HireListQuery, VehicleSource,
            VendorPayment, VendorPaymentPayload, VendorPaymentListResponse,
            HiredTransport, VendorSpend, HireMargin,
            VehicleDocumentPayload, VehicleMaintenancePayload, RecordMileagePayload,
            TransferStockPayload,
            UpdateDispatchStatusPayload, ProofOfDeliveryPayload, InvoicePayload,
            Organization, Vehicle, Unit, Location, Stock, Godown, StockTransfer, Customer, Driver,
            VehicleDocument, ComplianceDocType, ComplianceStatus,
            VehicleMaintenance, MaintenanceStatus, VehicleMaintenanceResponse, VehicleMaintenanceListResponse,
            DispatchOrder, DispatchLineItem, DispatchStatus, DispatchStatusEvent, ProofOfDelivery,
            Invoice, PaymentStatus, CustomerBillingSummary,
            OpsReport, VehicleUtilization, DeliveryPerformance, GodownInventory, CategoryUnits, DispatchVolumePoint,
            Notification, NotificationEvent, NotificationChannel, NotificationStatus,
            Trip, TripStatus,
            OrgSummary,
            OrgResponse, OrgListResponse, VehicleResponse, VehicleListResponse,
            VehicleDocumentResponse, VehicleDocumentListResponse,
            StockResponse, GodownResponse, GodownListResponse,
            StockTransferResponse, StockTransferListResponse,
            CustomerResponse, CustomerListResponse,
            DriverResponse, DriverListResponse,
            VendorResponse, VendorListResponse, VehicleHireResponse, VehicleHireListResponse,
            DispatchOrderResponse, DispatchOrderListResponse,
            InvoiceResponse, InvoiceListResponse, CustomerBillingResponse,
            OpsReportResponse, UserResponse, UserListResponse, NotificationListResponse,
            TripResponse, TripListResponse, CreateTripPayload, TripStopPayload,
            UploadedFileResponse, UploadedFileRef,
            AssistantAskPayload, AssistantAnswer, AssistantSource, AssistantAnswerResponse,
            AssistantReindexResult, AssistantReindexResponse,
            LocationResponse, OrgSummaryListResponse, EmptyResponse,
        )
    ),
    tags(
        (name = "Health", description = "Health check"),
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Users", description = "Role-scoped team members within an organization"),
        (name = "Organizations", description = "Organization management"),
        (name = "Vehicles", description = "Vehicle fleet management"),
        (name = "Drivers", description = "Driver records and vehicle assignment"),
        (name = "Vehicle vendors", description = "Transporters and brokers the organization hires vehicles from, and the trucks hired from them"),
        (name = "Vehicle compliance", description = "Vehicle paperwork (insurance, RC, permit, PUC, fitness) and expiry tracking"),
        (name = "Vehicle maintenance", description = "Preventive maintenance scheduling by due date and/or odometer mileage, with alerts"),
        (name = "Godowns", description = "Warehouse (godown) and stock management"),
        (name = "Customers", description = "Customer management"),
        (name = "Dispatch", description = "Stock dispatch"),
        (name = "Billing", description = "Freight invoices and customer payment status"),
        (name = "Reports", description = "Operational reporting: fleet utilization, delivery performance, inventory, dispatch volume"),
        (name = "Notifications", description = "Customer and driver notifications recorded for each dispatch"),
        (name = "Trips", description = "Multi-stop trips: one vehicle serving several customer orders in sequence"),
        (name = "Uploads", description = "File storage for uploaded images, e.g. proof-of-delivery photos and signatures"),
        (name = "AI Assistant", description = "Retrieval-grounded natural-language Q&A over an organization's own data"),
    )
)]
pub struct ApiDoc;

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .service(health_check)
            .service(auth_orgs)
            .service(auth_login)
            .service(user_login)
            .service(auth_me)
            .service(list_org_users)
            .service(add_org_user)
            .service(update_org_user)
            .service(delete_org_user)
            .service(list_orgs)
            .service(get_org)
            .service(create_org)
            .service(update_org)
            .service(update_org_location)
            .service(delete_org)
            .service(list_vehicles)
            .service(add_vehicle)
            .service(edit_vehicle)
            .service(update_vehicle_location)
            .service(track_vehicle_location)
            .service(rotate_vehicle_tracker_key)
            .service(delete_vehicle)
            .service(list_drivers)
            .service(add_driver)
            .service(update_driver)
            .service(delete_driver)
            .service(assign_vehicle_driver)
            .service(list_vendors)
            .service(add_vendor)
            .service(update_vendor)
            .service(delete_vendor)
            .service(list_vehicle_hires)
            .service(assign_vehicle_hire)
            .service(record_vendor_payment)
            .service(list_vendor_payments)
            .service(list_vehicle_documents)
            .service(add_vehicle_document)
            .service(update_vehicle_document)
            .service(delete_vehicle_document)
            .service(list_org_vehicle_documents)
            .service(list_vehicle_maintenance)
            .service(add_vehicle_maintenance)
            .service(update_vehicle_maintenance)
            .service(record_vehicle_maintenance_mileage)
            .service(delete_vehicle_maintenance)
            .service(list_org_vehicle_maintenance)
            .service(list_godowns)
            .service(create_godown)
            .service(get_godown)
            .service(update_godown)
            .service(delete_godown)
            .service(update_godown_location)
            .service(add_godown_stock)
            .service(update_godown_stock)
            .service(delete_godown_stock)
            .service(transfer_godown_stock)
            .service(list_stock_transfers)
            .service(list_customers)
            .service(add_customer)
            .service(update_customer_location)
            .service(delete_customer)
            .service(list_dispatches)
            .service(dispatch_stock)
            .service(update_dispatch_status)
            .service(get_dispatch_summary)
            .service(ask_assistant)
            .service(reindex_assistant)
            .service(get_daily_digest)
            .service(get_reorder_suggestions)
            .service(create_dispatch_invoice)
            .service(get_dispatch_invoice)
            .service(update_invoice)
            .service(pay_invoice)
            .service(list_org_invoices)
            .service(get_customer_billing)
            .service(get_ops_report)
            .service(get_ops_report_summary)
            .service(list_dispatch_notifications)
            .service(list_org_notifications)
            .service(create_trip)
            .service(list_org_trips)
            .service(get_trip)
            .service(upload_file)
            .service(get_uploaded_file),
    )
    .service(
        SwaggerUi::new("/swagger-ui/{_:.*}")
            .url("/api-docs/openapi.json", ApiDoc::openapi()),
    );
}

#[cfg(test)]
mod tests;
