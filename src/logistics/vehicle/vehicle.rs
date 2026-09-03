use crate::logistics::db::connection::DbConnection;
use crate::logistics::orgs::orgs::Organization;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A unit of measure for cargo — used for a vehicle's rated capacity and,
/// loosely, for talking about how much stock a shipment moves. `MetricTon`
/// is the historical default and the fallback [`Unit::from_str`] returns for
/// anything it doesn't recognize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum Unit {
    MetricTon,
    Kg,
    Litre,
    Box,
    Pallet,
    Piece,
}

impl Unit {
    pub fn as_str(&self) -> &'static str {
        match self {
            Unit::MetricTon => "MetricTon",
            Unit::Kg => "Kg",
            Unit::Litre => "Litre",
            Unit::Box => "Box",
            Unit::Pallet => "Pallet",
            Unit::Piece => "Piece",
        }
    }

    /// Parse a stored/incoming unit string. Accepts the canonical names
    /// ([`Unit::as_str`]) case-insensitively; unknown input falls back to
    /// `MetricTon` so a bad value never fails a request outright.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "kg" | "kilogram" | "kilograms" => Unit::Kg,
            "litre" | "liter" | "l" => Unit::Litre,
            "box" | "boxes" => Unit::Box,
            "pallet" | "pallets" => Unit::Pallet,
            "piece" | "pieces" | "pcs" => Unit::Piece,
            "metricton" | "metric_ton" | "ton" | "tonne" | "mt" => Unit::MetricTon,
            _ => Unit::MetricTon,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub timestamp: i64,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Vehicle {
    pub registration_number: String,
    pub capacity: i64,
    pub unit: Unit,
    pub location: Option<Location>,
    /// The driver currently assigned to this vehicle, if any. A vehicle
    /// can only be selected for a dispatch when this is set and the driver
    /// is active. Managed through `PUT /api/vehicles/{reg}/driver`.
    #[serde(default)]
    pub assigned_driver_id: Option<Uuid>,
}

impl Vehicle {
    pub fn new(registration_number: impl Into<String>, capacity: i64, unit: Unit) -> Self {
        Self {
            registration_number: registration_number.into(),
            capacity,
            unit,
            location: None,
            assigned_driver_id: None,
        }
    }

    /// Assign a driver to this vehicle, or clear the assignment with `None`.
    /// The caller is responsible for checking the driver belongs to the same
    /// org and (if it matters) is active.
    pub fn assign_driver(&mut self, driver_id: Option<Uuid>) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "UPDATE Vehicle SET assigned_driver_id = :driver_id WHERE registration_number = :registration_number",
            params! {
                "registration_number" => &self.registration_number,
                "driver_id" => driver_id.map(|d| d.to_string()),
            },
        )?;

        self.assigned_driver_id = driver_id;
        Ok(())
    }

    pub fn add_new_vehicle_to_org(&self, org: &Organization) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        // Ensure Vehicle table exists with location columns
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

        let (lat, lng, ts, addr) = match &self.location {
            Some(loc) => (
                Some(loc.latitude),
                Some(loc.longitude),
                Some(loc.timestamp),
                loc.address.clone(),
            ),
            None => (None, None, None, None),
        };

        // Insert vehicle record into MySQL database
        conn.exec_drop(
            "INSERT INTO Vehicle (registration_number, capacity, unit, org_id, latitude, longitude, last_updated_at, location_address) 
             VALUES (:registration_number, :capacity, :unit, :org_id, :latitude, :longitude, :last_updated_at, :location_address)
             ON DUPLICATE KEY UPDATE capacity = VALUES(capacity), unit = VALUES(unit), org_id = VALUES(org_id), latitude = VALUES(latitude), longitude = VALUES(longitude), last_updated_at = VALUES(last_updated_at), location_address = VALUES(location_address)",
            params! {
                "registration_number" => &self.registration_number,
                "capacity" => self.capacity,
                "unit" => self.unit.as_str(),
                "org_id" => org.id.to_string(),
                "latitude" => lat,
                "longitude" => lng,
                "last_updated_at" => ts,
                "location_address" => addr,
            },
        )?;

        Ok(())
    }

    pub fn update_vehicle(&mut self, capacity: i64, unit: Unit) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "UPDATE Vehicle SET capacity = :capacity, unit = :unit WHERE registration_number = :registration_number",
            params! {
                "registration_number" => &self.registration_number,
                "capacity" => capacity,
                "unit" => unit.as_str(),
            },
        )?;

        self.capacity = capacity;
        self.unit = unit;
        Ok(())
    }

    /// The id of the organization that owns the vehicle with this
    /// registration number, or `None` if there is no such vehicle.
    pub fn org_of(reg: &str) -> Result<Option<Uuid>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        let row: Option<String> = conn.exec_first(
            "SELECT org_id FROM Vehicle WHERE registration_number = :reg",
            params! { "reg" => reg },
        )?;

        Ok(row.and_then(|s| Uuid::parse_str(&s).ok()))
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
            "UPDATE Vehicle SET latitude = :latitude, longitude = :longitude, last_updated_at = :last_updated_at, location_address = :location_address WHERE registration_number = :registration_number",
            params! {
                "registration_number" => &self.registration_number,
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

    pub fn list_all() -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

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

        let rows: Vec<(String, i64, String, Option<String>, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn.exec_map(
            "SELECT registration_number, capacity, unit, assigned_driver_id, latitude, longitude, last_updated_at, location_address FROM Vehicle",
            (),
            |(reg, cap, unit_str, driver, lat, lng, ts, addr)| (reg, cap, unit_str, driver, lat, lng, ts, addr),
        )?;

        let vehicles = rows
            .into_iter()
            .map(|(reg, cap, unit_str, driver, lat, lng, ts, addr)| {
                let location = lat.map(|latitude| Location {
                    latitude,
                    longitude: lng.unwrap_or(0.0),
                    timestamp: ts.unwrap_or(0),
                    address: addr,
                });
                Vehicle {
                    registration_number: reg,
                    capacity: cap,
                    unit: Unit::from_str(&unit_str),
                    location,
                    assigned_driver_id: driver.and_then(|d| Uuid::parse_str(&d).ok()),
                }
            })
            .collect();

        Ok(vehicles)
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        let rows: Vec<(String, i64, String, Option<String>, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn.exec_map(
            "SELECT registration_number, capacity, unit, assigned_driver_id, latitude, longitude, last_updated_at, location_address FROM Vehicle WHERE org_id = :org_id",
            params! { "org_id" => org_id.to_string() },
            |(reg, cap, unit_str, driver, lat, lng, ts, addr)| (reg, cap, unit_str, driver, lat, lng, ts, addr),
        )?;

        Ok(rows
            .into_iter()
            .map(|(reg, cap, unit_str, driver, lat, lng, ts, addr)| {
                let location = lat.map(|latitude| Location {
                    latitude,
                    longitude: lng.unwrap_or(0.0),
                    timestamp: ts.unwrap_or(0),
                    address: addr,
                });
                Vehicle {
                    registration_number: reg,
                    capacity: cap,
                    unit: Unit::from_str(&unit_str),
                    location,
                    assigned_driver_id: driver.and_then(|d| Uuid::parse_str(&d).ok()),
                }
            })
            .collect())
    }

    pub fn remove_vehicle(&self) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "DELETE FROM Vehicle WHERE registration_number = :registration_number",
            params! {
                "registration_number" => &self.registration_number,
            },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::test_support::TestDb;

    #[test]
    fn test_add_new_vehicle_to_org() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Fleet Org", "Highway 101, Logistics Park")
            .expect("Failed to create organization for vehicle test");

        let vehicle = Vehicle::new("MH12 EN 3502", 50, Unit::MetricTon);
        let res = vehicle.add_new_vehicle_to_org(&org);
        assert!(res.is_ok(), "Failed to add vehicle to organization in database");

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for vehicle verification");

        let row: Option<(String, i64, String, String)> = conn
            .exec_first(
                "SELECT registration_number, capacity, unit, org_id FROM Vehicle WHERE registration_number = :reg",
                params! {
                    "reg" => &vehicle.registration_number,
                },
            )
            .expect("Failed to query database for vehicle");

        assert!(row.is_some(), "Vehicle record not found in database");
        let (db_reg, db_capacity, db_unit, db_org_id) = row.unwrap();
        assert_eq!(db_reg, vehicle.registration_number);
        assert_eq!(db_capacity, vehicle.capacity);
        assert_eq!(db_unit, vehicle.unit.as_str());
        assert_eq!(db_org_id, org.id.to_string());
    }

    #[test]
    fn test_update_vehicle() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Update Vehicle Org", "Sector 4, Industrial Area")
            .expect("Failed to create organization for update vehicle test");

        let mut vehicle = Vehicle::new("DL01 AB 1234", 30, Unit::MetricTon);
        vehicle.add_new_vehicle_to_org(&org).expect("Failed to add vehicle to org");

        let update_res = vehicle.update_vehicle(75, Unit::MetricTon);
        assert!(update_res.is_ok(), "Failed to update vehicle");
        assert_eq!(vehicle.capacity, 75);

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for vehicle update verification");

        let row: Option<(i64, String)> = conn
            .exec_first(
                "SELECT capacity, unit FROM Vehicle WHERE registration_number = :reg",
                params! {
                    "reg" => &vehicle.registration_number,
                },
            )
            .expect("Failed to query updated vehicle from database");

        assert!(row.is_some(), "Updated vehicle record not found in database");
        let (db_capacity, db_unit) = row.unwrap();
        assert_eq!(db_capacity, 75);
        assert_eq!(db_unit, "MetricTon");
    }

    #[test]
    fn test_update_vehicle_location() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Location Tracking Org", "Sector 62, Noida")
            .expect("Failed to create organization for location test");

        let mut vehicle = Vehicle::new("UP16 AB 5555", 60, Unit::MetricTon);
        vehicle.add_new_vehicle_to_org(&org).expect("Failed to add vehicle to org");

        let lat = 28.6139;
        let lng = 77.2090;
        let addr = "Connaught Place, New Delhi";

        let loc_res = vehicle.update_location(lat, lng, Some(addr));
        assert!(loc_res.is_ok(), "Failed to update vehicle location");

        assert!(vehicle.location.is_some());
        let loc = vehicle.get_location().unwrap();
        assert_eq!(loc.latitude, lat);
        assert_eq!(loc.longitude, lng);
        assert_eq!(loc.address.as_deref(), Some(addr));

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for location verification");

        let row: Option<(Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn
            .exec_first(
                "SELECT latitude, longitude, last_updated_at, location_address FROM Vehicle WHERE registration_number = :reg",
                params! {
                    "reg" => &vehicle.registration_number,
                },
            )
            .expect("Failed to query location from database");

        assert!(row.is_some(), "Vehicle location record not found in database");
        let (db_lat, db_lng, db_ts, db_addr) = row.unwrap();
        assert_eq!(db_lat, Some(lat));
        assert_eq!(db_lng, Some(lng));
        assert!(db_ts.is_some() && db_ts.unwrap() > 0);
        assert_eq!(db_addr.as_deref(), Some(addr));
    }

    #[test]
    fn test_remove_vehicle() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Remove Vehicle Org", "Sector 9, Transport Hub")
            .expect("Failed to create organization for remove vehicle test");

        let vehicle = Vehicle::new("HR26 CQ 9999", 40, Unit::MetricTon);
        vehicle.add_new_vehicle_to_org(&org).expect("Failed to add vehicle to org");

        let remove_res = vehicle.remove_vehicle();
        assert!(remove_res.is_ok(), "Failed to remove vehicle");

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for vehicle removal verification");

        let row: Option<(String, i64, String, String)> = conn
            .exec_first(
                "SELECT registration_number, capacity, unit, org_id FROM Vehicle WHERE registration_number = :reg",
                params! {
                    "reg" => &vehicle.registration_number,
                },
            )
            .expect("Failed to query database for removed vehicle");

        assert!(row.is_none(), "Vehicle record should be deleted from database");
    }

    #[test]
    fn test_unit_as_str_round_trips_through_from_str() {
        for unit in [
            Unit::MetricTon,
            Unit::Kg,
            Unit::Litre,
            Unit::Box,
            Unit::Pallet,
            Unit::Piece,
        ] {
            assert_eq!(Unit::from_str(unit.as_str()), unit);
        }
    }

    #[test]
    fn test_unit_from_str_is_case_insensitive_with_aliases() {
        assert_eq!(Unit::from_str("KG"), Unit::Kg);
        assert_eq!(Unit::from_str(" liter "), Unit::Litre);
        assert_eq!(Unit::from_str("Pallets"), Unit::Pallet);
        assert_eq!(Unit::from_str("pcs"), Unit::Piece);
        assert_eq!(Unit::from_str("tonne"), Unit::MetricTon);
    }

    #[test]
    fn test_unit_from_str_falls_back_to_metric_ton() {
        assert_eq!(Unit::from_str("furlongs"), Unit::MetricTon);
        assert_eq!(Unit::from_str(""), Unit::MetricTon);
    }

    #[test]
    fn test_non_default_unit_persists_and_reloads() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Litre Fleet Org", "Tank Farm Rd")
            .expect("create org");

        let vehicle = Vehicle::new("GJ01 TT 4040", 12000, Unit::Litre);
        vehicle.add_new_vehicle_to_org(&org).expect("add vehicle");

        let reloaded = Organization::get_by_id(org.id)
            .expect("get org")
            .expect("org exists");
        let v = reloaded
            .vehicles
            .iter()
            .find(|v| v.registration_number == "GJ01 TT 4040")
            .expect("vehicle present on org");
        assert_eq!(v.unit, Unit::Litre);
        assert_eq!(v.capacity, 12000);
    }
}
