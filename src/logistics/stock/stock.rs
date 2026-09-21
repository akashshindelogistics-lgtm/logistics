use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

/// The default category for a stock item that doesn't specify one, so every
/// existing caller/payload that predates categories keeps working.
pub fn default_category() -> String {
    "General".to_string()
}

/// A stock item held in a godown. Identified within a godown by its
/// `description` — `category` is a free-text, org-defined tag layered on
/// top of that identity (e.g. "Cement", "Electronics"), not part of it.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Stock {
    pub volume_in_size: i64,
    pub quantity: i64,
    pub description: String,
    /// Free-text, org-defined category (e.g. "Cement", "Electronics").
    /// Defaults to `"General"` when not supplied.
    #[serde(default = "default_category")]
    pub category: String,
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
            category: default_category(),
            reorder_threshold: None,
            below_threshold: false,
        }
    }

    /// Tag this item with a category, overriding the `"General"` default.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
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
                category VARCHAR(255) NOT NULL DEFAULT 'General',
                reorder_threshold BIGINT DEFAULT NULL,
                godown_id VARCHAR(36) NOT NULL,
                CONSTRAINT fk_stock_godown FOREIGN KEY (godown_id) REFERENCES Godowns(id) ON DELETE CASCADE
            )",
        )?;
        // `category` was added after this table first shipped; back-fill it
        // onto a local/deployed database that predates it. Fresh databases
        // always get it via CREATE TABLE above.
        let has_category: Option<i64> = conn.exec_first(
            "SELECT 1 FROM information_schema.columns
             WHERE table_schema = DATABASE() AND table_name = 'Stock' AND column_name = 'category'",
            (),
        )?;
        if has_category.is_none() {
            conn.query_drop("ALTER TABLE Stock ADD COLUMN category VARCHAR(255) NOT NULL DEFAULT 'General'")?;
        }
        Ok(())
    }

    /// Load every stock item in a godown. Takes an existing connection so
    /// callers building a `Godown` don't open a second one.
    pub fn list_by_godown(
        conn: &mut mysql::PooledConn,
        godown_id: Uuid,
    ) -> Result<Vec<Self>, Box<dyn Error>> {
        Self::ensure_table(conn)?;
        let rows: Vec<(i64, i64, String, String, Option<i64>)> = conn.exec_map(
            "SELECT volume_in_size, quantity, description, category, reorder_threshold FROM Stock WHERE godown_id = :godown_id ORDER BY description",
            params! { "godown_id" => godown_id.to_string() },
            |(vol, qty, desc, category, threshold)| (vol, qty, desc, category, threshold),
        )?;
        Ok(rows
            .into_iter()
            .map(|(volume_in_size, quantity, description, category, reorder_threshold)| Stock {
                volume_in_size,
                quantity,
                description,
                category,
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
            "INSERT INTO Stock (volume_in_size, quantity, description, category, reorder_threshold, godown_id)
             VALUES (:volume_in_size, :quantity, :description, :category, :reorder_threshold, :godown_id)",
            params! {
                "volume_in_size" => self.volume_in_size,
                "quantity" => self.quantity,
                "description" => &self.description,
                "category" => &self.category,
                "reorder_threshold" => self.reorder_threshold,
                "godown_id" => godown_id.to_string(),
            },
        )?;
        crate::logistics::ai::chunk::reindex_stock_by_description_best_effort(
            godown_id,
            &self.description,
        );
        Ok(())
    }

    pub fn update_in_godown(
        &mut self,
        godown_id: Uuid,
        volume_in_size: i64,
        quantity: i64,
        category: impl Into<String>,
        reorder_threshold: Option<i64>,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env()
            .get_connection()?;
        let category = category.into();

        conn.exec_drop(
            "UPDATE Stock SET volume_in_size = :volume_in_size, quantity = :quantity, category = :category, reorder_threshold = :reorder_threshold WHERE godown_id = :godown_id AND description = :description",
            params! {
                "volume_in_size" => volume_in_size,
                "quantity" => quantity,
                "category" => &category,
                "reorder_threshold" => reorder_threshold,
                "description" => &self.description,
                "godown_id" => godown_id.to_string(),
            },
        )?;

        self.volume_in_size = volume_in_size;
        self.quantity = quantity;
        self.category = category;
        self.reorder_threshold = reorder_threshold;
        self.below_threshold = Self::is_below(quantity, reorder_threshold);
        crate::logistics::ai::chunk::reindex_stock_by_description_best_effort(
            godown_id,
            &self.description,
        );
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
        crate::logistics::ai::chunk::reindex_stock_by_description_best_effort(
            godown_id,
            &self.description,
        );
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
    fn test_add_stock_defaults_category_to_general() {
        let _db = TestDb::create();
        let godown = make_godown();

        Stock::new(10, 20, "Unlabeled Widget").add_to_godown(godown.id).expect("add");

        let mut conn = DbConnection::from_env().get_connection().expect("connect");
        let loaded = Stock::list_by_godown(&mut conn, godown.id).expect("list");
        let widget = loaded.iter().find(|s| s.description == "Unlabeled Widget").unwrap();
        assert_eq!(widget.category, "General");
    }

    #[test]
    fn test_add_stock_with_an_explicit_category() {
        let _db = TestDb::create();
        let godown = make_godown();

        Stock::new(10, 20, "Server Rack")
            .with_category("Electronics")
            .add_to_godown(godown.id)
            .expect("add");

        let mut conn = DbConnection::from_env().get_connection().expect("connect");
        let loaded = Stock::list_by_godown(&mut conn, godown.id).expect("list");
        let rack = loaded.iter().find(|s| s.description == "Server Rack").unwrap();
        assert_eq!(rack.category, "Electronics");
    }

    #[test]
    fn test_update_stock() {
        let _db = TestDb::create();
        let godown = make_godown();

        let mut stock = Stock::new(50, 200, "Raw Aluminum Sheets");
        stock.add_to_godown(godown.id).expect("Failed to add stock to godown");

        let update_res = stock.update_in_godown(godown.id, 120, 800, "Metals", None);
        assert!(update_res.is_ok(), "Failed to update stock");
        assert_eq!(stock.volume_in_size, 120);
        assert_eq!(stock.quantity, 800);
        assert_eq!(stock.category, "Metals");

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
            .update_in_godown(godown.id, 5, 30, "General", Some(50))
            .expect("update");
        assert!(stock.below_threshold);
        assert_eq!(stock.reorder_threshold, Some(50));

        // Clearing the threshold clears the flag.
        stock
            .update_in_godown(godown.id, 5, 30, "General", None)
            .expect("update again");
        assert!(!stock.below_threshold);

        let mut conn = DbConnection::from_env().get_connection().expect("connect");
        let reloaded = Stock::list_by_godown(&mut conn, godown.id).expect("list");
        let bolts = reloaded.iter().find(|s| s.description == "Bolts").unwrap();
        assert_eq!(bolts.reorder_threshold, None);
        assert!(!bolts.below_threshold);
    }

    #[test]
    fn test_add_update_and_remove_keep_the_assistant_index_in_sync() {
        use crate::logistics::ai::chunk;

        let _db = TestDb::create();
        let godown = make_godown();
        let mut stock = Stock::new(1, 100, "Pallets");
        stock.add_to_godown(godown.id).expect("add");

        let results = chunk::search_by_org(godown.org_id, "Pallets", 8).expect("search");
        assert!(
            results.iter().any(|c| c.text.contains("100 units of Pallets")),
            "adding stock should index it: {results:?}"
        );

        stock.update_in_godown(godown.id, 1, 5, "General", Some(50)).expect("update");
        let results = chunk::search_by_org(godown.org_id, "Pallets", 8).expect("search");
        assert!(
            results.iter().any(|c| c.text.contains("5 units of Pallets")
                && c.text.contains("below its reorder threshold")),
            "updating stock should reindex it with the new quantity: {results:?}"
        );

        stock.remove_from_godown(godown.id).expect("remove");
        let results = chunk::search_by_org(godown.org_id, "Pallets", 8).expect("search");
        assert!(
            !results.iter().any(|c| c.text.contains("Pallets")),
            "removing stock should drop its chunk: {results:?}"
        );
    }
}
