//! Hiring a vendor's truck for one dispatch or one multi-stop trip.
//!
//! Phase 2 of `docs/vehicle-vendors.md`. A hired dispatch is created as
//! `AWAITING_VEHICLE` with a `REQUESTED` hire (stock already reserved). After
//! phoning the vendor the dispatcher calls [`VehicleHire::assign`] with the
//! truck, driver and agreed rate, which moves the hire to `CONFIRMED` and
//! every dispatch it covers to `PENDING`. Once all of those dispatches finish,
//! [`VehicleHire::close_if_finished`] moves the hire to `RELEASED`, or to
//! `CANCELLED` if a truck was never assigned.
//!
//! A hired truck never enters the `Vehicle` table: its number is free text on
//! the hire and is copied onto the dispatches.

use crate::logistics::db::connection::DbConnection;
use crate::logistics::dispatch::dispatch::DispatchOrder;
use crate::logistics::vehicle::vehicle::Unit;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HireStatus {
    /// The vendor has been asked for a truck; none assigned yet.
    Requested,
    /// The vendor's truck, driver and rate are recorded; the trip is on.
    Confirmed,
    /// Every dispatch the hire covered has finished; the truck is let go.
    Released,
    /// Called off before a truck was assigned.
    Cancelled,
}

impl HireStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "REQUESTED",
            Self::Confirmed => "CONFIRMED",
            Self::Released => "RELEASED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "CONFIRMED" => Self::Confirmed,
            "RELEASED" => Self::Released,
            "CANCELLED" => Self::Cancelled,
            _ => Self::Requested,
        }
    }

    /// `Requested` or `Confirmed`: the hire still ties up a vendor.
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Requested | Self::Confirmed)
    }
}

/// One vendor truck booked for one dispatch (`dispatch_id`) or one
/// multi-stop trip (`trip_id`). Exactly one of the two is set.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VehicleHire {
    pub id: Uuid,
    pub org_id: Uuid,
    pub vendor_id: Uuid,
    /// The vendor's name at read time, for display. Empty if the vendor row
    /// is gone.
    #[serde(default)]
    pub vendor_name: String,
    pub dispatch_id: Option<Uuid>,
    pub trip_id: Option<Uuid>,
    /// Summed volume of everything the hire must carry, fixed when the hire
    /// was requested. The assigned truck's capacity must be at least this.
    pub required_volume: i64,
    pub status: HireStatus,
    // Filled in by `assign`:
    pub registration_number: Option<String>,
    pub capacity: Option<i64>,
    pub unit: Option<Unit>,
    pub driver_name: Option<String>,
    pub driver_phone: Option<String>,
    pub driver_license: Option<String>,
    /// Agreed hire cost for the whole trip, whole currency units.
    pub freight_amount: Option<i64>,
    /// Paid to the vendor up front (usually at loading).
    pub advance_paid: i64,
    pub requested_at: i64,
    pub confirmed_at: Option<i64>,
    pub closed_at: Option<i64>,
}

/// What the dispatcher enters after the vendor confirms a truck.
#[derive(Debug, Clone)]
pub struct HireAssignment {
    pub registration_number: String,
    pub capacity: i64,
    pub unit: Unit,
    pub driver_name: String,
    pub driver_phone: String,
    pub driver_license: Option<String>,
    pub freight_amount: i64,
    pub advance_paid: i64,
}

#[derive(Debug)]
pub enum HireError {
    /// A missing or out-of-range field, or a truck too small for the load.
    InvalidInput(String),
    /// The hire is not `REQUESTED` (already assigned, or closed).
    Conflict(String),
    Db(Box<dyn Error>),
}

impl fmt::Display for HireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HireError::InvalidInput(m) | HireError::Conflict(m) => write!(f, "{m}"),
            HireError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for HireError {}

impl From<mysql::Error> for HireError {
    fn from(e: mysql::Error) -> Self {
        HireError::Db(Box::new(e))
    }
}

impl From<Box<dyn Error>> for HireError {
    fn from(e: Box<dyn Error>) -> Self {
        HireError::Db(e)
    }
}

const SELECT_HIRE: &str = "SELECT h.id, h.org_id, h.vendor_id, COALESCE(v.name, ''), h.dispatch_id, h.trip_id,
        h.required_volume, h.status, h.registration_number, h.capacity, h.unit,
        h.driver_name, h.driver_phone, h.driver_license, h.freight_amount, h.advance_paid,
        h.requested_at, h.confirmed_at, h.closed_at
     FROM VehicleHires h LEFT JOIN VehicleVendors v ON v.id = h.vendor_id";

fn parse_uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or_else(|_| Uuid::new_v4())
}

impl VehicleHire {
    /// Kept in sync with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        crate::logistics::vendor::vendor::VehicleVendor::ensure_table(conn)?;
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS VehicleHires (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                vendor_id VARCHAR(36) NOT NULL,
                dispatch_id VARCHAR(36) DEFAULT NULL,
                trip_id VARCHAR(36) DEFAULT NULL,
                required_volume BIGINT NOT NULL,
                status VARCHAR(20) NOT NULL,
                registration_number VARCHAR(255) DEFAULT NULL,
                capacity BIGINT DEFAULT NULL,
                unit VARCHAR(50) DEFAULT NULL,
                driver_name VARCHAR(255) DEFAULT NULL,
                driver_phone VARCHAR(64) DEFAULT NULL,
                driver_license VARCHAR(255) DEFAULT NULL,
                freight_amount BIGINT DEFAULT NULL,
                advance_paid BIGINT NOT NULL DEFAULT 0,
                requested_at BIGINT NOT NULL,
                confirmed_at BIGINT DEFAULT NULL,
                closed_at BIGINT DEFAULT NULL,
                CONSTRAINT fk_hire_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    /// Record a `REQUESTED` hire. Called by the org's dispatch / trip
    /// creation with the id it already stamped onto the dispatches.
    pub(crate) fn insert_requested(
        conn: &mut mysql::PooledConn,
        id: Uuid,
        org_id: Uuid,
        vendor_id: Uuid,
        dispatch_id: Option<Uuid>,
        trip_id: Option<Uuid>,
        required_volume: i64,
    ) -> Result<(), Box<dyn Error>> {
        Self::ensure_table(conn)?;
        conn.exec_drop(
            "INSERT INTO VehicleHires (id, org_id, vendor_id, dispatch_id, trip_id, required_volume, status, requested_at)
             VALUES (:id, :org_id, :vendor_id, :dispatch_id, :trip_id, :required_volume, 'REQUESTED', :requested_at)",
            params! {
                "id" => id.to_string(),
                "org_id" => org_id.to_string(),
                "vendor_id" => vendor_id.to_string(),
                "dispatch_id" => dispatch_id.map(|d| d.to_string()),
                "trip_id" => trip_id.map(|t| t.to_string()),
                "required_volume" => required_volume,
                "requested_at" => now_unix(),
            },
        )?;
        Ok(())
    }

    fn from_row(row: mysql::Row) -> Self {
        let get_s = |r: &mysql::Row, i: usize| r.get::<Option<String>, _>(i).flatten();
        let get_i = |r: &mysql::Row, i: usize| r.get::<Option<i64>, _>(i).flatten();
        VehicleHire {
            id: parse_uuid(&get_s(&row, 0).unwrap_or_default()),
            org_id: parse_uuid(&get_s(&row, 1).unwrap_or_default()),
            vendor_id: parse_uuid(&get_s(&row, 2).unwrap_or_default()),
            vendor_name: get_s(&row, 3).unwrap_or_default(),
            dispatch_id: get_s(&row, 4).and_then(|s| Uuid::parse_str(&s).ok()),
            trip_id: get_s(&row, 5).and_then(|s| Uuid::parse_str(&s).ok()),
            required_volume: get_i(&row, 6).unwrap_or(0),
            status: HireStatus::from_db(&get_s(&row, 7).unwrap_or_default()),
            registration_number: get_s(&row, 8),
            capacity: get_i(&row, 9),
            unit: get_s(&row, 10).map(|u| Unit::from_str(&u)),
            driver_name: get_s(&row, 11),
            driver_phone: get_s(&row, 12),
            driver_license: get_s(&row, 13),
            freight_amount: get_i(&row, 14),
            advance_paid: get_i(&row, 15).unwrap_or(0),
            requested_at: get_i(&row, 16).unwrap_or(0),
            confirmed_at: get_i(&row, 17),
            closed_at: get_i(&row, 18),
        }
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let row: Option<mysql::Row> = conn.exec_first(
            format!("{SELECT_HIRE} WHERE h.id = :id"),
            params! { "id" => id.to_string() },
        )?;
        Ok(row.map(Self::from_row))
    }

    /// The org's hires, newest first, optionally only those in `status`.
    pub fn list_by_org(org_id: Uuid, status: Option<HireStatus>) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let rows: Vec<mysql::Row> = conn.exec(
            format!(
                "{SELECT_HIRE} WHERE h.org_id = :org_id
                   AND (:status IS NULL OR h.status = :status)
                 ORDER BY h.requested_at DESC, h.id"
            ),
            params! {
                "org_id" => org_id.to_string(),
                "status" => status.map(|s| s.as_str()),
            },
        )?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    /// Whether any hire, open or closed, was ever made with `vendor_id`.
    /// A vendor with hire history can't be deleted (it would orphan the
    /// history); deactivate it instead.
    pub fn exists_for_vendor(vendor_id: Uuid) -> Result<bool, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let hit: Option<i64> = conn.exec_first(
            "SELECT 1 FROM VehicleHires WHERE vendor_id = :vendor_id LIMIT 1",
            params! { "vendor_id" => vendor_id.to_string() },
        )?;
        Ok(hit.is_some())
    }

    /// Record the vendor's truck, driver and agreed rate, and release the
    /// dispatches waiting on it.
    ///
    /// Validates the input (non-blank truck/driver fields, `freight_amount >
    /// 0`, `0 <= advance_paid <= freight_amount`, `capacity >=
    /// required_volume`) and that the hire is still `REQUESTED`. Then, in one
    /// transaction: the hire becomes `CONFIRMED`; every `AWAITING_VEHICLE`
    /// dispatch on it gets the truck number and moves to `PENDING` with a
    /// status-history entry; a hired trip gets the truck number too.
    ///
    /// Returns the dispatches that were released, reloaded, so the caller can
    /// notify their customers and the hired driver.
    pub fn assign(&mut self, input: HireAssignment) -> Result<Vec<DispatchOrder>, HireError> {
        let reg = input.registration_number.trim().to_string();
        let driver_name = input.driver_name.trim().to_string();
        let driver_phone = input.driver_phone.trim().to_string();
        let driver_license = input
            .driver_license
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());
        if reg.is_empty() {
            return Err(HireError::InvalidInput("the hired truck's registration number is required".into()));
        }
        if driver_name.is_empty() || driver_phone.is_empty() {
            return Err(HireError::InvalidInput("the hired driver's name and phone are required".into()));
        }
        if input.freight_amount <= 0 {
            return Err(HireError::InvalidInput("freight amount must be greater than zero".into()));
        }
        if input.advance_paid < 0 || input.advance_paid > input.freight_amount {
            return Err(HireError::InvalidInput(
                "advance paid must be between zero and the freight amount".into(),
            ));
        }
        if input.capacity < self.required_volume {
            return Err(HireError::InvalidInput(format!(
                "truck {reg} is too small: capacity {} but the load needs {}",
                input.capacity, self.required_volume
            )));
        }
        if self.status != HireStatus::Requested {
            return Err(HireError::Conflict(format!(
                "this hire is already {} and can't be assigned again",
                self.status.as_str()
            )));
        }

        let mut conn = DbConnection::from_env().get_connection()?;
        crate::logistics::dispatch::dispatch::ensure_tables(&mut conn)?;
        crate::logistics::dispatch::trip::Trip::ensure_table(&mut conn)?;
        let now = now_unix();

        let released_ids: Vec<String> = {
            let mut tx = conn.start_transaction(TxOpts::default())?;
            // Re-check under the transaction so two dispatchers can't both
            // assign the same hire.
            tx.exec_drop(
                "UPDATE VehicleHires
                 SET status = 'CONFIRMED', registration_number = :reg, capacity = :capacity,
                     unit = :unit, driver_name = :driver_name, driver_phone = :driver_phone,
                     driver_license = :driver_license, freight_amount = :freight_amount,
                     advance_paid = :advance_paid, confirmed_at = :now
                 WHERE id = :id AND status = 'REQUESTED'",
                params! {
                    "reg" => &reg,
                    "capacity" => input.capacity,
                    "unit" => input.unit.as_str(),
                    "driver_name" => &driver_name,
                    "driver_phone" => &driver_phone,
                    "driver_license" => &driver_license,
                    "freight_amount" => input.freight_amount,
                    "advance_paid" => input.advance_paid,
                    "now" => now,
                    "id" => self.id.to_string(),
                },
            )?;
            if tx.affected_rows() == 0 {
                tx.rollback()?;
                return Err(HireError::Conflict("this hire was assigned or closed meanwhile".into()));
            }

            let ids: Vec<String> = tx.exec(
                "SELECT id FROM Dispatches WHERE hire_id = :hire_id AND status = 'AWAITING_VEHICLE'",
                params! { "hire_id" => self.id.to_string() },
            )?;
            tx.exec_drop(
                "UPDATE Dispatches SET vehicle_registration_number = :reg, status = 'PENDING'
                 WHERE hire_id = :hire_id AND status = 'AWAITING_VEHICLE'",
                params! { "reg" => &reg, "hire_id" => self.id.to_string() },
            )?;
            for id in &ids {
                tx.exec_drop(
                    "INSERT INTO DispatchStatusHistory (dispatch_id, status, changed_at)
                     VALUES (:dispatch_id, 'PENDING', :changed_at)",
                    params! { "dispatch_id" => id, "changed_at" => now },
                )?;
            }
            if let Some(trip_id) = self.trip_id {
                tx.exec_drop(
                    "UPDATE Trips SET vehicle_registration_number = :reg WHERE id = :id",
                    params! { "reg" => &reg, "id" => trip_id.to_string() },
                )?;
            }
            tx.commit()?;
            ids
        };

        self.status = HireStatus::Confirmed;
        self.registration_number = Some(reg);
        self.capacity = Some(input.capacity);
        self.unit = Some(input.unit);
        self.driver_name = Some(driver_name);
        self.driver_phone = Some(driver_phone);
        self.driver_license = driver_license;
        self.freight_amount = Some(input.freight_amount);
        self.advance_paid = input.advance_paid;
        self.confirmed_at = Some(now);

        let mut released = Vec::with_capacity(released_ids.len());
        for id in released_ids {
            if let Some(order) = DispatchOrder::get_by_id(parse_uuid(&id))? {
                crate::logistics::ai::chunk::reindex_dispatch_best_effort(&order);
                released.push(order);
            }
        }
        released.sort_by_key(|d| d.stop_sequence);
        Ok(released)
    }

    /// Called after a dispatch on `hire_id` reaches a terminal status. Once
    /// every dispatch on the hire is terminal, close the hire: `RELEASED` if a
    /// truck had been assigned, `CANCELLED` if it never was. Does nothing
    /// while any dispatch on the hire is still running, or if the hire is
    /// already closed.
    pub(crate) fn close_if_finished(
        conn: &mut mysql::PooledConn,
        hire_id: Uuid,
    ) -> Result<(), Box<dyn Error>> {
        Self::ensure_table(conn)?;
        let running: Option<i64> = conn.exec_first(
            "SELECT 1 FROM Dispatches
             WHERE hire_id = :hire_id AND status NOT IN ('DELIVERED', 'RETURNED', 'CANCELLED')
             LIMIT 1",
            params! { "hire_id" => hire_id.to_string() },
        )?;
        if running.is_some() {
            return Ok(());
        }
        conn.exec_drop(
            "UPDATE VehicleHires
             SET status = IF(status = 'REQUESTED', 'CANCELLED', 'RELEASED'), closed_at = :now
             WHERE id = :id AND status IN ('REQUESTED', 'CONFIRMED')",
            params! { "now" => now_unix(), "id" => hire_id.to_string() },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::customer::customer::Customer;
    use crate::logistics::dispatch::dispatch::{
        DispatchLineItemInput, DispatchStatus, ProofOfDeliveryInput, VehicleSource,
    };
    use crate::logistics::dispatch::trip::Trip;
    use crate::logistics::driver::driver::Driver;
    use crate::logistics::godown::godown::Godown;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::stock::stock::Stock;
    use crate::logistics::test_support::TestDb;
    use crate::logistics::vehicle::vehicle::Vehicle;
    use crate::logistics::vendor::vendor::{VehicleVendor, VendorInput};

    fn line(description: &str, quantity: i64) -> DispatchLineItemInput {
        DispatchLineItemInput {
            stock_description: description.to_string(),
            requested_quantity: quantity,
        }
    }

    /// An org with **no vehicles**, one godown holding 100 Cement (volume 2
    /// each), a located customer, and an active vendor.
    fn fleetless_org() -> (Organization, Customer, VehicleVendor) {
        let org = Organization::create_organization("No Fleet Co", "Pune").expect("org");
        let godown = Godown::create(org.id, "Main", "MIDC", None).expect("godown");
        Stock::new(2, 100, "Cement").add_to_godown(godown.id).expect("stock");
        let mut customer = Customer::create_customer(org.id, "Buyer", "Baner").expect("customer");
        customer.update_location(18.56, 73.78, Some("Baner")).expect("location");
        let vendor = VehicleVendor::create(
            org.id,
            VendorInput { name: "Sharma Roadlines".into(), phone: "+91 1".into(), ..Default::default() },
        )
        .expect("vendor");
        (org, customer, vendor)
    }

    fn cement_left(org: &Organization) -> i64 {
        Godown::list_by_org(org.id)
            .expect("godowns")
            .iter()
            .flat_map(|g| g.stock.iter())
            .filter(|s| s.description == "Cement")
            .map(|s| s.quantity)
            .sum()
    }

    fn truck(reg: &str, capacity: i64) -> HireAssignment {
        HireAssignment {
            registration_number: reg.into(),
            capacity,
            unit: Unit::MetricTon,
            driver_name: "Hired Driver".into(),
            driver_phone: "+91 98000 00000".into(),
            driver_license: None,
            freight_amount: 12_000,
            advance_paid: 10_000,
        }
    }

    #[test]
    fn test_hired_dispatch_reserves_stock_and_awaits_a_vehicle() {
        let _db = TestDb::create();
        let (org, customer, vendor) = fleetless_org();

        // No own vehicles, so an own-fleet dispatch fails and points at hiring.
        let err = org
            .dispatch_stock_to_customer(&customer, &[line("Cement", 10)])
            .expect_err("no fleet");
        assert!(err.to_string().contains("hire a truck"), "{err}");

        let order = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 10)], vendor.id)
            .expect("hired dispatch");
        assert_eq!(order.status, DispatchStatus::AwaitingVehicle);
        assert_eq!(order.vehicle_source, VehicleSource::Hired);
        assert_eq!(order.vehicle_registration_number, None);
        assert_eq!(cement_left(&org), 90, "stock is reserved straight away");

        let hire = VehicleHire::get_by_id(order.hire_id.expect("hire id")).unwrap().unwrap();
        assert_eq!(hire.status, HireStatus::Requested);
        assert_eq!(hire.dispatch_id, Some(order.id));
        assert_eq!(hire.vendor_name, "Sharma Roadlines");
        assert_eq!(hire.required_volume, 20, "10 units x volume 2");

        let reloaded = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
        assert_eq!(reloaded.status, DispatchStatus::AwaitingVehicle);
        assert_eq!(reloaded.hire_id, order.hire_id);
    }

    #[test]
    fn test_hire_rejects_inactive_or_foreign_vendor_without_touching_stock() {
        let _db = TestDb::create();
        let (org, customer, mut vendor) = fleetless_org();
        let other = Organization::create_organization("Other Org", "x").expect("org");
        let foreign = VehicleVendor::create(
            other.id,
            VendorInput { name: "Theirs".into(), phone: "1".into(), ..Default::default() },
        )
        .expect("foreign vendor");

        let err = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 5)], foreign.id)
            .expect_err("foreign vendor");
        assert!(err.to_string().contains("not found"), "{err}");

        vendor
            .update(VendorInput { name: vendor.name.clone(), phone: vendor.phone.clone(), ..Default::default() }, false)
            .expect("deactivate");
        let err = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 5)], vendor.id)
            .expect_err("inactive vendor");
        assert!(err.to_string().contains("inactive"), "{err}");

        assert_eq!(cement_left(&org), 100);
        assert!(VehicleHire::list_by_org(org.id, None).unwrap().is_empty());
    }

    #[test]
    fn test_assign_validates_then_moves_dispatch_to_pending() {
        let _db = TestDb::create();
        let (org, customer, vendor) = fleetless_org();
        let order = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 10)], vendor.id)
            .expect("hired dispatch");
        let mut hire = VehicleHire::get_by_id(order.hire_id.unwrap()).unwrap().unwrap();

        let too_small = hire.assign(truck("MH12 HR 0001", 19)).expect_err("capacity 19 < 20");
        assert!(matches!(too_small, HireError::InvalidInput(ref m) if m.contains("too small")), "{too_small}");
        let bad_advance = hire
            .assign(HireAssignment { advance_paid: 20_000, ..truck("MH12 HR 0001", 20) })
            .expect_err("advance above freight");
        assert!(matches!(bad_advance, HireError::InvalidInput(_)));
        let no_driver = hire
            .assign(HireAssignment { driver_phone: " ".into(), ..truck("MH12 HR 0001", 20) })
            .expect_err("blank driver phone");
        assert!(matches!(no_driver, HireError::InvalidInput(_)));

        let released = hire.assign(truck("MH12 HR 0001", 20)).expect("assign");
        assert_eq!(released.len(), 1);
        assert_eq!(hire.status, HireStatus::Confirmed);

        let d = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
        assert_eq!(d.status, DispatchStatus::Pending);
        assert_eq!(d.vehicle_registration_number.as_deref(), Some("MH12 HR 0001"));
        let history: Vec<DispatchStatus> = d.status_history.iter().map(|e| e.status).collect();
        assert_eq!(history, [DispatchStatus::AwaitingVehicle, DispatchStatus::Pending]);

        let stored = VehicleHire::get_by_id(hire.id).unwrap().unwrap();
        assert_eq!(stored.freight_amount, Some(12_000));
        assert_eq!(stored.advance_paid, 10_000);
        assert_eq!(stored.driver_name.as_deref(), Some("Hired Driver"));

        let again = hire.assign(truck("MH12 HR 0002", 50)).expect_err("already assigned");
        assert!(matches!(again, HireError::Conflict(_)));
    }

    #[test]
    fn test_awaiting_dispatch_cannot_skip_to_pending_through_a_status_change() {
        let _db = TestDb::create();
        let (org, customer, vendor) = fleetless_org();
        let mut order = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 1)], vendor.id)
            .expect("hired dispatch");
        assert!(order.transition_to(DispatchStatus::Pending, None, None).is_err());
        assert!(order.transition_to(DispatchStatus::Confirmed, None, None).is_err());
    }

    #[test]
    fn test_cancelling_an_awaiting_dispatch_returns_stock_and_cancels_hire() {
        let _db = TestDb::create();
        let (org, customer, vendor) = fleetless_org();
        let mut order = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 10)], vendor.id)
            .expect("hired dispatch");
        assert_eq!(cement_left(&org), 90);

        order.transition_to(DispatchStatus::Cancelled, None, None).expect("cancel");
        assert_eq!(cement_left(&org), 100, "reserved stock goes back");

        let hire = VehicleHire::get_by_id(order.hire_id.unwrap()).unwrap().unwrap();
        assert_eq!(hire.status, HireStatus::Cancelled);
        assert!(hire.closed_at.is_some());
    }

    #[test]
    fn test_delivering_a_hired_dispatch_releases_the_hire() {
        let _db = TestDb::create();
        let (org, customer, vendor) = fleetless_org();
        let order = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 10)], vendor.id)
            .expect("hired dispatch");
        let mut hire = VehicleHire::get_by_id(order.hire_id.unwrap()).unwrap().unwrap();
        hire.assign(truck("MH12 HR 0003", 40)).expect("assign");

        let mut d = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
        for s in [DispatchStatus::Confirmed, DispatchStatus::Loaded, DispatchStatus::InTransit] {
            d.transition_to(s, None, None).expect("advance");
            assert_eq!(VehicleHire::get_by_id(hire.id).unwrap().unwrap().status, HireStatus::Confirmed);
        }
        d.transition_to(
            DispatchStatus::Delivered,
            Some(ProofOfDeliveryInput { receiver_name: "R".into(), signature_or_photo_url: "u".into() }),
            None,
        )
        .expect("deliver");
        assert_eq!(VehicleHire::get_by_id(hire.id).unwrap().unwrap().status, HireStatus::Released);
    }

    #[test]
    fn test_hired_dispatch_does_not_block_an_own_vehicle_with_the_same_number() {
        let _db = TestDb::create();
        let (mut org, customer, vendor) = fleetless_org();
        org.update_location(18.52, 73.85, Some("Pune")).expect("org location");

        // An active hired dispatch on truck "MH12 SAME 1"…
        let order = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 1)], vendor.id)
            .expect("hired dispatch");
        VehicleHire::get_by_id(order.hire_id.unwrap())
            .unwrap()
            .unwrap()
            .assign(truck("MH12 SAME 1", 10))
            .expect("assign");

        // …doesn't make an own vehicle with that number look busy.
        let mut v = Vehicle::new("MH12 SAME 1", 1_000, Unit::MetricTon);
        v.add_new_vehicle_to_org(&org).expect("vehicle");
        let driver = Driver::create(org.id, "Own Driver", "L", "1").expect("driver");
        v.assign_driver(Some(driver.id)).expect("assign driver");
        let own = org
            .dispatch_stock_to_customer(&customer, &[line("Cement", 1)])
            .expect("own vehicle is free");
        assert_eq!(own.vehicle_source, VehicleSource::Own);
    }

    #[test]
    fn test_hired_trip_assigns_every_stop_and_releases_after_the_last() {
        let _db = TestDb::create();
        let (org, first, vendor) = fleetless_org();
        let mut second = Customer::create_customer(org.id, "Second", "Aundh").expect("customer");
        second.update_location(18.55, 73.80, Some("Aundh")).expect("location");

        let (lines_a, lines_b) = ([line("Cement", 3)], [line("Cement", 4)]);
        let stops: Vec<(&Customer, &[DispatchLineItemInput])> =
            vec![(&first, &lines_a[..]), (&second, &lines_b[..])];
        let trip = org
            .dispatch_trip_on_hired_vehicle(&stops, false, vendor.id)
            .expect("hired trip");
        assert_eq!(trip.vehicle_source, VehicleSource::Hired);
        assert_eq!(trip.vehicle_registration_number, None);
        assert!(trip.stops.iter().all(|s| s.status == DispatchStatus::AwaitingVehicle));

        let mut hire = VehicleHire::get_by_id(trip.hire_id.unwrap()).unwrap().unwrap();
        assert_eq!(hire.trip_id, Some(trip.id));
        assert_eq!(hire.required_volume, 14, "7 units x volume 2");
        let released = hire.assign(truck("MH12 TRIP 1", 14)).expect("assign");
        assert_eq!(released.len(), 2);

        let trip = Trip::get_by_id(trip.id).unwrap().unwrap();
        assert_eq!(trip.vehicle_registration_number.as_deref(), Some("MH12 TRIP 1"));
        assert!(trip.stops.iter().all(|s| s.status == DispatchStatus::Pending));

        // Cancelling one stop leaves the hire open; cancelling the last closes it.
        let mut stops = trip.stops;
        stops[0].transition_to(DispatchStatus::Cancelled, None, None).expect("cancel 1");
        assert_eq!(VehicleHire::get_by_id(hire.id).unwrap().unwrap().status, HireStatus::Confirmed);
        stops[1].transition_to(DispatchStatus::Cancelled, None, None).expect("cancel 2");
        assert_eq!(VehicleHire::get_by_id(hire.id).unwrap().unwrap().status, HireStatus::Released);
    }

    #[test]
    fn test_list_filters_by_status_and_vendor_history_is_detected() {
        let _db = TestDb::create();
        let (org, customer, vendor) = fleetless_org();
        assert!(!VehicleHire::exists_for_vendor(vendor.id).unwrap());

        let a = org
            .dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 1)], vendor.id)
            .expect("a");
        org.dispatch_stock_on_hired_vehicle(&customer, &[line("Cement", 1)], vendor.id)
            .expect("b");
        VehicleHire::get_by_id(a.hire_id.unwrap())
            .unwrap()
            .unwrap()
            .assign(truck("MH12 LIST 1", 5))
            .expect("assign a");

        assert_eq!(VehicleHire::list_by_org(org.id, None).unwrap().len(), 2);
        let requested = VehicleHire::list_by_org(org.id, Some(HireStatus::Requested)).unwrap();
        assert_eq!(requested.len(), 1);
        assert_eq!(requested[0].status, HireStatus::Requested);
        assert!(VehicleHire::exists_for_vendor(vendor.id).unwrap());
    }
}
