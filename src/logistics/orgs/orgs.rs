use crate::logistics::customer::customer::Customer;
use crate::logistics::db::connection::DbConnection;
use crate::logistics::dispatch::dispatch::{DispatchOrder, DispatchStatus};
use crate::logistics::driver::driver::Driver;
use crate::logistics::godown::godown::Godown;
use crate::logistics::stock::stock::Stock;
use crate::logistics::vehicle::vehicle::{Location, Unit, Vehicle};
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn haversine_distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    #[allow(dead_code)]
    pub vehicles: Vec<Vehicle>,
    /// Warehouses owned by this organization, each carrying its own stock.
    pub godowns: Vec<Godown>,
    pub location: Option<Location>,
}

impl Organization {
    /// Create every table an organization read/write may touch, if it does not
    /// already exist. Each entity module also creates its own table lazily on
    /// first write; this guards the read paths that join across all of them.
    fn ensure_tables(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Orgs (
                id VARCHAR(36) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                address VARCHAR(255) NOT NULL,
                latitude DOUBLE DEFAULT NULL,
                longitude DOUBLE DEFAULT NULL,
                last_updated_at BIGINT DEFAULT NULL,
                location_address VARCHAR(255) DEFAULT NULL
            )",
            (),
        )?;
        Driver::ensure_table(conn)?;
        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Vehicle (
                registration_number VARCHAR(255) PRIMARY KEY,
                capacity BIGINT NOT NULL,
                unit VARCHAR(50) NOT NULL,
                org_id VARCHAR(36) NOT NULL,
                assigned_driver_id VARCHAR(36) DEFAULT NULL,
                latitude DOUBLE DEFAULT NULL,
                longitude DOUBLE DEFAULT NULL,
                last_updated_at BIGINT DEFAULT NULL,
                location_address VARCHAR(255) DEFAULT NULL,
                CONSTRAINT fk_vehicle_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
            (),
        )?;
        Godown::ensure_table(conn)?;
        Stock::ensure_table(conn)?;
        Ok(())
    }

    pub fn create_organization(
        name: impl Into<String>,
        address: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        let name_str = name.into();
        let address_str = address.into();
        let org = Organization {
            id: Uuid::new_v4(),
            name: name_str,
            address: address_str,
            vehicles: Vec::new(),
            godowns: Vec::new(),
            location: None,
        };

        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Orgs (
                id VARCHAR(36) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                address VARCHAR(255) NOT NULL,
                latitude DOUBLE DEFAULT NULL,
                longitude DOUBLE DEFAULT NULL,
                last_updated_at BIGINT DEFAULT NULL,
                location_address VARCHAR(255) DEFAULT NULL
            )",
            (),
        )?;

        // Insert organization into MySQL database
        conn.exec_drop(
            "INSERT INTO Orgs (id, name, address) VALUES (:id, :name, :address)",
            params! {
                "id" => org.id.to_string(),
                "name" => &org.name,
                "address" => &org.address,
            },
        )?;

        Ok(org)
    }

    pub fn update_organization(
        &mut self,
        name: impl Into<String>,
        address: impl Into<String>,
    ) -> Result<(), Box<dyn Error>> {
        let new_name = name.into();
        let new_address = address.into();

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "UPDATE Orgs SET name = :name, address = :address WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "name" => &new_name,
                "address" => &new_address,
            },
        )?;

        self.name = new_name;
        self.address = new_address;
        Ok(())
    }

    pub fn update_location(
        &mut self,
        latitude: f64,
        longitude: f64,
        address: Option<impl Into<String>>,
    ) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let address_str = address.map(|a| a.into());

        conn.exec_drop(
            "UPDATE Orgs SET latitude = :latitude, longitude = :longitude, last_updated_at = :last_updated_at, location_address = :location_address WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "latitude" => latitude,
                "longitude" => longitude,
                "last_updated_at" => now,
                "location_address" => &address_str,
            },
        )?;

        self.location = Some(Location {
            latitude,
            longitude,
            timestamp: now,
            address: address_str,
        });

        Ok(())
    }

    pub fn get_location(&self) -> Option<&Location> {
        self.location.as_ref()
    }

    pub fn dispatch_stock_to_customer(
        &self,
        customer: &Customer,
        stock_description: &str,
        requested_quantity: i64,
    ) -> Result<DispatchOrder, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        // 1. Verify stock availability across all of the org's godowns. A stock
        //    item can be split across several godowns; the requested quantity is
        //    checked against — and later drawn from — the combined holding.
        let godowns = Godown::list_by_org(self.id)?;
        let mut holdings: Vec<(Uuid, i64)> = Vec::new();
        let mut stock_volume_in_size: Option<i64> = None;
        for g in &godowns {
            if let Some(s) = g.stock.iter().find(|s| s.description == stock_description) {
                holdings.push((g.id, s.quantity));
                stock_volume_in_size.get_or_insert(s.volume_in_size);
            }
        }

        if holdings.is_empty() {
            return Err("Requested stock description not found in any of the organization's godowns".into());
        }

        let total_available: i64 = holdings.iter().map(|(_, qty)| qty).sum();
        if total_available < requested_quantity {
            return Err(format!(
                "Insufficient stock quantity. Available: {}, Requested: {}",
                total_available, requested_quantity
            )
            .into());
        }

        // Total volume this shipment occupies — checked against a vehicle's
        // rated `capacity` below.
        let required_volume = stock_volume_in_size
            .unwrap_or(0)
            .saturating_mul(requested_quantity);

        // 2. Fetch this org's vehicles that can actually take this trip:
        //    - an *active* driver is assigned (Vehicle.assigned_driver_id ->
        //      an is_active Driver row),
        //    - the vehicle's rated `capacity` covers `required_volume`, and
        //    - the vehicle is not already on an active (non-terminal) trip,
        //      so the same truck can't be double-booked onto two orders.
        Driver::ensure_table(&mut conn)?;
        crate::logistics::dispatch::dispatch::ensure_tables(&mut conn)?;
        let vehicle_rows: Vec<(String, i64, String, Option<f64>, Option<f64>)> = conn.exec_map(
            "SELECT v.registration_number, v.capacity, v.unit, v.latitude, v.longitude
             FROM Vehicle v
             JOIN Drivers d ON d.id = v.assigned_driver_id AND d.is_active = TRUE
             WHERE v.org_id = :org_id
               AND v.capacity >= :required_volume
               AND NOT EXISTS (
                   SELECT 1 FROM Dispatches disp
                   WHERE disp.vehicle_registration_number = v.registration_number
                     AND disp.status NOT IN ('DELIVERED', 'RETURNED', 'CANCELLED')
               )",
            params! {
                "org_id" => self.id.to_string(),
                "required_volume" => required_volume,
            },
            |(reg, cap, unit, lat, lng)| (reg, cap, unit, lat, lng),
        )?;

        if vehicle_rows.is_empty() {
            let any_vehicle: Option<i64> = conn.exec_first(
                "SELECT 1 FROM Vehicle WHERE org_id = :org_id LIMIT 1",
                params! { "org_id" => self.id.to_string() },
            )?;
            let with_active_driver: Option<i64> = conn.exec_first(
                "SELECT 1 FROM Vehicle v
                 JOIN Drivers d ON d.id = v.assigned_driver_id AND d.is_active = TRUE
                 WHERE v.org_id = :org_id LIMIT 1",
                params! { "org_id" => self.id.to_string() },
            )?;
            return Err(if any_vehicle.is_none() {
                "No vehicles registered under this organization for dispatch".into()
            } else if with_active_driver.is_none() {
                "No vehicle with an active assigned driver is available for dispatch; \
                 assign one via PUT /api/vehicles/{reg}/driver"
                    .into()
            } else {
                format!(
                    "No vehicle is free and large enough for this shipment \
                     (needs capacity >= {required_volume}); every eligible vehicle is \
                     either below capacity or already on an active trip"
                )
                .into()
            });
        }

        // Customer target coordinates
        let (cust_lat, cust_lng) = match &customer.location {
            Some(loc) => (loc.latitude, loc.longitude),
            None => return Err("Customer location is not set for dispatch".into()),
        };

        // Fallback organization coordinates if vehicle location is unset
        let (org_lat, org_lng) = match &self.location {
            Some(loc) => (loc.latitude, loc.longitude),
            None => (0.0, 0.0),
        };

        // 3. Find nearest vehicle based on Haversine distance
        let mut nearest_vehicle_reg: Option<String> = None;
        let mut min_distance = f64::MAX;

        for (reg, _cap, _unit, v_lat_opt, v_lng_opt) in vehicle_rows {
            let v_lat = v_lat_opt.unwrap_or(org_lat);
            let v_lng = v_lng_opt.unwrap_or(org_lng);

            let dist = haversine_distance_km(v_lat, v_lng, cust_lat, cust_lng);
            if dist < min_distance {
                min_distance = dist;
                nearest_vehicle_reg = Some(reg);
            }
        }

        let selected_vehicle_reg =
            nearest_vehicle_reg.ok_or("Failed to select vehicle for dispatch")?;

        // 4. Draw the requested quantity down from the godowns, largest holding
        //    first, until the request is satisfied.
        holdings.sort_by(|a, b| b.1.cmp(&a.1));
        let mut remaining = requested_quantity;
        for (godown_id, available) in holdings {
            if remaining <= 0 {
                break;
            }
            let taken = remaining.min(available);
            conn.exec_drop(
                "UPDATE Stock SET quantity = :quantity WHERE godown_id = :godown_id AND description = :desc",
                params! {
                    "quantity" => available - taken,
                    "godown_id" => godown_id.to_string(),
                    "desc" => stock_description,
                },
            )?;
            remaining -= taken;
        }

        // 5. Create and save DispatchOrder
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        // Stock is reserved and a vehicle is already picked at this point,
        // but nothing has physically moved — the dispatch starts its
        // lifecycle at PENDING and advances via DispatchOrder::transition_to
        // (PUT /api/dispatches/{id}/status). See DispatchStatus's docs for
        // the full state machine.
        let mut dispatch_order = DispatchOrder {
            id: Uuid::new_v4(),
            org_id: self.id,
            customer_id: customer.id,
            vehicle_registration_number: selected_vehicle_reg,
            stock_description: stock_description.to_string(),
            quantity: requested_quantity,
            status: DispatchStatus::Pending,
            dispatched_at: now,
            status_history: Vec::new(),
            proof_of_delivery: None,
        };

        dispatch_order.save()?;

        Ok(dispatch_order)
    }

    pub fn list_all() -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Orgs (
                id VARCHAR(36) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                address VARCHAR(255) NOT NULL,
                latitude DOUBLE DEFAULT NULL,
                longitude DOUBLE DEFAULT NULL,
                last_updated_at BIGINT DEFAULT NULL,
                location_address VARCHAR(255) DEFAULT NULL
            )",
            (),
        )?;

        let rows: Vec<(String, String, String, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn.exec_map(
            "SELECT id, name, address, latitude, longitude, last_updated_at, location_address FROM Orgs",
            (),
            |(id, name, address, lat, lng, ts, addr)| (id, name, address, lat, lng, ts, addr),
        )?;

        let orgs = rows
            .into_iter()
            .map(|(id, name, address, lat, lng, ts, addr)| {
                let location = lat.map(|latitude| Location {
                    latitude,
                    longitude: lng.unwrap_or(0.0),
                    timestamp: ts.unwrap_or(0),
                    address: addr,
                });
                Organization {
                    id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                    name,
                    address,
                    vehicles: Vec::new(),
                    godowns: Vec::new(),
                    location,
                }
            })
            .collect();

        Ok(orgs)
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        // Ensure the tables this query touches exist. On a brand-new database the
        // Vehicle / Stock tables are only created lazily when the first vehicle or
        // stock item is added, so a freshly-registered org with neither would make
        // the SELECTs below fail with "table doesn't exist" and surface as a 500.
        Self::ensure_tables(&mut conn)?;

        let row: Option<(String, String, String, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn
            .exec_first(
                "SELECT id, name, address, latitude, longitude, last_updated_at, location_address FROM Orgs WHERE id = :id",
                params! { "id" => id.to_string() },
            )?;

        let (org_id_str, name, address, lat, lng, ts, addr) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let location = lat.map(|latitude| Location {
            latitude,
            longitude: lng.unwrap_or(0.0),
            timestamp: ts.unwrap_or(0),
            address: addr,
        });

        let vehicles: Vec<Vehicle> = conn
            .exec_map(
                "SELECT registration_number, capacity, unit, assigned_driver_id, latitude, longitude, last_updated_at, location_address FROM Vehicle WHERE org_id = :org_id",
                params! { "org_id" => &org_id_str },
                |(reg, cap, unit_str, driver, v_lat, v_lng, v_ts, v_addr): (String, i64, String, Option<String>, Option<f64>, Option<f64>, Option<i64>, Option<String>)| {
                    let v_location = v_lat.map(|latitude| Location {
                        latitude,
                        longitude: v_lng.unwrap_or(0.0),
                        timestamp: v_ts.unwrap_or(0),
                        address: v_addr,
                    });
                    Vehicle {
                        registration_number: reg,
                        capacity: cap,
                        unit: Unit::from_str(&unit_str),
                        location: v_location,
                        assigned_driver_id: driver.and_then(|d| Uuid::parse_str(&d).ok()),
                    }
                },
            )?;

        let godowns = Godown::list_by_org(id)?;

        Ok(Some(Organization {
            id: Uuid::parse_str(&org_id_str).unwrap_or(id),
            name,
            address,
            vehicles,
            godowns,
            location,
        }))
    }

    pub fn remove_organization(&self) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "DELETE FROM Orgs WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
            },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::test_support::TestDb;
    use crate::logistics::vehicle::vehicle::Unit;

    #[test]
    fn test_create_organization() {
        let _db = TestDb::create();
        let test_name = "Flow Logic";
        let test_address = "Shop 60, Market No.1, N.I.T Faridabad, New Delhi";

        let org = Organization::create_organization(test_name, test_address)
            .expect("Failed to create organization in database");

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for verification");

        let row: Option<(String, String, String)> = conn
            .exec_first(
                "SELECT id, name, address FROM Orgs WHERE id = :id",
                params! {
                    "id" => org.id.to_string(),
                },
            )
            .expect("Failed to query database for organization");

        assert!(row.is_some(), "Organization record not found in database");
        let (db_id, db_name, db_address) = row.unwrap();
        assert_eq!(db_id, org.id.to_string());
        assert_eq!(db_name, test_name);
        assert_eq!(db_address, test_address);
    }

    #[test]
    fn test_update_organization() {
        let _db = TestDb::create();
        let org_res = Organization::create_organization("Initial Org Name", "Initial Address");
        assert!(org_res.is_ok(), "Failed to create organization");

        let mut org = org_res.unwrap();
        let update_res = org.update_organization("Updated Org Name", "Updated Address Location");
        assert!(update_res.is_ok(), "Failed to update organization");

        assert_eq!(org.name, "Updated Org Name");
        assert_eq!(org.address, "Updated Address Location");

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for verification");

        let row: Option<(String, String, String)> = conn
            .exec_first(
                "SELECT id, name, address FROM Orgs WHERE id = :id",
                params! {
                    "id" => org.id.to_string(),
                },
            )
            .expect("Failed to query database for updated organization");

        assert!(row.is_some(), "Updated organization record not found in database");
        let (db_id, db_name, db_address) = row.unwrap();
        assert_eq!(db_id, org.id.to_string());
        assert_eq!(db_name, "Updated Org Name");
        assert_eq!(db_address, "Updated Address Location");
    }

    #[test]
    fn test_update_organization_location() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Org Location Test", "Central HQ, Cyber City")
            .expect("Failed to create organization for location test");

        let lat = 28.4595;
        let lng = 77.0266;
        let addr = "DLF Cyber City, Gurugram";

        let update_res = org.update_location(lat, lng, Some(addr));
        assert!(update_res.is_ok(), "Failed to update organization location");

        assert!(org.location.is_some());
        let loc = org.get_location().unwrap();
        assert_eq!(loc.latitude, lat);
        assert_eq!(loc.longitude, lng);
        assert_eq!(loc.address.as_deref(), Some(addr));

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for org location verification");

        let row: Option<(Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn
            .exec_first(
                "SELECT latitude, longitude, last_updated_at, location_address FROM Orgs WHERE id = :id",
                params! {
                    "id" => org.id.to_string(),
                },
            )
            .expect("Failed to query org location from database");

        assert!(row.is_some(), "Organization location record not found in database");
        let (db_lat, db_lng, db_ts, db_addr) = row.unwrap();
        assert_eq!(db_lat, Some(lat));
        assert_eq!(db_lng, Some(lng));
        assert!(db_ts.is_some() && db_ts.unwrap() > 0);
        assert_eq!(db_addr.as_deref(), Some(addr));
    }

    #[test]
    fn test_dispatch_stock_to_customer_nearest_vehicle() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Global Logistics", "Delhi HQ")
            .expect("Failed to create organization");
        org.update_location(28.6139, 77.2090, Some("Delhi HQ")).expect("Failed to update org location");

        // Add a godown and stock it
        let godown = Godown::create(org.id, "Delhi Godown", "Okhla Phase 1", None)
            .expect("Failed to create godown");
        let stock = Stock::new(50, 100, "High-End Laptops");
        stock.add_to_godown(godown.id).expect("Failed to add stock");

        // Each vehicle needs an active assigned driver to be dispatch-eligible.
        let d1 = Driver::create(org.id, "Driver One", "LIC-1", "111").expect("driver 1");
        let d2 = Driver::create(org.id, "Driver Two", "LIC-2", "222").expect("driver 2");

        // Add Vehicle 1 (Far away - Mumbai: 19.0760, 72.8777)
        let mut v1 = Vehicle::new("MH01 AX 1111", 10_000, Unit::MetricTon);
        v1.add_new_vehicle_to_org(&org).expect("Failed to add v1");
        v1.update_location(19.0760, 72.8777, Some("Mumbai Port")).expect("Failed to update v1 location");
        v1.assign_driver(Some(d1.id)).expect("assign d1");

        // Add Vehicle 2 (Near Customer - Noida: 28.5355, 77.3910)
        let mut v2 = Vehicle::new("UP16 BZ 2222", 10_000, Unit::MetricTon);
        v2.add_new_vehicle_to_org(&org).expect("Failed to add v2");
        v2.update_location(28.5355, 77.3910, Some("Noida Hub")).expect("Failed to update v2 location");
        v2.assign_driver(Some(d2.id)).expect("assign d2");

        // Create Customer (Located in Delhi / NCR: 28.6200, 77.2100)
        let mut customer = Customer::create_customer("Tech Store India", "Connaught Place, New Delhi")
            .expect("Failed to create customer");
        customer.update_location(28.6200, 77.2100, Some("Connaught Place")).expect("Failed to update customer location");

        // Dispatch 15 Laptops to Customer
        let dispatch_res = org.dispatch_stock_to_customer(&customer, "High-End Laptops", 15);
        assert!(dispatch_res.is_ok(), "Failed to dispatch stock to customer");

        let dispatch_order = dispatch_res.unwrap();
        // Vehicle 2 (UP16 BZ 2222) in Noida is closest to Delhi customer vs Vehicle 1 in Mumbai
        assert_eq!(dispatch_order.vehicle_registration_number, "UP16 BZ 2222");
        assert_eq!(dispatch_order.quantity, 15);
        assert_eq!(dispatch_order.status, DispatchStatus::Pending);
        assert_eq!(dispatch_order.status_history.len(), 1);
        assert_eq!(
            dispatch_order.status_history[0].status,
            DispatchStatus::Pending
        );

        // Verify stock quantity decremented in MySQL database (100 - 15 = 85)
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for stock verification");

        let stock_qty: Option<i64> = conn
            .exec_first(
                "SELECT quantity FROM Stock WHERE godown_id = :godown_id AND description = :desc",
                params! {
                    "godown_id" => godown.id.to_string(),
                    "desc" => "High-End Laptops",
                },
            )
            .expect("Failed to query stock from DB");

        assert_eq!(stock_qty, Some(85));
    }

    #[test]
    fn test_dispatch_requires_a_vehicle_with_an_active_driver() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Driverless Logistics", "Pune HQ")
            .expect("create org");
        org.update_location(18.5204, 73.8567, Some("Pune HQ")).expect("org location");

        let godown = Godown::create(org.id, "Pune Godown", "Hinjewadi", None).expect("godown");
        Stock::new(50, 100, "Ceramic Tiles").add_to_godown(godown.id).expect("stock");

        let mut vehicle = Vehicle::new("MH12 ZZ 9999", 10_000, Unit::MetricTon);
        vehicle.add_new_vehicle_to_org(&org).expect("add vehicle");
        vehicle.update_location(18.53, 73.85, Some("Pune")).expect("vehicle location");

        let mut customer = Customer::create_customer("Tile Mart", "FC Road, Pune").expect("customer");
        customer.update_location(18.52, 73.84, Some("FC Road")).expect("customer location");

        // No driver assigned yet -> rejected.
        let err = org
            .dispatch_stock_to_customer(&customer, "Ceramic Tiles", 10)
            .expect_err("dispatch should fail without an active assigned driver");
        assert!(
            err.to_string().contains("active assigned driver"),
            "unexpected error: {err}"
        );

        // Assign an *inactive* driver -> still rejected.
        let mut driver = Driver::create(org.id, "On Leave", "LIC-X", "000").expect("driver");
        driver.update("On Leave", "LIC-X", "000", false).expect("deactivate");
        vehicle.assign_driver(Some(driver.id)).expect("assign");
        assert!(
            org.dispatch_stock_to_customer(&customer, "Ceramic Tiles", 10)
                .is_err(),
            "dispatch should fail with an inactive driver"
        );

        // Reactivate the driver -> dispatch succeeds.
        driver.update("Back To Work", "LIC-X", "000", true).expect("reactivate");
        let order = org
            .dispatch_stock_to_customer(&customer, "Ceramic Tiles", 10)
            .expect("dispatch should succeed once an active driver is assigned");
        assert_eq!(order.vehicle_registration_number, "MH12 ZZ 9999");
    }

    #[test]
    fn test_dispatch_checks_vehicle_capacity() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Capacity Logistics", "Nagpur HQ")
            .expect("org");
        org.update_location(21.1458, 79.0882, Some("Nagpur")).expect("org loc");

        let godown = Godown::create(org.id, "Nagpur Godown", "MIDC", None).expect("godown");
        // 20 units at volume 10 each -> a 5-unit shipment needs volume 50.
        Stock::new(10, 20, "Marble Slabs").add_to_godown(godown.id).expect("stock");

        let mut customer = Customer::create_customer("Stone Co", "Civil Lines").expect("customer");
        customer.update_location(21.15, 79.09, Some("Civil Lines")).expect("cust loc");

        let driver = Driver::create(org.id, "Cap Driver", "LIC-C", "000").expect("driver");

        // Too-small vehicle (capacity 40 < 50) -> rejected.
        let mut small = Vehicle::new("MH31 SM 0001", 40, Unit::MetricTon);
        small.add_new_vehicle_to_org(&org).expect("add small");
        small.update_location(21.15, 79.08, Some("Nagpur")).expect("loc");
        small.assign_driver(Some(driver.id)).expect("assign");

        let err = org
            .dispatch_stock_to_customer(&customer, "Marble Slabs", 5)
            .expect_err("should reject: vehicle too small");
        assert!(err.to_string().contains("capacity >= 50"), "unexpected: {err}");

        // Big-enough vehicle -> succeeds and is the one chosen.
        let driver2 = Driver::create(org.id, "Big Driver", "LIC-B", "000").expect("driver2");
        let mut big = Vehicle::new("MH31 BG 0002", 500, Unit::MetricTon);
        big.add_new_vehicle_to_org(&org).expect("add big");
        big.update_location(21.15, 79.08, Some("Nagpur")).expect("loc");
        big.assign_driver(Some(driver2.id)).expect("assign");

        let order = org
            .dispatch_stock_to_customer(&customer, "Marble Slabs", 5)
            .expect("should succeed with a big-enough vehicle");
        assert_eq!(order.vehicle_registration_number, "MH31 BG 0002");
    }

    #[test]
    fn test_dispatch_does_not_double_book_a_vehicle() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Busy Fleet", "Surat HQ").expect("org");
        org.update_location(21.1702, 72.8311, Some("Surat")).expect("org loc");

        let godown = Godown::create(org.id, "Surat Godown", "Sachin GIDC", None).expect("godown");
        Stock::new(1, 100, "Fabric Rolls").add_to_godown(godown.id).expect("stock");

        let mut customer = Customer::create_customer("Textile Buyer", "Ring Road").expect("customer");
        customer.update_location(21.18, 72.83, Some("Ring Road")).expect("cust loc");

        // Two eligible vehicles, both near the customer.
        let d1 = Driver::create(org.id, "D1", "L1", "1").expect("d1");
        let d2 = Driver::create(org.id, "D2", "L2", "2").expect("d2");
        let mut v1 = Vehicle::new("GJ05 AA 0001", 1_000, Unit::MetricTon);
        v1.add_new_vehicle_to_org(&org).expect("v1");
        v1.update_location(21.18, 72.83, Some("Surat")).expect("v1 loc");
        v1.assign_driver(Some(d1.id)).expect("assign d1");
        let mut v2 = Vehicle::new("GJ05 BB 0002", 1_000, Unit::MetricTon);
        v2.add_new_vehicle_to_org(&org).expect("v2");
        v2.update_location(21.18, 72.83, Some("Surat")).expect("v2 loc");
        v2.assign_driver(Some(d2.id)).expect("assign d2");

        let first = org
            .dispatch_stock_to_customer(&customer, "Fabric Rolls", 10)
            .expect("first dispatch");
        let second = org
            .dispatch_stock_to_customer(&customer, "Fabric Rolls", 10)
            .expect("second dispatch uses the other vehicle");
        assert_ne!(
            first.vehicle_registration_number, second.vehicle_registration_number,
            "the same vehicle must not be booked onto two active trips"
        );

        // Both vehicles are now on active trips -> a third dispatch fails.
        let err = org
            .dispatch_stock_to_customer(&customer, "Fabric Rolls", 10)
            .expect_err("third dispatch: no vehicle free");
        assert!(err.to_string().contains("active trip"), "unexpected: {err}");

        // Completing the first trip frees its vehicle for reuse.
        let mut first = DispatchOrder::get_by_id(first.id).expect("get").expect("exists");
        first.transition_to(DispatchStatus::Confirmed, None).expect("confirm");
        first.transition_to(DispatchStatus::Loaded, None).expect("load");
        first.transition_to(DispatchStatus::InTransit, None).expect("in transit");
        first
            .transition_to(
                DispatchStatus::Delivered,
                Some(crate::logistics::dispatch::dispatch::ProofOfDeliveryInput {
                    receiver_name: "Buyer".to_string(),
                    signature_or_photo_url: "https://example.test/sig.png".to_string(),
                }),
            )
            .expect("deliver");

        let fourth = org
            .dispatch_stock_to_customer(&customer, "Fabric Rolls", 10)
            .expect("a freed vehicle can be dispatched again");
        assert_eq!(
            fourth.vehicle_registration_number, first.vehicle_registration_number,
            "the delivered trip's vehicle should be available again"
        );
    }

    #[test]
    fn test_remove_organization() {
        let _db = TestDb::create();
        let org_res = Organization::create_organization("Org To Delete", "Delete Address");
        assert!(org_res.is_ok(), "Failed to create organization for removal test");

        let org = org_res.unwrap();
        let remove_res = org.remove_organization();
        assert!(remove_res.is_ok(), "Failed to remove organization");

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for verification");

        let row: Option<(String, String, String)> = conn
            .exec_first(
                "SELECT id, name, address FROM Orgs WHERE id = :id",
                params! {
                    "id" => org.id.to_string(),
                },
            )
            .expect("Failed to query database for deleted organization");

        assert!(row.is_none(), "Organization record should be deleted from database");
    }
}
