use crate::logistics::db::connection::DbConnection;
use crate::logistics::stock::stock::Stock;
use crate::logistics::vehicle::vehicle::Location;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A warehouse ("godown") belonging to an organization. Stock is held in a
/// godown rather than directly by the organization.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Godown {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub address: String,
    pub location: Option<Location>,
    /// Maximum total volume this godown can hold, in the same abstract units
    /// as a stock item's `volume_in_size * quantity`. `None` means no limit.
    /// Enforced on stock add/update via [`Godown::check_capacity_for`].
    pub max_capacity: Option<i64>,
    /// Stock held in this godown. Populated by [`Godown::get_by_id`] and
    /// [`Godown::list_by_org`]; empty on a freshly created value.
    pub stock: Vec<Stock>,
}

impl Godown {
    /// Create the `Godowns` table if it does not exist. Kept in sync with
    /// `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS Godowns (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                name VARCHAR(255) NOT NULL,
                address VARCHAR(255) NOT NULL,
                max_capacity BIGINT DEFAULT NULL,
                latitude DOUBLE DEFAULT NULL,
                longitude DOUBLE DEFAULT NULL,
                last_updated_at BIGINT DEFAULT NULL,
                location_address VARCHAR(255) DEFAULT NULL,
                CONSTRAINT fk_godown_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    pub fn create(
        org_id: Uuid,
        name: impl Into<String>,
        address: impl Into<String>,
        max_capacity: Option<i64>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        let godown = Godown {
            id: Uuid::new_v4(),
            org_id,
            name: name.into(),
            address: address.into(),
            location: None,
            max_capacity,
            stock: Vec::new(),
        };

        conn.exec_drop(
            "INSERT INTO Godowns (id, org_id, name, address, max_capacity)
             VALUES (:id, :org_id, :name, :address, :max_capacity)",
            params! {
                "id" => godown.id.to_string(),
                "org_id" => godown.org_id.to_string(),
                "name" => &godown.name,
                "address" => &godown.address,
                "max_capacity" => godown.max_capacity,
            },
        )?;

        Ok(godown)
    }

    pub fn update(
        &mut self,
        name: impl Into<String>,
        address: impl Into<String>,
        max_capacity: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        let new_name = name.into();
        let new_address = address.into();

        let mut conn = DbConnection::from_env()
            .get_connection()?;

        conn.exec_drop(
            "UPDATE Godowns SET name = :name, address = :address, max_capacity = :max_capacity WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "name" => &new_name,
                "address" => &new_address,
                "max_capacity" => max_capacity,
            },
        )?;

        self.name = new_name;
        self.address = new_address;
        self.max_capacity = max_capacity;
        Ok(())
    }

    /// Total volume currently stored: Σ(`volume_in_size` × `quantity`) over
    /// the godown's loaded `stock`. Meaningful only when `stock` is populated
    /// (i.e. on a value from [`Godown::get_by_id`] / [`Godown::list_by_org`]).
    pub fn used_capacity(&self) -> i64 {
        self.stock
            .iter()
            .map(|s| s.volume_in_size.saturating_mul(s.quantity))
            .sum()
    }

    /// Check whether bringing this godown's holding of one stock item to
    /// `incoming_volume` (`volume_in_size` × `quantity`) would exceed
    /// `max_capacity`. `replacing` is the description of a stock item being
    /// updated in place — its current contribution is excluded from the
    /// total — or `None` when adding a new item.
    ///
    /// Returns `Ok(projected_total)` when it fits (always `Ok` when
    /// `max_capacity` is `None`), or `Err(message)` describing the overflow.
    pub fn check_capacity_for(
        &self,
        incoming_volume: i64,
        replacing: Option<&str>,
    ) -> Result<i64, String> {
        let others: i64 = self
            .stock
            .iter()
            .filter(|s| Some(s.description.as_str()) != replacing)
            .map(|s| s.volume_in_size.saturating_mul(s.quantity))
            .sum();
        let projected = others.saturating_add(incoming_volume);

        match self.max_capacity {
            Some(limit) if projected > limit => Err(format!(
                "Godown capacity exceeded: max_capacity is {limit}, this change would bring the total stored volume to {projected}"
            )),
            _ => Ok(projected),
        }
    }

    pub fn update_location(
        &mut self,
        latitude: f64,
        longitude: f64,
        address: Option<impl Into<String>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let address_str = address.map(|a| a.into());

        conn.exec_drop(
            "UPDATE Godowns SET latitude = :latitude, longitude = :longitude, last_updated_at = :last_updated_at, location_address = :location_address WHERE id = :id",
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

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<(String, String, String, Option<i64>, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn
            .exec_first(
                "SELECT org_id, name, address, max_capacity, latitude, longitude, last_updated_at, location_address FROM Godowns WHERE id = :id",
                params! { "id" => id.to_string() },
            )?;

        let (org_id_str, name, address, max_capacity, lat, lng, ts, addr) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let location = lat.map(|latitude| Location {
            latitude,
            longitude: lng.unwrap_or(0.0),
            timestamp: ts.unwrap_or(0),
            address: addr,
        });

        Ok(Some(Godown {
            id,
            org_id: Uuid::parse_str(&org_id_str).unwrap_or_else(|_| Uuid::new_v4()),
            name,
            address,
            location,
            max_capacity,
            stock: Stock::list_by_godown(&mut conn, id)?,
        }))
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<(String, String, String, Option<i64>, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn.exec_map(
            "SELECT id, name, address, max_capacity, latitude, longitude, last_updated_at, location_address FROM Godowns WHERE org_id = :org_id ORDER BY name",
            params! { "org_id" => org_id.to_string() },
            |(id, name, address, max_capacity, lat, lng, ts, addr)| (id, name, address, max_capacity, lat, lng, ts, addr),
        )?;

        let mut godowns = Vec::with_capacity(rows.len());
        for (id_str, name, address, max_capacity, lat, lng, ts, addr) in rows {
            let gid = Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4());
            let location = lat.map(|latitude| Location {
                latitude,
                longitude: lng.unwrap_or(0.0),
                timestamp: ts.unwrap_or(0),
                address: addr,
            });
            godowns.push(Godown {
                id: gid,
                org_id,
                name,
                address,
                location,
                max_capacity,
                stock: Stock::list_by_godown(&mut conn, gid)?,
            });
        }
        Ok(godowns)
    }

    pub fn remove(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;
        conn.exec_drop(
            "DELETE FROM Godowns WHERE id = :id",
            params! { "id" => self.id.to_string() },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::TestDb;

    fn make_org() -> Organization {
        Organization::create_organization("Godown Test Org", "1 Warehouse Way")
            .expect("create org")
    }

    #[test]
    fn test_create_and_get_godown() {
        let _db = TestDb::create();
        let org = make_org();
        let g = Godown::create(org.id, "North Godown", "Plot 5, Industrial Area", None).expect("create godown");

        let fetched = Godown::get_by_id(g.id).expect("get_by_id").expect("godown exists");
        assert_eq!(fetched.name, "North Godown");
        assert_eq!(fetched.org_id, org.id);
        assert!(fetched.location.is_none());
        assert!(fetched.stock.is_empty());
        assert_eq!(fetched.max_capacity, None);
    }

    #[test]
    fn test_update_godown_and_location() {
        let _db = TestDb::create();
        let org = make_org();
        let mut g = Godown::create(org.id, "Old Name", "Old Address", None).expect("create godown");

        g.update("New Name", "New Address", Some(5_000)).expect("update");
        g.update_location(19.07, 72.87, Some("Bandra")).expect("update location");

        let fetched = Godown::get_by_id(g.id).expect("get").expect("exists");
        assert_eq!(fetched.name, "New Name");
        assert_eq!(fetched.address, "New Address");
        assert_eq!(fetched.max_capacity, Some(5_000));
        let loc = fetched.location.expect("location set");
        assert_eq!(loc.latitude, 19.07);
        assert_eq!(loc.address.as_deref(), Some("Bandra"));
    }

    #[test]
    fn test_list_by_org_and_remove() {
        let _db = TestDb::create();
        let org = make_org();
        Godown::create(org.id, "G1", "A1", None).expect("g1");
        Godown::create(org.id, "G2", "A2", None).expect("g2");
        let g3 = Godown::create(org.id, "G3", "A3", None).expect("g3");

        let listed = Godown::list_by_org(org.id).expect("list");
        assert_eq!(listed.len(), 3);
        // ORDER BY name
        assert_eq!(listed[0].name, "G1");

        g3.remove().expect("remove");
        assert_eq!(Godown::list_by_org(org.id).expect("list").len(), 2);
    }

    #[test]
    fn test_deleting_org_cascades_to_godowns() {
        let _db = TestDb::create();
        let org = make_org();
        Godown::create(org.id, "Doomed Godown", "X", None).expect("create");
        org.remove_organization().expect("remove org");
        assert!(Godown::list_by_org(org.id).expect("list").is_empty());
    }

    #[test]
    fn test_check_capacity_for_enforces_max_capacity() {
        let _db = TestDb::create();
        let org = make_org();
        let g = Godown::create(org.id, "Capped Godown", "Bay 7", Some(1_000))
            .expect("create godown");

        Stock::new(10, 50, "Pallets A").add_to_godown(g.id).expect("add stock a"); // 500
        let g = Godown::get_by_id(g.id).expect("get").expect("exists");
        assert_eq!(g.used_capacity(), 500);

        // Another 500 exactly fills it.
        assert!(g.check_capacity_for(500, None).is_ok());
        // 501 tips it over.
        assert!(g.check_capacity_for(501, None).is_err());
        // Updating the existing 500 item up to 900 is fine — its old
        // contribution is excluded from the running total.
        assert!(g.check_capacity_for(900, Some("Pallets A")).is_ok());
        assert!(g.check_capacity_for(1_001, Some("Pallets A")).is_err());
    }

    #[test]
    fn test_check_capacity_for_is_unbounded_without_max_capacity() {
        let _db = TestDb::create();
        let org = make_org();
        let g = Godown::create(org.id, "Open Godown", "Yard", None).expect("create");
        assert_eq!(g.check_capacity_for(i64::MAX, None), Ok(i64::MAX));
    }
}
