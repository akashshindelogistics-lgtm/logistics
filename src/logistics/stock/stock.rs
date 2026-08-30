use crate::logistics::db::connection::DbConnection;
use crate::logistics::orgs::orgs::Organization;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Stock {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
}

impl Stock {
    pub fn new(volume_in_size: i64, quantity: i64, description: impl Into<String>) -> Self {
        Stock {
            volume_in_size,
            quantity,
            description: description.into(),
        }
    }

    pub fn add_new_stock(&self, org: &Organization) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        // Ensure Stock table exists in database
        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS Stock (
                id INT AUTO_INCREMENT PRIMARY KEY,
                volume_in_size BIGINT NOT NULL,
                quantity BIGINT NOT NULL,
                description VARCHAR(255) NOT NULL,
                org_id VARCHAR(36) NOT NULL,
                CONSTRAINT fk_stock_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
            (),
        )?;

        // Insert stock record into MySQL database
        conn.exec_drop(
            "INSERT INTO Stock (volume_in_size, quantity, description, org_id) 
             VALUES (:volume_in_size, :quantity, :description, :org_id)",
            params! {
                "volume_in_size" => self.volume_in_size,
                "quantity" => self.quantity,
                "description" => &self.description,
                "org_id" => org.id.to_string(),
            },
        )?;

        Ok(())
    }

    pub fn update_stock(
        &mut self,
        org: &Organization,
        volume_in_size: i64,
        quantity: i64,
    ) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "UPDATE Stock SET volume_in_size = :volume_in_size, quantity = :quantity WHERE org_id = :org_id AND description = :description",
            params! {
                "volume_in_size" => volume_in_size,
                "quantity" => quantity,
                "description" => &self.description,
                "org_id" => org.id.to_string(),
            },
        )?;

        self.volume_in_size = volume_in_size;
        self.quantity = quantity;
        Ok(())
    }

    pub fn remove_stock(&self, org: &Organization) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;

        conn.exec_drop(
            "DELETE FROM Stock WHERE org_id = :org_id AND description = :description",
            params! {
                "description" => &self.description,
                "org_id" => org.id.to_string(),
            },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::test_support::reset_database;
    use serial_test::serial;

    #[test]
    #[serial(db)]
    fn test_add_new_stock() {
        reset_database();
        let org = Organization::create_organization("Stock Warehouse Org", "Sector 18, Logistics Hub")
            .expect("Failed to create organization for stock test");

        let stock = Stock::new(100, 500, "Electronic Components");
        stock.add_new_stock(&org).expect("Failed to add stock to organization in database");

        // Verify stock record in MySQL database
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for stock verification");

        let row: Option<(i64, i64, String, String)> = conn
            .exec_first(
                "SELECT volume_in_size, quantity, description, org_id FROM Stock WHERE org_id = :org_id AND description = :desc",
                params! {
                    "org_id" => org.id.to_string(),
                    "desc" => &stock.description,
                },
            )
            .expect("Failed to query database for stock");

        assert!(row.is_some(), "Stock record not found in database");
        let (db_volume, db_quantity, db_desc, db_org_id) = row.unwrap();
        assert_eq!(db_volume, stock.volume_in_size);
        assert_eq!(db_quantity, stock.quantity);
        assert_eq!(db_desc, stock.description);
        assert_eq!(db_org_id, org.id.to_string());
    }

    #[test]
    #[serial(db)]
    fn test_update_stock() {
        reset_database();
        let org = Organization::create_organization("Update Stock Org", "Zone A, Warehouse 2")
            .expect("Failed to create organization for update stock test");

        let mut stock = Stock::new(50, 200, "Raw Aluminum Sheets");
        stock.add_new_stock(&org).expect("Failed to add stock to organization");

        let update_res = stock.update_stock(&org, 120, 800);
        assert!(update_res.is_ok(), "Failed to update stock");
        assert_eq!(stock.volume_in_size, 120);
        assert_eq!(stock.quantity, 800);

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for stock update verification");

        let row: Option<(i64, i64)> = conn
            .exec_first(
                "SELECT volume_in_size, quantity FROM Stock WHERE org_id = :org_id AND description = :desc",
                params! {
                    "org_id" => org.id.to_string(),
                    "desc" => &stock.description,
                },
            )
            .expect("Failed to query database for updated stock");

        assert!(row.is_some(), "Updated stock record not found in database");
        let (db_volume, db_quantity) = row.unwrap();
        assert_eq!(db_volume, 120);
        assert_eq!(db_quantity, 800);
    }

    #[test]
    #[serial(db)]
    fn test_remove_stock() {
        reset_database();
        let org = Organization::create_organization("Remove Stock Org", "Zone B, Cargo Hub")
            .expect("Failed to create organization for remove stock test");

        let stock = Stock::new(30, 150, "Steel Rods");
        stock.add_new_stock(&org).expect("Failed to add stock to organization");

        let remove_res = stock.remove_stock(&org);
        assert!(remove_res.is_ok(), "Failed to remove stock");

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection
            .get_connection()
            .expect("Failed to connect to database for stock removal verification");

        let row: Option<(i64, i64, String, String)> = conn
            .exec_first(
                "SELECT volume_in_size, quantity, description, org_id FROM Stock WHERE org_id = :org_id AND description = :desc",
                params! {
                    "org_id" => org.id.to_string(),
                    "desc" => &stock.description,
                },
            )
            .expect("Failed to query database for removed stock");

        assert!(row.is_none(), "Stock record should be deleted from database");
    }
}
