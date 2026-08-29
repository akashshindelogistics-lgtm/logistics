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
    ) -> Result<Self, Box<dyn Error>> {
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        let godown = Godown {
            id: Uuid::new_v4(),
            org_id,
            name: name.into(),
            address: address.into(),
            location: None,
            stock: Vec::new(),
        };

        conn.exec_drop(
            "INSERT INTO Godowns (id, org_id, name, address) VALUES (:id, :org_id, :name, :address)",
            params! {
                "id" => godown.id.to_string(),
                "org_id" => godown.org_id.to_string(),
                "name" => &godown.name,
                "address" => &godown.address,
            },
        )?;

        Ok(godown)
    }

    pub fn update(
        &mut self,
        name: impl Into<String>,
        address: impl Into<String>,
    ) -> Result<(), Box<dyn Error>> {
        let new_name = name.into();
        let new_address = address.into();

        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;

        conn.exec_drop(
            "UPDATE Godowns SET name = :name, address = :address WHERE id = :id",
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
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
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
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<(String, String, String, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn
            .exec_first(
                "SELECT org_id, name, address, latitude, longitude, last_updated_at, location_address FROM Godowns WHERE id = :id",
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

        Ok(Some(Godown {
            id,
            org_id: Uuid::parse_str(&org_id_str).unwrap_or_else(|_| Uuid::new_v4()),
            name,
            address,
            location,
            stock: Stock::list_by_godown(&mut conn, id)?,
        }))
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<(String, String, String, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn.exec_map(
            "SELECT id, name, address, latitude, longitude, last_updated_at, location_address FROM Godowns WHERE org_id = :org_id ORDER BY name",
            params! { "org_id" => org_id.to_string() },
            |(id, name, address, lat, lng, ts, addr)| (id, name, address, lat, lng, ts, addr),
        )?;

        let mut godowns = Vec::with_capacity(rows.len());
        for (id_str, name, address, lat, lng, ts, addr) in rows {
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
                stock: Stock::list_by_godown(&mut conn, gid)?,
            });
        }
        Ok(godowns)
    }

    pub fn remove(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
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
    use crate::logistics::test_support::reset_database;
    use serial_test::serial;

    fn make_org() -> Organization {
        Organization::create_organization("Godown Test Org", "1 Warehouse Way")
            .expect("create org")
    }

    #[test]
    #[serial(db)]
    fn test_create_and_get_godown() {
        reset_database();
        let org = make_org();
        let g = Godown::create(org.id, "North Godown", "Plot 5, Industrial Area").expect("create godown");

        let fetched = Godown::get_by_id(g.id).expect("get_by_id").expect("godown exists");
        assert_eq!(fetched.name, "North Godown");
        assert_eq!(fetched.org_id, org.id);
        assert!(fetched.location.is_none());
        assert!(fetched.stock.is_empty());
    }

    #[test]
    #[serial(db)]
    fn test_update_godown_and_location() {
        reset_database();
        let org = make_org();
        let mut g = Godown::create(org.id, "Old Name", "Old Address").expect("create godown");

        g.update("New Name", "New Address").expect("update");
        g.update_location(19.07, 72.87, Some("Bandra")).expect("update location");

        let fetched = Godown::get_by_id(g.id).expect("get").expect("exists");
        assert_eq!(fetched.name, "New Name");
        assert_eq!(fetched.address, "New Address");
        let loc = fetched.location.expect("location set");
        assert_eq!(loc.latitude, 19.07);
        assert_eq!(loc.address.as_deref(), Some("Bandra"));
    }

    #[test]
    #[serial(db)]
    fn test_list_by_org_and_remove() {
        reset_database();
        let org = make_org();
        Godown::create(org.id, "G1", "A1").expect("g1");
        Godown::create(org.id, "G2", "A2").expect("g2");
        let g3 = Godown::create(org.id, "G3", "A3").expect("g3");

        let listed = Godown::list_by_org(org.id).expect("list");
        assert_eq!(listed.len(), 3);
        // ORDER BY name
        assert_eq!(listed[0].name, "G1");

        g3.remove().expect("remove");
        assert_eq!(Godown::list_by_org(org.id).expect("list").len(), 2);
    }

    #[test]
    #[serial(db)]
    fn test_deleting_org_cascades_to_godowns() {
        reset_database();
        let org = make_org();
        Godown::create(org.id, "Doomed Godown", "X").expect("create");
        org.remove_organization().expect("remove org");
        assert!(Godown::list_by_org(org.id).expect("list").is_empty());
    }
}
