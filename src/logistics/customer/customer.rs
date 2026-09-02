use crate::logistics::db::connection::DbConnection;
use crate::logistics::vehicle::vehicle::Location;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// A delivery recipient belonging to a single organization. Customers are
/// **not** shared between organizations: every customer is created under one
/// org (`POST /api/orgs/{id}/customers`), only that org can list, locate,
/// delete, or dispatch to it, and deleting the org cascades to its customers.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Customer {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub address: String,
    pub location: Option<Location>,
}

impl Customer {
    /// Create the `Customers` table if it does not exist. Kept in sync with
    /// `test_support::migrate`.
    ///
    /// Customers used to be a flat, org-less table. A database created before
    /// this change still has that schema; there is no hosted backend and
    /// dev/test customers are disposable, so the legacy table is dropped and
    /// recreated rather than carrying a real data migration (same approach the
    /// godown change took for `Stock` — see `docs/customers.md`).
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        let customers_exists: Option<i64> = conn.exec_first(
            "SELECT 1 FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name = 'Customers'",
            (),
        )?;
        if customers_exists.is_some() {
            let has_org_id: Option<i64> = conn.exec_first(
                "SELECT 1 FROM information_schema.columns
                 WHERE table_schema = DATABASE() AND table_name = 'Customers'
                   AND column_name = 'org_id'",
                (),
            )?;
            if has_org_id.is_none() {
                conn.query_drop("DROP TABLE Customers")?;
            }
        }

        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS Customers (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                name VARCHAR(255) NOT NULL,
                address VARCHAR(255) NOT NULL,
                latitude DOUBLE DEFAULT NULL,
                longitude DOUBLE DEFAULT NULL,
                last_updated_at BIGINT DEFAULT NULL,
                location_address VARCHAR(255) DEFAULT NULL,
                CONSTRAINT fk_customer_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    pub fn create_customer(
        org_id: Uuid,
        name: impl Into<String>,
        address: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        Self::ensure_table(&mut conn)?;

        let customer = Customer {
            id: Uuid::new_v4(),
            org_id,
            name: name.into(),
            address: address.into(),
            location: None,
        };

        conn.exec_drop(
            "INSERT INTO Customers (id, org_id, name, address) VALUES (:id, :org_id, :name, :address)",
            params! {
                "id" => customer.id.to_string(),
                "org_id" => customer.org_id.to_string(),
                "name" => &customer.name,
                "address" => &customer.address,
            },
        )?;

        Ok(customer)
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
            "UPDATE Customers SET latitude = :latitude, longitude = :longitude, last_updated_at = :last_updated_at, location_address = :location_address WHERE id = :id",
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

    fn row_to_customer(
        id_str: String,
        org_id_str: String,
        name: String,
        address: String,
        lat: Option<f64>,
        lng: Option<f64>,
        ts: Option<i64>,
        addr: Option<String>,
    ) -> Self {
        let location = lat.map(|latitude| Location {
            latitude,
            longitude: lng.unwrap_or(0.0),
            timestamp: ts.unwrap_or(0),
            address: addr,
        });
        Customer {
            id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id_str).unwrap_or_else(|_| Uuid::new_v4()),
            name,
            address,
            location,
        }
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<(String, String, String, String, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn
            .exec_first(
                "SELECT id, org_id, name, address, latitude, longitude, last_updated_at, location_address FROM Customers WHERE id = :id",
                params! { "id" => id.to_string() },
            )?;

        Ok(row.map(|(id, org_id, name, address, lat, lng, ts, addr)| {
            Self::row_to_customer(id, org_id, name, address, lat, lng, ts, addr)
        }))
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, Option<f64>, Option<f64>, Option<i64>, Option<String>)> = conn.exec_map(
            "SELECT id, org_id, name, address, latitude, longitude, last_updated_at, location_address FROM Customers WHERE org_id = :org_id ORDER BY name",
            params! { "org_id" => org_id.to_string() },
            |(id, org_id, name, address, lat, lng, ts, addr)| (id, org_id, name, address, lat, lng, ts, addr),
        )?;

        Ok(rows
            .into_iter()
            .map(|(id, org_id, name, address, lat, lng, ts, addr)| {
                Self::row_to_customer(id, org_id, name, address, lat, lng, ts, addr)
            })
            .collect())
    }

    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        conn.exec_drop(
            "DELETE FROM Customers WHERE id = :id",
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

    fn make_org(name: &str) -> Organization {
        Organization::create_organization(name, "1 Market St").expect("create org")
    }

    #[test]
    fn test_create_and_update_customer() {
        let _db = TestDb::create();
        let org = make_org("Acme Org");

        let customer_res = Customer::create_customer(org.id, "Acme Retail Corp", "100 Market St, Mumbai");
        assert!(customer_res.is_ok(), "Failed to create customer");

        let mut customer = customer_res.unwrap();
        assert_eq!(customer.org_id, org.id);
        let lat = 19.0760;
        let lng = 72.8777;
        let addr = "Bandra West, Mumbai";

        let update_res = customer.update_location(lat, lng, Some(addr));
        assert!(update_res.is_ok(), "Failed to update customer location");

        let loc = customer.get_location().unwrap();
        assert_eq!(loc.latitude, lat);
        assert_eq!(loc.longitude, lng);

        let fetched = Customer::get_by_id(customer.id).expect("get").expect("exists");
        assert_eq!(fetched.name, "Acme Retail Corp");
        assert_eq!(fetched.address, "100 Market St, Mumbai");
        assert_eq!(fetched.org_id, org.id);
        assert_eq!(fetched.location.map(|l| l.latitude), Some(lat));
    }

    #[test]
    fn test_list_by_org_is_scoped() {
        let _db = TestDb::create();
        let org_a = make_org("Org A");
        let org_b = make_org("Org B");

        Customer::create_customer(org_a.id, "Beta Stores", "2 St").expect("create");
        Customer::create_customer(org_a.id, "Alpha Stores", "1 St").expect("create");
        Customer::create_customer(org_b.id, "Gamma Stores", "3 St").expect("create");

        let a = Customer::list_by_org(org_a.id).expect("list a");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].name, "Alpha Stores"); // ORDER BY name
        assert!(a.iter().all(|c| c.org_id == org_a.id));

        let b = Customer::list_by_org(org_b.id).expect("list b");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].name, "Gamma Stores");
    }

    #[test]
    fn test_delete_customer() {
        let _db = TestDb::create();
        let org = make_org("Del Org");
        let customer = Customer::create_customer(org.id, "Doomed Co", "9 St").expect("create");

        customer.delete().expect("delete");
        assert!(Customer::get_by_id(customer.id).expect("get").is_none());
    }

    #[test]
    fn test_deleting_org_cascades_to_customers() {
        let _db = TestDb::create();
        let org = make_org("Cascade Org");
        Customer::create_customer(org.id, "Will Vanish", "0 St").expect("create");

        org.remove_organization().expect("remove org");
        assert!(Customer::list_by_org(org.id).expect("list").is_empty());
    }
}
