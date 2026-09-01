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
    /// Reorder point: when `quantity` falls below this, [`Stock::below_threshold`]
    /// is set so the godown can flag the item for restocking. `None` disables
    /// the check.
    #[serde(default)]
    pub reorder_threshold: Option<i64>,
    /// `true` when `reorder_threshold` is set and `quantity` is under it.
    /// Server-computed on every read and on `with_reorder_threshold` /
    /// `update_in_godown`; there is no request path that lets a client set it.
    #[serde(default)]
    pub below_threshold: bool,
}

impl Stock {
    pub fn new(volume_in_size: i64, quantity: i64, description: impl Into<String>) -> Self {
        Stock {
            volume_in_size,
            quantity,
            description: description.into(),
            reorder_threshold: None,
            below_threshold: false,
        }
    }

    /// Set a reorder threshold, recomputing [`Stock::below_threshold`].
    pub fn with_reorder_threshold(mut self, reorder_threshold: Option<i64>) -> Self {
        self.reorder_threshold = reorder_threshold;
        self.below_threshold = Self::is_below(self.quantity, reorder_threshold);
        self
    }

    /// Whether `quantity` is under `reorder_threshold` (always `false` when
    /// the threshold is unset).
    fn is_below(quantity: i64, reorder_threshold: Option<i64>) -> bool {
        reorder_threshold.is_some_and(|t| quantity < t)
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
                reorder_threshold BIGINT DEFAULT NULL,
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
        let rows: Vec<(i64, i64, String, Option<i64>)> = conn.exec_map(
            "SELECT volume_in_size, quantity, description, reorder_threshold FROM Stock WHERE godown_id = :godown_id ORDER BY description",
            params! { "godown_id" => godown_id.to_string() },
            |(vol, qty, desc, threshold)| (vol, qty, desc, threshold),
        )?;
        Ok(rows
            .into_iter()
            .map(|(volume_in_size, quantity, description, reorder_threshold)| Stock {
                volume_in_size,
                quantity,
                description,
                reorder_threshold,
                below_threshold: Self::is_below(quantity, reorder_threshold),
            })
            .collect())
    }

    pub fn add_to_godown(&self, godown_id: Uuid) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;
        Self::ensure_table(&mut conn)?;

        conn.exec_drop(
            "INSERT INTO Stock (volume_in_size, quantity, description, reorder_threshold, godown_id)
             VALUES (:volume_in_size, :quantity, :description, :reorder_threshold, :godown_id)",
            params! {
                "volume_in_size" => self.volume_in_size,
                "quantity" => self.quantity,
                "description" => &self.description,
                "reorder_threshold" => self.reorder_threshold,
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
        reorder_threshold: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;

        conn.exec_drop(
            "UPDATE Stock SET volume_in_size = :volume_in_size, quantity = :quantity, reorder_threshold = :reorder_threshold WHERE godown_id = :godown_id AND description = :description",
            params! {
                "volume_in_size" => volume_in_size,
                "quantity" => quantity,
                "reorder_threshold" => reorder_threshold,
                "description" => &self.description,
                "godown_id" => godown_id.to_string(),
            },
        )?;

        self.volume_in_size = volume_in_size;
        self.quantity = quantity;
        self.reorder_threshold = reorder_threshold;
        self.below_threshold = Self::is_below(quantity, reorder_threshold);
        Ok(())
    }

    pub fn remove_from_godown(&self, godown_id: Uuid) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
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
    use crate::logistics::test_support::TestDb;

    fn make_godown() -> Godown {
        let org = Organization::create_organization("Stock Warehouse Org", "Sector 18, Logistics Hub")
            .expect("Failed to create organization for stock test");
        Godown::create(org.id, "Main Godown", "Dock 1", None).expect("Failed to create godown")
    }

    #[test]
    fn test_add_new_stock() {
        let _db = TestDb::create();
        let godown = make_godown();

        let stock = Stock::new(100, 500, "Electronic Components");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        let mut conn = DbConnection::from_env()
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
    fn test_update_stock() {
        let _db = TestDb::create();
        let godown = make_godown();

        let mut stock = Stock::new(50, 200, "Raw Aluminum Sheets");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        let update_res = stock.update_in_godown(godown.id, 120, 800, None);
        assert!(update_res.is_ok(), "Failed to update stock");
        assert_eq!(stock.volume_in_size, 120);
        assert_eq!(stock.quantity, 800);

        let mut conn = DbConnection::from_env()
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
    fn test_remove_stock() {
        let _db = TestDb::create();
        let godown = make_godown();

        let stock = Stock::new(30, 150, "Steel Rods");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        stock.remove_from_godown(godown.id).expect("Failed to remove stock");

        let mut conn = DbConnection::from_env()
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

    #[test]
    fn test_reorder_threshold_sets_below_flag_on_read() {
        let _db = TestDb::create();
        let godown = make_godown();

        // 40 on hand, reorder point 100 -> below threshold.
        Stock::new(5, 40, "Packing Tape")
            .with_reorder_threshold(Some(100))
            .add_to_godown(godown.id)
            .expect("add low stock");
        // 500 on hand, reorder point 100 -> not below.
        Stock::new(5, 500, "Shrink Wrap")
            .with_reorder_threshold(Some(100))
            .add_to_godown(godown.id)
            .expect("add healthy stock");
        // No threshold -> never flagged.
        Stock::new(5, 1, "Misc")
            .add_to_godown(godown.id)
            .expect("add unthresholded stock");

        let mut conn = DbConnection::from_env().get_connection().expect("connect");
        let loaded = Stock::list_by_godown(&mut conn, godown.id).expect("list stock");

        let tape = loaded.iter().find(|s| s.description == "Packing Tape").unwrap();
        assert_eq!(tape.reorder_threshold, Some(100));
        assert!(tape.below_threshold, "40 < 100 should flag the item");

        let wrap = loaded.iter().find(|s| s.description == "Shrink Wrap").unwrap();
        assert!(!wrap.below_threshold, "500 >= 100 should not flag the item");

        let misc = loaded.iter().find(|s| s.description == "Misc").unwrap();
        assert_eq!(misc.reorder_threshold, None);
        assert!(!misc.below_threshold);
    }

    #[test]
    fn test_update_recomputes_below_threshold() {
        let _db = TestDb::create();
        let godown = make_godown();

        let mut stock = Stock::new(5, 200, "Bolts");
        stock.add_to_godown(godown.id).expect("add");

        // Drop quantity under a freshly set threshold.
        stock
            .update_in_godown(godown.id, 5, 30, Some(50))
            .expect("update");
        assert!(stock.below_threshold);
        assert_eq!(stock.reorder_threshold, Some(50));

        // Clearing the threshold clears the flag.
        stock
            .update_in_godown(godown.id, 5, 30, None)
            .expect("update again");
        assert!(!stock.below_threshold);

        let mut conn = DbConnection::from_env().get_connection().expect("connect");
        let reloaded = Stock::list_by_godown(&mut conn, godown.id).expect("list");
        let bolts = reloaded.iter().find(|s| s.description == "Bolts").unwrap();
        assert_eq!(bolts.reorder_threshold, None);
        assert!(!bolts.below_threshold);
    }
}
