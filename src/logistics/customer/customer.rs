use crate::logistics::db::connection::DbConnection;
use crate::logistics::vehicle::vehicle::Location;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Customer {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub location: Option<Location>,
}

impl Customer {
    pub fn create_customer(
        name: impl Into<String>,
        address: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;

        let customer = Customer {
            id: Uuid::new_v4(),
            name: name.into(),
            address: address.into(),
            location: None,
        };

        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Customers (
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

        conn.exec_drop(
            "INSERT INTO Customers (id, name, address) VALUES (:id, :name, :address)",
            params! {
                "id" => customer.id.to_string(),
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
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
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

    pub fn list_all() -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Customers (
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
            "SELECT id, name, address, latitude, longitude, last_updated_at, location_address FROM Customers",
            (),
            |(id, name, address, lat, lng, ts, addr)| (id, name, address, lat, lng, ts, addr),
        )?;

        let customers = rows
            .into_iter()
            .map(|(id, name, address, lat, lng, ts, addr)| {
                let location = lat.map(|latitude| Location {
                    latitude,
                    longitude: lng.unwrap_or(0.0),
                    timestamp: ts.unwrap_or(0),
                    address: addr,
                });
                Customer {
                    id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                    name,
                    address,
                    location,
                }
            })
            .collect();

        Ok(customers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_update_customer() {
        let customer_res = Customer::create_customer("Acme Retail Corp", "100 Market St, Mumbai");
        assert!(customer_res.is_ok(), "Failed to create customer");

        let mut customer = customer_res.unwrap();
        let lat = 19.0760;
        let lng = 72.8777;
        let addr = "Bandra West, Mumbai";

        let update_res = customer.update_location(lat, lng, Some(addr));
        assert!(update_res.is_ok(), "Failed to update customer location");

        let loc = customer.get_location().unwrap();
        assert_eq!(loc.latitude, lat);
        assert_eq!(loc.longitude, lng);

        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for customer verification");

        let row: Option<(String, String, Option<f64>, Option<f64>)> = conn
            .exec_first(
                "SELECT name, address, latitude, longitude FROM Customers WHERE id = :id",
                params! {
                    "id" => customer.id.to_string(),
                },
            )
            .expect("Failed to query database for customer");

        assert!(row.is_some(), "Customer record not found in database");
        let (db_name, db_address, db_lat, db_lng) = row.unwrap();
        assert_eq!(db_name, "Acme Retail Corp");
        assert_eq!(db_address, "100 Market St, Mumbai");
        assert_eq!(db_lat, Some(lat));
        assert_eq!(db_lng, Some(lng));
    }
}
