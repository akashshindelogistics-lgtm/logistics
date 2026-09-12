use crate::logistics::customer::customer::Customer;
use crate::logistics::db::connection::DbConnection;
use crate::logistics::dispatch::dispatch::{
    DispatchLineItem, DispatchLineItemInput, DispatchOrder, DispatchStatus,
};
use crate::logistics::dispatch::trip::Trip;
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

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn haversine_distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

/// A validated dispatch line item plus the godown holdings it will be drawn
/// from (largest first). Built up-front for every requested line so nothing is
/// drawn down until the whole order is known to be satisfiable.
struct LineItemPlan {
    description: String,
    quantity: i64,
    volume_in_size: i64,
    /// `(godown_id, quantity_to_draw_from_it)` — the exact per-godown draw
    /// this line needs, largest source first.
    holdings: Vec<(Uuid, i64)>,
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
                tracker_key VARCHAR(36) DEFAULT NULL,
                CONSTRAINT fk_vehicle_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
            (),
        )?;
        crate::logistics::vehicle::vehicle::ensure_tracker_key_column(conn)?;
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
        line_items: &[DispatchLineItemInput],
    ) -> Result<DispatchOrder, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        let godowns = Godown::list_by_org(self.id)?;

        let mut remaining = Self::stock_snapshot(&godowns);
        let (plans, required_volume) =
            self.plan_stock_draw(&godowns, &mut remaining, line_items)?;
        let vehicle_reg = self.select_free_vehicle(&mut conn, required_volume, customer)?;
        let order_line_items = Self::draw_down_plans(&mut conn, plans)?;

        let mut dispatch_order = DispatchOrder {
            id: Uuid::new_v4(),
            org_id: self.id,
            customer_id: customer.id,
            vehicle_registration_number: vehicle_reg,
            line_items: order_line_items,
            status: DispatchStatus::Pending,
            dispatched_at: now_secs(),
            status_history: Vec::new(),
            proof_of_delivery: None,
            trip_id: None,
            stop_sequence: None,
        };
        dispatch_order.save()?;
        Ok(dispatch_order)
    }

    /// Send one vehicle on a **multi-stop trip**: several customers' orders,
    /// each drawn from the org's stock, all carried on the same truck in the
    /// given sequence. All-or-nothing — every stop is validated (shape, stock,
    /// customer location) and the combined shipment is fit to a single free
    /// vehicle before any stock is drawn down.
    ///
    /// Each stop becomes a normal `DispatchOrder` (its own PENDING → …
    /// lifecycle, its own invoice, its own proof of delivery), linked by
    /// `trip_id` and ordered by `stop_sequence`.
    pub fn dispatch_trip_to_customers(
        &self,
        stops: &[(&Customer, &[DispatchLineItemInput])],
    ) -> Result<Trip, Box<dyn Error>> {
        if stops.len() < 2 {
            return Err(
                "A trip needs at least two stops; use a single dispatch for one customer".into(),
            );
        }
        {
            let mut seen = std::collections::HashSet::new();
            for (c, _) in stops {
                if !seen.insert(c.id) {
                    return Err("The same customer appears twice in the trip".into());
                }
            }
        }

        let mut conn = DbConnection::from_env().get_connection()?;
        let godowns = Godown::list_by_org(self.id)?;

        // Plan every stop against a shared, decrementing stock snapshot so two
        // stops can't over-commit the same item, and sum the volume the truck
        // must carry for the whole trip.
        let mut remaining = Self::stock_snapshot(&godowns);
        let mut stop_plans: Vec<(&Customer, Vec<LineItemPlan>)> = Vec::with_capacity(stops.len());
        let mut trip_volume: i64 = 0;
        for (idx, (customer, line_items)) in stops.iter().enumerate() {
            if customer.location.is_none() {
                return Err(format!(
                    "Stop {}: customer '{}' has no delivery location set",
                    idx + 1,
                    customer.name
                )
                .into());
            }
            let (plans, volume) = self
                .plan_stock_draw(&godowns, &mut remaining, line_items)
                .map_err(|e| -> Box<dyn Error> { format!("Stop {}: {e}", idx + 1).into() })?;
            trip_volume = trip_volume.saturating_add(volume);
            stop_plans.push((customer, plans));
        }

        // One vehicle for the whole trip — nearest to the first stop.
        let vehicle_reg = self.select_free_vehicle(&mut conn, trip_volume, stops[0].0)?;

        let now = now_secs();
        let trip_id = Uuid::new_v4();
        conn.exec_drop(
            "INSERT INTO Trips (id, org_id, vehicle_registration_number, created_at)
             VALUES (:id, :org_id, :veh, :created_at)",
            params! {
                "id" => trip_id.to_string(),
                "org_id" => self.id.to_string(),
                "veh" => &vehicle_reg,
                "created_at" => now,
            },
        )?;

        let mut trip_stops = Vec::with_capacity(stop_plans.len());
        for (i, (customer, plans)) in stop_plans.into_iter().enumerate() {
            let order_line_items = Self::draw_down_plans(&mut conn, plans)?;
            let mut order = DispatchOrder {
                id: Uuid::new_v4(),
                org_id: self.id,
                customer_id: customer.id,
                vehicle_registration_number: vehicle_reg.clone(),
                line_items: order_line_items,
                status: DispatchStatus::Pending,
                dispatched_at: now,
                status_history: Vec::new(),
                proof_of_delivery: None,
                trip_id: Some(trip_id),
                stop_sequence: Some(i as i64 + 1),
            };
            order.save()?;
            trip_stops.push(order);
        }

        let status = Trip::compute_status(&trip_stops);
        Ok(Trip {
            id: trip_id,
            org_id: self.id,
            vehicle_registration_number: vehicle_reg,
            created_at: now,
            status,
            stops: trip_stops,
        })
    }

    // ── shared dispatch helpers ─────────────────────────────────────────────

    /// Every godown's holding of every stock item, keyed by
    /// `(godown_id, description)` → quantity on hand. Planning a draw
    /// decrements this in place so a multi-stop trip can't over-commit stock.
    fn stock_snapshot(godowns: &[Godown]) -> std::collections::HashMap<(Uuid, String), i64> {
        let mut m = std::collections::HashMap::new();
        for g in godowns {
            for s in &g.stock {
                m.insert((g.id, s.description.clone()), s.quantity);
            }
        }
        m
    }

    /// Validate one customer's requested line items against `remaining` (a
    /// mutable stock snapshot). On success `remaining` is decremented by the
    /// planned draw and the per-line plan plus the total volume it needs are
    /// returned; on failure `remaining` is left untouched.
    fn plan_stock_draw(
        &self,
        godowns: &[Godown],
        remaining: &mut std::collections::HashMap<(Uuid, String), i64>,
        line_items: &[DispatchLineItemInput],
    ) -> Result<(Vec<LineItemPlan>, i64), Box<dyn Error>> {
        if line_items.is_empty() {
            return Err("A dispatch must carry at least one stock line item".into());
        }
        if line_items.iter().any(|li| li.requested_quantity <= 0) {
            return Err("Every line item's requested quantity must be greater than zero".into());
        }
        let mut seen = std::collections::HashSet::new();
        for li in line_items {
            if !seen.insert(li.stock_description.as_str()) {
                return Err(format!(
                    "Line item '{}' appears more than once; combine it into a single line",
                    li.stock_description
                )
                .into());
            }
        }

        // Scratch copy so a mid-loop failure leaves `remaining` untouched.
        let mut scratch = remaining.clone();
        let mut plans: Vec<LineItemPlan> = Vec::new();
        let mut required_volume: i64 = 0;

        for li in line_items {
            let mut volume_in_size: Option<i64> = None;
            let mut holdings: Vec<(Uuid, i64)> = Vec::new();
            for g in godowns {
                if let Some(s) = g.stock.iter().find(|s| s.description == li.stock_description) {
                    volume_in_size.get_or_insert(s.volume_in_size);
                    let have = scratch
                        .get(&(g.id, li.stock_description.clone()))
                        .copied()
                        .unwrap_or(0);
                    if have > 0 {
                        holdings.push((g.id, have));
                    }
                }
            }

            let Some(volume_in_size) = volume_in_size else {
                return Err(format!(
                    "Stock '{}' was not found in any of the organization's godowns",
                    li.stock_description
                )
                .into());
            };

            let total_available: i64 = holdings.iter().map(|(_, q)| q).sum();
            if total_available < li.requested_quantity {
                return Err(format!(
                    "Insufficient stock for '{}'. Available: {}, Requested: {}",
                    li.stock_description, total_available, li.requested_quantity
                )
                .into());
            }

            required_volume = required_volume
                .saturating_add(volume_in_size.saturating_mul(li.requested_quantity));

            // Largest holding first, so the drawdown touches the fewest rows.
            holdings.sort_by_key(|&(_, q)| std::cmp::Reverse(q));
            let mut take_from: Vec<(Uuid, i64)> = Vec::new();
            let mut want = li.requested_quantity;
            for (gid, have) in &holdings {
                if want <= 0 {
                    break;
                }
                let take = want.min(*have);
                take_from.push((*gid, take));
                if let Some(v) = scratch.get_mut(&(*gid, li.stock_description.clone())) {
                    *v -= take;
                }
                want -= take;
            }

            plans.push(LineItemPlan {
                description: li.stock_description.clone(),
                quantity: li.requested_quantity,
                volume_in_size,
                holdings: take_from,
            });
        }

        *remaining = scratch;
        Ok((plans, required_volume))
    }

    /// Apply a set of planned draws to `Stock` and return the resulting
    /// [`DispatchLineItem`]s (with `volume_in_size` snapshotted).
    fn draw_down_plans(
        conn: &mut mysql::PooledConn,
        plans: Vec<LineItemPlan>,
    ) -> Result<Vec<DispatchLineItem>, Box<dyn Error>> {
        let mut items = Vec::with_capacity(plans.len());
        for plan in plans {
            for (godown_id, take) in &plan.holdings {
                conn.exec_drop(
                    "UPDATE Stock SET quantity = quantity - :take
                     WHERE godown_id = :godown_id AND description = :desc",
                    params! {
                        "take" => take,
                        "godown_id" => godown_id.to_string(),
                        "desc" => &plan.description,
                    },
                )?;
                crate::logistics::ai::chunk::reindex_stock_by_description_best_effort(
                    *godown_id,
                    &plan.description,
                );
            }
            items.push(DispatchLineItem {
                stock_description: plan.description,
                quantity: plan.quantity,
                volume_in_size: plan.volume_in_size,
            });
        }
        Ok(items)
    }

    /// Pick the vehicle to carry a shipment of `required_volume`: one of this
    /// org's vehicles with an active assigned driver, spare capacity, and no
    /// trip already in progress — the closest such vehicle to `customer`.
    fn select_free_vehicle(
        &self,
        conn: &mut mysql::PooledConn,
        required_volume: i64,
        customer: &Customer,
    ) -> Result<String, Box<dyn Error>> {
        Driver::ensure_table(conn)?;
        crate::logistics::dispatch::dispatch::ensure_tables(conn)?;

        let vehicle_rows: Vec<(String, Option<f64>, Option<f64>)> = conn.exec_map(
            "SELECT v.registration_number, v.latitude, v.longitude
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
            |(reg, lat, lng)| (reg, lat, lng),
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

        let (cust_lat, cust_lng) = match &customer.location {
            Some(loc) => (loc.latitude, loc.longitude),
            None => return Err("Customer location is not set for dispatch".into()),
        };
        let (org_lat, org_lng) = match &self.location {
            Some(loc) => (loc.latitude, loc.longitude),
            None => (0.0, 0.0),
        };

        let mut nearest: Option<String> = None;
        let mut min_distance = f64::MAX;
        for (reg, v_lat, v_lng) in vehicle_rows {
            let dist = haversine_distance_km(
                v_lat.unwrap_or(org_lat),
                v_lng.unwrap_or(org_lng),
                cust_lat,
                cust_lng,
            );
            if dist < min_distance {
                min_distance = dist;
                nearest = Some(reg);
            }
        }
        nearest.ok_or_else(|| "Failed to select vehicle for dispatch".into())
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
                "SELECT registration_number, capacity, unit, assigned_driver_id, latitude, longitude, last_updated_at, location_address, tracker_key FROM Vehicle WHERE org_id = :org_id",
                params! { "org_id" => &org_id_str },
                |(reg, cap, unit_str, driver, v_lat, v_lng, v_ts, v_addr, tracker): (String, i64, String, Option<String>, Option<f64>, Option<f64>, Option<i64>, Option<String>, Option<String>)| {
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
                        tracker_key: tracker
                            .and_then(|t| Uuid::parse_str(&t).ok())
                            .unwrap_or_else(Uuid::new_v4),
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

    /// A one-line dispatch request, the common case in these tests.
    fn line(description: &str, quantity: i64) -> DispatchLineItemInput {
        DispatchLineItemInput {
            stock_description: description.to_string(),
            requested_quantity: quantity,
        }
    }

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
        let mut customer = Customer::create_customer(org.id, "Tech Store India", "Connaught Place, New Delhi")
            .expect("Failed to create customer");
        customer.update_location(28.6200, 77.2100, Some("Connaught Place")).expect("Failed to update customer location");

        // Dispatch 15 Laptops to Customer
        let dispatch_res = org.dispatch_stock_to_customer(&customer, &[line("High-End Laptops", 15)]);
        assert!(dispatch_res.is_ok(), "Failed to dispatch stock to customer");

        let dispatch_order = dispatch_res.unwrap();
        // Vehicle 2 (UP16 BZ 2222) in Noida is closest to Delhi customer vs Vehicle 1 in Mumbai
        assert_eq!(dispatch_order.vehicle_registration_number, "UP16 BZ 2222");
        assert_eq!(dispatch_order.line_items.len(), 1);
        assert_eq!(dispatch_order.line_items[0].stock_description, "High-End Laptops");
        assert_eq!(dispatch_order.total_quantity(), 15);
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
    fn test_dispatch_stock_to_customer_reindexes_the_drawn_down_stock_chunk() {
        use crate::logistics::ai::chunk;
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Global Logistics", "Delhi HQ")
            .expect("Failed to create organization");
        org.update_location(28.6139, 77.2090, Some("Delhi HQ")).expect("Failed to update org location");

        let godown = Godown::create(org.id, "Delhi Godown", "Okhla Phase 1", None)
            .expect("Failed to create godown");
        Stock::new(50, 100, "High-End Laptops").add_to_godown(godown.id).expect("Failed to add stock");

        let driver = Driver::create(org.id, "Driver One", "LIC-1", "111").expect("driver");
        let mut vehicle = Vehicle::new("UP16 BZ 2222", 10_000, Unit::MetricTon);
        vehicle.add_new_vehicle_to_org(&org).expect("Failed to add vehicle");
        vehicle.update_location(28.5355, 77.3910, Some("Noida Hub")).expect("Failed to update vehicle location");
        vehicle.assign_driver(Some(driver.id)).expect("assign driver");

        let mut customer = Customer::create_customer(org.id, "Tech Store India", "Connaught Place, New Delhi")
            .expect("Failed to create customer");
        customer.update_location(28.6200, 77.2100, Some("Connaught Place")).expect("Failed to update customer location");

        org.dispatch_stock_to_customer(&customer, &[line("High-End Laptops", 15)])
            .expect("Failed to dispatch stock to customer");

        let results = chunk::search_by_org(org.id, "Delhi Godown Laptops", 8).expect("search");
        assert!(
            results.iter().any(|c| c.text.contains("Delhi Godown") && c.text.contains("85 units of High-End Laptops")),
            "draw_down_plans should reindex the stock chunk with the decremented quantity: {results:?}"
        );
    }

    #[test]
    fn test_dispatch_carries_several_line_items_in_one_trip() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Mixed Load Co", "Pune HQ").expect("org");
        org.update_location(18.52, 73.85, Some("Pune")).expect("org loc");

        // Two stock items in one godown, a third split across a second godown.
        let g1 = Godown::create(org.id, "G1", "MIDC A", None).expect("g1");
        let g2 = Godown::create(org.id, "G2", "MIDC B", None).expect("g2");
        Stock::new(2, 100, "Cement").add_to_godown(g1.id).expect("cement");
        Stock::new(3, 40, "Rebar").add_to_godown(g1.id).expect("rebar g1");
        Stock::new(3, 40, "Rebar").add_to_godown(g2.id).expect("rebar g2");
        Stock::new(1, 500, "Sand").add_to_godown(g1.id).expect("sand");

        let driver = Driver::create(org.id, "Mix Driver", "LIC-M", "0").expect("driver");
        let mut v = Vehicle::new("MH14 MX 0001", 10_000, Unit::MetricTon);
        v.add_new_vehicle_to_org(&org).expect("vehicle");
        v.update_location(18.52, 73.85, Some("Pune")).expect("v loc");
        v.assign_driver(Some(driver.id)).expect("assign");

        let mut customer = Customer::create_customer(org.id, "Site Buyer", "Baner").expect("customer");
        customer.update_location(18.55, 73.78, Some("Baner")).expect("cust loc");

        // Rebar: 60 units, more than either godown alone holds (40 + 40).
        let order = org
            .dispatch_stock_to_customer(
                &customer,
                &[line("Cement", 30), line("Rebar", 60), line("Sand", 200)],
            )
            .expect("multi-line dispatch");

        assert_eq!(order.line_items.len(), 3);
        assert_eq!(order.total_quantity(), 290);

        // Every line drew down: cement 100->70, rebar 80->20 total, sand 500->300.
        let mut conn = DbConnection::from_env().get_connection().unwrap();
        let cement: Option<i64> = conn
            .exec_first(
                "SELECT quantity FROM Stock WHERE godown_id = :g AND description = 'Cement'",
                params! { "g" => g1.id.to_string() },
            )
            .unwrap();
        assert_eq!(cement, Some(70));
        let rebar_total: Option<i64> = conn
            .exec_first(
                "SELECT COALESCE(SUM(quantity), 0) FROM Stock s
                 JOIN Godowns gd ON gd.id = s.godown_id
                 WHERE gd.org_id = :org AND s.description = 'Rebar'",
                params! { "org" => org.id.to_string() },
            )
            .unwrap();
        assert_eq!(rebar_total, Some(20));

        // Reloads with all three lines, ordered as inserted.
        let reloaded = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
        let descs: Vec<&str> = reloaded
            .line_items
            .iter()
            .map(|li| li.stock_description.as_str())
            .collect();
        assert_eq!(descs, ["Cement", "Rebar", "Sand"]);
    }

    #[test]
    fn test_dispatch_rejects_the_whole_order_if_any_line_is_short() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("All Or Nothing", "HQ").expect("org");
        org.update_location(18.52, 73.85, Some("HQ")).expect("loc");
        let g = Godown::create(org.id, "G", "addr", None).expect("g");
        Stock::new(1, 100, "Widgets").add_to_godown(g.id).expect("widgets");
        Stock::new(1, 5, "Gadgets").add_to_godown(g.id).expect("gadgets");

        let driver = Driver::create(org.id, "D", "L", "0").expect("driver");
        let mut v = Vehicle::new("MH14 AN 0001", 10_000, Unit::MetricTon);
        v.add_new_vehicle_to_org(&org).expect("vehicle");
        v.update_location(18.52, 73.85, Some("HQ")).expect("v loc");
        v.assign_driver(Some(driver.id)).expect("assign");

        let mut customer = Customer::create_customer(org.id, "Buyer", "addr").expect("customer");
        customer.update_location(18.55, 73.78, Some("addr")).expect("cust loc");

        let err = org
            .dispatch_stock_to_customer(&customer, &[line("Widgets", 10), line("Gadgets", 50)])
            .expect_err("Gadgets is short, so the whole dispatch is rejected");
        assert!(err.to_string().contains("Gadgets"), "unexpected: {err}");

        // Widgets must NOT have been drawn down — the order is all-or-nothing.
        let mut conn = DbConnection::from_env().get_connection().unwrap();
        let widgets: Option<i64> = conn
            .exec_first(
                "SELECT quantity FROM Stock WHERE godown_id = :g AND description = 'Widgets'",
                params! { "g" => g.id.to_string() },
            )
            .unwrap();
        assert_eq!(widgets, Some(100));
    }

    #[test]
    fn test_dispatch_rejects_an_empty_or_duplicated_line_list() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Edge Cases", "HQ").expect("org");
        let customer = Customer::create_customer(org.id, "Buyer", "addr").expect("customer");

        assert!(
            org.dispatch_stock_to_customer(&customer, &[]).is_err(),
            "an empty line list is rejected"
        );
        let dup = org
            .dispatch_stock_to_customer(&customer, &[line("Cement", 5), line("Cement", 3)])
            .expect_err("a repeated description is rejected");
        assert!(dup.to_string().contains("more than once"), "unexpected: {dup}");
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

        let mut customer = Customer::create_customer(org.id, "Tile Mart", "FC Road, Pune").expect("customer");
        customer.update_location(18.52, 73.84, Some("FC Road")).expect("customer location");

        // No driver assigned yet -> rejected.
        let err = org
            .dispatch_stock_to_customer(&customer, &[line("Ceramic Tiles", 10)])
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
            org.dispatch_stock_to_customer(&customer, &[line("Ceramic Tiles", 10)])
                .is_err(),
            "dispatch should fail with an inactive driver"
        );

        // Reactivate the driver -> dispatch succeeds.
        driver.update("Back To Work", "LIC-X", "000", true).expect("reactivate");
        let order = org
            .dispatch_stock_to_customer(&customer, &[line("Ceramic Tiles", 10)])
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

        let mut customer = Customer::create_customer(org.id, "Stone Co", "Civil Lines").expect("customer");
        customer.update_location(21.15, 79.09, Some("Civil Lines")).expect("cust loc");

        let driver = Driver::create(org.id, "Cap Driver", "LIC-C", "000").expect("driver");

        // Too-small vehicle (capacity 40 < 50) -> rejected.
        let mut small = Vehicle::new("MH31 SM 0001", 40, Unit::MetricTon);
        small.add_new_vehicle_to_org(&org).expect("add small");
        small.update_location(21.15, 79.08, Some("Nagpur")).expect("loc");
        small.assign_driver(Some(driver.id)).expect("assign");

        let err = org
            .dispatch_stock_to_customer(&customer, &[line("Marble Slabs", 5)])
            .expect_err("should reject: vehicle too small");
        assert!(err.to_string().contains("capacity >= 50"), "unexpected: {err}");

        // Big-enough vehicle -> succeeds and is the one chosen.
        let driver2 = Driver::create(org.id, "Big Driver", "LIC-B", "000").expect("driver2");
        let mut big = Vehicle::new("MH31 BG 0002", 500, Unit::MetricTon);
        big.add_new_vehicle_to_org(&org).expect("add big");
        big.update_location(21.15, 79.08, Some("Nagpur")).expect("loc");
        big.assign_driver(Some(driver2.id)).expect("assign");

        let order = org
            .dispatch_stock_to_customer(&customer, &[line("Marble Slabs", 5)])
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

        let mut customer = Customer::create_customer(org.id, "Textile Buyer", "Ring Road").expect("customer");
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
            .dispatch_stock_to_customer(&customer, &[line("Fabric Rolls", 10)])
            .expect("first dispatch");
        let second = org
            .dispatch_stock_to_customer(&customer, &[line("Fabric Rolls", 10)])
            .expect("second dispatch uses the other vehicle");
        assert_ne!(
            first.vehicle_registration_number, second.vehicle_registration_number,
            "the same vehicle must not be booked onto two active trips"
        );

        // Both vehicles are now on active trips -> a third dispatch fails.
        let err = org
            .dispatch_stock_to_customer(&customer, &[line("Fabric Rolls", 10)])
            .expect_err("third dispatch: no vehicle free");
        assert!(err.to_string().contains("active trip"), "unexpected: {err}");

        // Completing the first trip frees its vehicle for reuse.
        let mut first = DispatchOrder::get_by_id(first.id).expect("get").expect("exists");
        first.transition_to(DispatchStatus::Confirmed, None, None).expect("confirm");
        first.transition_to(DispatchStatus::Loaded, None, None).expect("load");
        first.transition_to(DispatchStatus::InTransit, None, None).expect("in transit");
        first
            .transition_to(
                DispatchStatus::Delivered,
                Some(crate::logistics::dispatch::dispatch::ProofOfDeliveryInput {
                    receiver_name: "Buyer".to_string(),
                    signature_or_photo_url: "https://example.test/sig.png".to_string(),
                }),
                None,
            )
            .expect("deliver");

        let fourth = org
            .dispatch_stock_to_customer(&customer, &[line("Fabric Rolls", 10)])
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

    // ── Multi-stop trips ────────────────────────────────────────────────────

    /// An org at Pune with one big truck + active driver and a godown stocked
    /// with plenty of Cement, plus `n` located customers.
    fn trip_ready_org(n: usize) -> (Organization, Vec<Customer>) {
        let mut org = Organization::create_organization("Trip Co", "Pune HQ").expect("org");
        org.update_location(18.52, 73.85, Some("Pune")).expect("org loc");
        let g = Godown::create(org.id, "G", "MIDC", None).expect("godown");
        Stock::new(1, 10_000, "Cement").add_to_godown(g.id).expect("stock");
        let driver = Driver::create(org.id, "Trip Driver", "LIC-T", "0").expect("driver");
        let mut v = Vehicle::new("MH14 TR 0001", 100_000, Unit::MetricTon);
        v.add_new_vehicle_to_org(&org).expect("vehicle");
        v.update_location(18.52, 73.85, Some("Pune")).expect("v loc");
        v.assign_driver(Some(driver.id)).expect("assign");

        let customers = (0..n)
            .map(|i| {
                let mut c = Customer::create_customer(org.id, format!("Buyer {i}"), format!("{i} St"))
                    .expect("customer");
                c.update_location(18.5 + i as f64 * 0.01, 73.8, Some("loc")).expect("loc");
                c
            })
            .collect();
        (org, customers)
    }

    #[test]
    fn test_trip_needs_at_least_two_distinct_stops() {
        let _db = TestDb::create();
        let (org, customers) = trip_ready_org(1);
        let err = org
            .dispatch_trip_to_customers(&[(&customers[0], &[line("Cement", 1)][..])])
            .unwrap_err();
        assert!(err.to_string().contains("at least two stops"));

        let (org2, customers2) = trip_ready_org(1);
        let dup = &customers2[0];
        let err = org2
            .dispatch_trip_to_customers(&[
                (dup, &[line("Cement", 1)][..]),
                (dup, &[line("Cement", 1)][..]),
            ])
            .unwrap_err();
        assert!(err.to_string().contains("same customer"));
    }

    #[test]
    fn test_trip_puts_every_stop_on_one_vehicle_and_draws_stock_down() {
        let _db = TestDb::create();
        let (org, customers) = trip_ready_org(3);

        let trip = org
            .dispatch_trip_to_customers(&[
                (&customers[0], &[line("Cement", 10)][..]),
                (&customers[1], &[line("Cement", 20)][..]),
                (&customers[2], &[line("Cement", 5)][..]),
            ])
            .expect("trip");

        assert_eq!(trip.stops.len(), 3);
        // One truck for the whole trip.
        assert!(trip.stops.iter().all(|s| s.vehicle_registration_number == trip.vehicle_registration_number));
        // Sequenced 1, 2, 3 and all linked to the trip.
        assert_eq!(
            trip.stops.iter().filter_map(|s| s.stop_sequence).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(trip.stops.iter().all(|s| s.trip_id == Some(trip.id)));

        // 35 units drawn: 10_000 - 35 = 9_965.
        let mut conn = DbConnection::from_env().get_connection().unwrap();
        let left: Option<i64> = conn
            .exec_first(
                "SELECT SUM(quantity) FROM Stock WHERE description = 'Cement'",
                (),
            )
            .unwrap();
        assert_eq!(left, Some(9_965));

        // Reload via Trip::get_by_id — status starts PLANNED.
        let reloaded = Trip::get_by_id(trip.id).expect("get").expect("exists");
        assert_eq!(reloaded.stops.len(), 3);
        assert_eq!(reloaded.status, crate::logistics::dispatch::trip::TripStatus::Planned);
    }

    #[test]
    fn test_trip_is_all_or_nothing_when_a_later_stop_is_short() {
        let _db = TestDb::create();
        let (org, customers) = trip_ready_org(2);

        let err = org
            .dispatch_trip_to_customers(&[
                (&customers[0], &[line("Cement", 10)][..]),
                (&customers[1], &[line("Cement", 99_999)][..]), // can't be met
            ])
            .unwrap_err();
        assert!(err.to_string().contains("Stop 2"));

        // Nothing was drawn and no trip / dispatches were created.
        let mut conn = DbConnection::from_env().get_connection().unwrap();
        let left: Option<i64> = conn
            .exec_first("SELECT SUM(quantity) FROM Stock WHERE description = 'Cement'", ())
            .unwrap();
        assert_eq!(left, Some(10_000));
        assert!(Trip::list_by_org(org.id).expect("list").is_empty());
        assert!(DispatchOrder::list_by_org(org.id).expect("list").is_empty());
    }

    #[test]
    fn test_trip_rejected_when_no_vehicle_fits_the_combined_load() {
        let _db = TestDb::create();
        let mut org = Organization::create_organization("Small Truck Co", "HQ").expect("org");
        org.update_location(18.5, 73.8, Some("x")).expect("loc");
        let g = Godown::create(org.id, "G", "A", None).expect("g");
        // volume 10 each — two stops of 5 units = 100 combined volume.
        Stock::new(10, 1000, "Rebar").add_to_godown(g.id).expect("stock");
        let d = Driver::create(org.id, "D", "L", "0").expect("d");
        let mut v = Vehicle::new("SM 0001", 60, Unit::MetricTon); // fits one stop (50), not both (100)
        v.add_new_vehicle_to_org(&org).expect("v");
        v.assign_driver(Some(d.id)).expect("assign");

        let mut c1 = Customer::create_customer(org.id, "C1", "1").expect("c1");
        c1.update_location(18.5, 73.8, Some("x")).expect("l");
        let mut c2 = Customer::create_customer(org.id, "C2", "2").expect("c2");
        c2.update_location(18.5, 73.8, Some("x")).expect("l");

        let err = org
            .dispatch_trip_to_customers(&[
                (&c1, &[line("Rebar", 5)][..]),
                (&c2, &[line("Rebar", 5)][..]),
            ])
            .unwrap_err();
        assert!(err.to_string().contains("free and large enough"));
    }
}
