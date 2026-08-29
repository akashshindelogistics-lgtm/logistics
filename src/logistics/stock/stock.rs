use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

/// A stock item held in a godown. Identified within a godown by its
/// `description`.
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

    /// Create the `Stock` table if it does not exist. Kept in sync with
    /// `test_support::migrate` — stock now references a godown, not an org.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS Stock (
                id INT AUTO_INCREMENT PRIMARY KEY,
                volume_in_size BIGINT NOT NULL,
                quantity BIGINT NOT NULL,
                description VARCHAR(255) NOT NULL,
                godown_id VARCHAR(36) NOT NULL,
                CONSTRAINT fk_stock_godown FOREIGN KEY (godown_id) REFERENCES Godowns(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    /// Load every stock item in a godown. Takes an existing connection so
    /// callers building a `Godown` don't open a second one.
    pub fn list_by_godown(
        conn: &mut mysql::PooledConn,
        godown_id: Uuid,
    ) -> Result<Vec<Self>, Box<dyn Error>> {
        Self::ensure_table(conn)?;
        let rows: Vec<(i64, i64, String)> = conn.exec_map(
            "SELECT volume_in_size, quantity, description FROM Stock WHERE godown_id = :godown_id ORDER BY description",
            params! { "godown_id" => godown_id.to_string() },
            |(vol, qty, desc)| (vol, qty, desc),
        )?;
        Ok(rows
            .into_iter()
            .map(|(volume_in_size, quantity, description)| Stock {
                volume_in_size,
                quantity,
                description,
            })
            .collect())
    }

    pub fn add_to_godown(&self, godown_id: Uuid) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        conn.exec_drop(
            "INSERT INTO Stock (volume_in_size, quantity, description, godown_id)
             VALUES (:volume_in_size, :quantity, :description, :godown_id)",
            params! {
                "volume_in_size" => self.volume_in_size,
                "quantity" => self.quantity,
                "description" => &self.description,
                "godown_id" => godown_id.to_string(),
            },
        )?;
        Ok(())
    }

    pub fn update_in_godown(
        &mut self,
        godown_id: Uuid,
        volume_in_size: i64,
        quantity: i64,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;

        conn.exec_drop(
            "UPDATE Stock SET volume_in_size = :volume_in_size, quantity = :quantity WHERE godown_id = :godown_id AND description = :description",
            params! {
                "volume_in_size" => volume_in_size,
                "quantity" => quantity,
                "description" => &self.description,
                "godown_id" => godown_id.to_string(),
            },
        )?;

        self.volume_in_size = volume_in_size;
        self.quantity = quantity;
        Ok(())
    }

    pub fn remove_from_godown(&self, godown_id: Uuid) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()?;

        conn.exec_drop(
            "DELETE FROM Stock WHERE godown_id = :godown_id AND description = :description",
            params! {
                "description" => &self.description,
                "godown_id" => godown_id.to_string(),
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::godown::godown::Godown;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::reset_database;
    use serial_test::serial;

    fn make_godown() -> Godown {
        let org = Organization::create_organization("Stock Warehouse Org", "Sector 18, Logistics Hub")
            .expect("Failed to create organization for stock test");
        Godown::create(org.id, "Main Godown", "Dock 1").expect("Failed to create godown")
    }

    #[test]
    #[serial(db)]
    fn test_add_new_stock() {
        reset_database();
        let godown = make_godown();

        let stock = Stock::new(100, 500, "Electronic Components");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()
            .expect("Failed to connect to database for stock verification");

        let row: Option<(i64, i64, String, String)> = conn
            .exec_first(
                "SELECT volume_in_size, quantity, description, godown_id FROM Stock WHERE godown_id = :godown_id AND description = :desc",
                params! {
                    "godown_id" => godown.id.to_string(),
                    "desc" => &stock.description,
                },
            )
            .expect("Failed to query database for stock");

        assert!(row.is_some(), "Stock record not found in database");
        let (db_volume, db_quantity, db_desc, db_godown_id) = row.unwrap();
        assert_eq!(db_volume, stock.volume_in_size);
        assert_eq!(db_quantity, stock.quantity);
        assert_eq!(db_desc, stock.description);
        assert_eq!(db_godown_id, godown.id.to_string());
    }

    #[test]
    #[serial(db)]
    fn test_update_stock() {
        reset_database();
        let godown = make_godown();

        let mut stock = Stock::new(50, 200, "Raw Aluminum Sheets");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        let update_res = stock.update_in_godown(godown.id, 120, 800);
        assert!(update_res.is_ok(), "Failed to update stock");
        assert_eq!(stock.volume_in_size, 120);
        assert_eq!(stock.quantity, 800);

        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()
            .expect("Failed to connect to database for stock update verification");

        let row: Option<(i64, i64)> = conn
            .exec_first(
                "SELECT volume_in_size, quantity FROM Stock WHERE godown_id = :godown_id AND description = :desc",
                params! {
                    "godown_id" => godown.id.to_string(),
                    "desc" => &stock.description,
                },
            )
            .expect("Failed to query database for updated stock");

        assert_eq!(row, Some((120, 800)));
    }

    #[test]
    #[serial(db)]
    fn test_remove_stock() {
        reset_database();
        let godown = make_godown();

        let stock = Stock::new(30, 150, "Steel Rods");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        stock.remove_from_godown(godown.id).expect("Failed to remove stock");

        let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
            .get_connection()
            .expect("Failed to connect to database for stock removal verification");

        let row: Option<(i64,)> = conn
            .exec_first(
                "SELECT quantity FROM Stock WHERE godown_id = :godown_id AND description = :desc",
                params! {
                    "godown_id" => godown.id.to_string(),
                    "desc" => &stock.description,
                },
            )
            .expect("Failed to query database for removed stock");

        assert!(row.is_none(), "Stock record should be deleted from database");
    }
}
