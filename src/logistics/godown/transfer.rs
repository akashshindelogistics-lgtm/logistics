use crate::logistics::db::connection::DbConnection;
use crate::logistics::godown::godown::Godown;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// One completed move of a stock item from one godown to another godown of the
/// same organization. Rows are insert-only — the table is an audit log of every
/// transfer, never updated or deleted except by the `ON DELETE CASCADE` when the
/// owning organization is removed.
///
/// A transfer is distinct from a dispatch: a dispatch sends stock *out* to a
/// customer and decrements the org's holding, whereas a transfer just relocates
/// stock between the org's own warehouses and conserves the total.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StockTransfer {
    pub id: Uuid,
    pub org_id: Uuid,
    pub from_godown_id: Uuid,
    pub to_godown_id: Uuid,
    /// Stock item moved, identified by its description (the same key a godown
    /// uses for its stock).
    pub description: String,
    /// Number of units moved.
    pub quantity: i64,
    /// Per-unit volume of the item at the time of the move, copied from the
    /// source godown's stock so the audit row is self-contained.
    pub volume_in_size: i64,
    /// Unix seconds, stamped server-side when the move committed.
    pub transferred_at: i64,
}

/// Why a [`StockTransfer::execute`] call was rejected. The route layer maps
/// these to HTTP status codes (400 for the request-shape problems, 409 for a
/// destination capacity overflow, 500 for a database failure).
#[derive(Debug)]
pub enum TransferError {
    /// Source and destination are the same godown.
    SameGodown,
    /// The two godowns belong to different organizations.
    DifferentOrg,
    /// `quantity` was zero or negative.
    NonPositiveQuantity,
    /// The source godown holds no stock item with that description.
    ItemNotInSource,
    /// The source godown does not have enough units to move.
    InsufficientQuantity { available: i64, requested: i64 },
    /// The move would push the destination godown over its `max_capacity`.
    DestinationCapacity(String),
    /// An underlying database error.
    Db(Box<dyn Error>),
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferError::SameGodown => {
                write!(f, "Source and destination godown must be different")
            }
            TransferError::DifferentOrg => write!(
                f,
                "Both godowns must belong to the same organization"
            ),
            TransferError::NonPositiveQuantity => {
                write!(f, "Transfer quantity must be greater than zero")
            }
            TransferError::ItemNotInSource => {
                write!(f, "The source godown has no such stock item")
            }
            TransferError::InsufficientQuantity {
                available,
                requested,
            } => write!(
                f,
                "Source godown holds only {available} units, cannot move {requested}"
            ),
            TransferError::DestinationCapacity(msg) => write!(f, "{msg}"),
            TransferError::Db(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TransferError {}

impl From<Box<dyn Error>> for TransferError {
    fn from(err: Box<dyn Error>) -> Self {
        TransferError::Db(err)
    }
}

impl From<mysql::Error> for TransferError {
    fn from(err: mysql::Error) -> Self {
        TransferError::Db(Box::new(err))
    }
}

impl StockTransfer {
    /// Create the `StockTransfers` table if it does not exist. Kept in sync
    /// with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS StockTransfers (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                from_godown_id VARCHAR(36) NOT NULL,
                to_godown_id VARCHAR(36) NOT NULL,
                description VARCHAR(255) NOT NULL,
                quantity BIGINT NOT NULL,
                volume_in_size BIGINT NOT NULL,
                transferred_at BIGINT NOT NULL,
                CONSTRAINT fk_stock_transfer_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    /// Move `quantity` units of `description` from `from` to `to` and record
    /// the move.
    ///
    /// Both godowns must already be loaded with their `stock` populated and
    /// verified to belong to the caller's organization. The steps, in order:
    ///
    /// 1. reject same-godown / cross-org / non-positive-quantity requests,
    /// 2. check the source actually holds enough of the item,
    /// 3. check the destination has room (`Godown::check_capacity_for`),
    /// 4. decrement the source (deleting the row if it hits zero),
    /// 5. add to the destination (a new row, or a bump to the existing one),
    /// 6. insert the audit row.
    ///
    /// Steps 4-6 run as one MySQL transaction so a mid-way failure leaves the
    /// stock untouched.
    pub fn execute(
        from: &Godown,
        to: &Godown,
        description: &str,
        quantity: i64,
    ) -> Result<Self, TransferError> {
        if from.id == to.id {
            return Err(TransferError::SameGodown);
        }
        if from.org_id != to.org_id {
            return Err(TransferError::DifferentOrg);
        }
        if quantity <= 0 {
            return Err(TransferError::NonPositiveQuantity);
        }

        let source_item = from
            .stock
            .iter()
            .find(|s| s.description == description)
            .ok_or(TransferError::ItemNotInSource)?;

        if source_item.quantity < quantity {
            return Err(TransferError::InsufficientQuantity {
                available: source_item.quantity,
                requested: quantity,
            });
        }

        // The item keeps the volume it has at the destination if it already
        // exists there, otherwise it arrives at the source's volume.
        let existing_dest = to.stock.iter().find(|s| s.description == description);
        let effective_volume = existing_dest
            .map(|s| s.volume_in_size)
            .unwrap_or(source_item.volume_in_size);
        let dest_final_qty = existing_dest.map(|s| s.quantity).unwrap_or(0) + quantity;

        to.check_capacity_for(
            effective_volume.saturating_mul(dest_final_qty),
            Some(description),
        )
        .map_err(TransferError::DestinationCapacity)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let transfer = StockTransfer {
            id: Uuid::new_v4(),
            org_id: from.org_id,
            from_godown_id: from.id,
            to_godown_id: to.id,
            description: description.to_string(),
            quantity,
            volume_in_size: source_item.volume_in_size,
            transferred_at: now,
        };

        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let mut tx = conn.start_transaction(TxOpts::default())?;

        // 4. draw down the source
        let source_remaining = source_item.quantity - quantity;
        if source_remaining == 0 {
            tx.exec_drop(
                "DELETE FROM Stock WHERE godown_id = :godown_id AND description = :description",
                params! {
                    "godown_id" => from.id.to_string(),
                    "description" => description,
                },
            )?;
        } else {
            tx.exec_drop(
                "UPDATE Stock SET quantity = :quantity WHERE godown_id = :godown_id AND description = :description",
                params! {
                    "quantity" => source_remaining,
                    "godown_id" => from.id.to_string(),
                    "description" => description,
                },
            )?;
        }

        // 5. top up the destination
        if existing_dest.is_some() {
            tx.exec_drop(
                "UPDATE Stock SET quantity = quantity + :delta WHERE godown_id = :godown_id AND description = :description",
                params! {
                    "delta" => quantity,
                    "godown_id" => to.id.to_string(),
                    "description" => description,
                },
            )?;
        } else {
            tx.exec_drop(
                "INSERT INTO Stock (volume_in_size, quantity, description, reorder_threshold, godown_id)
                 VALUES (:volume_in_size, :quantity, :description, NULL, :godown_id)",
                params! {
                    "volume_in_size" => source_item.volume_in_size,
                    "quantity" => quantity,
                    "description" => description,
                    "godown_id" => to.id.to_string(),
                },
            )?;
        }

        // 6. audit row
        tx.exec_drop(
            "INSERT INTO StockTransfers
                (id, org_id, from_godown_id, to_godown_id, description, quantity, volume_in_size, transferred_at)
             VALUES
                (:id, :org_id, :from_godown_id, :to_godown_id, :description, :quantity, :volume_in_size, :transferred_at)",
            params! {
                "id" => transfer.id.to_string(),
                "org_id" => transfer.org_id.to_string(),
                "from_godown_id" => transfer.from_godown_id.to_string(),
                "to_godown_id" => transfer.to_godown_id.to_string(),
                "description" => &transfer.description,
                "quantity" => transfer.quantity,
                "volume_in_size" => transfer.volume_in_size,
                "transferred_at" => transfer.transferred_at,
            },
        )?;

        tx.commit()?;
        Ok(transfer)
    }

    /// Every transfer recorded for an organization, most recent first.
    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, String, i64, i64, i64)> = conn.exec_map(
            "SELECT id, org_id, from_godown_id, to_godown_id, description, quantity, volume_in_size, transferred_at
             FROM StockTransfers WHERE org_id = :org_id ORDER BY transferred_at DESC, id DESC",
            params! { "org_id" => org_id.to_string() },
            |row| row,
        )?;

        Ok(rows
            .into_iter()
            .map(
                |(id, org_id, from_godown_id, to_godown_id, description, quantity, volume_in_size, transferred_at)| {
                    StockTransfer {
                        id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                        org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
                        from_godown_id: Uuid::parse_str(&from_godown_id)
                            .unwrap_or_else(|_| Uuid::new_v4()),
                        to_godown_id: Uuid::parse_str(&to_godown_id)
                            .unwrap_or_else(|_| Uuid::new_v4()),
                        description,
                        quantity,
                        volume_in_size,
                        transferred_at,
                    }
                },
            )
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::stock::stock::Stock;
    use crate::logistics::test_support::TestDb;

    fn org() -> Organization {
        Organization::create_organization("Transfer Test Org", "1 Warehouse Way").expect("create org")
    }

    /// Reload a godown so its `stock` reflects the latest writes.
    fn reload(id: Uuid) -> Godown {
        Godown::get_by_id(id).expect("get godown").expect("godown exists")
    }

    #[test]
    fn test_execute_moves_stock_and_records_an_audit_row() {
        let _db = TestDb::create();
        let org = org();
        let a = Godown::create(org.id, "A", "Addr A", None).expect("godown a");
        let b = Godown::create(org.id, "B", "Addr B", None).expect("godown b");
        Stock::new(5, 100, "Cement").add_to_godown(a.id).expect("seed stock");

        let transfer = StockTransfer::execute(&reload(a.id), &reload(b.id), "Cement", 40)
            .expect("transfer succeeds");
        assert_eq!(transfer.quantity, 40);
        assert_eq!(transfer.volume_in_size, 5);
        assert_eq!(transfer.from_godown_id, a.id);
        assert_eq!(transfer.to_godown_id, b.id);

        let a_cement = reload(a.id).stock.into_iter().find(|s| s.description == "Cement").unwrap();
        assert_eq!(a_cement.quantity, 60);
        let b_cement = reload(b.id).stock.into_iter().find(|s| s.description == "Cement").unwrap();
        assert_eq!(b_cement.quantity, 40);
        assert_eq!(b_cement.volume_in_size, 5);

        let log = StockTransfer::list_by_org(org.id).expect("list");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].id, transfer.id);
    }

    #[test]
    fn test_execute_merges_into_an_existing_destination_item() {
        let _db = TestDb::create();
        let org = org();
        let a = Godown::create(org.id, "A", "Addr A", None).expect("a");
        let b = Godown::create(org.id, "B", "Addr B", None).expect("b");
        Stock::new(5, 100, "Bolts").add_to_godown(a.id).expect("seed a");
        Stock::new(5, 30, "Bolts").add_to_godown(b.id).expect("seed b");

        StockTransfer::execute(&reload(a.id), &reload(b.id), "Bolts", 100).expect("transfer");

        // Source item is fully drained -> its row is gone.
        assert!(reload(a.id).stock.iter().all(|s| s.description != "Bolts"));
        let b_bolts = reload(b.id).stock.into_iter().find(|s| s.description == "Bolts").unwrap();
        assert_eq!(b_bolts.quantity, 130);
    }

    #[test]
    fn test_execute_rejects_insufficient_quantity_and_leaves_stock_untouched() {
        let _db = TestDb::create();
        let org = org();
        let a = Godown::create(org.id, "A", "Addr A", None).expect("a");
        let b = Godown::create(org.id, "B", "Addr B", None).expect("b");
        Stock::new(5, 10, "Pipes").add_to_godown(a.id).expect("seed");

        let err = StockTransfer::execute(&reload(a.id), &reload(b.id), "Pipes", 25).unwrap_err();
        assert!(matches!(
            err,
            TransferError::InsufficientQuantity { available: 10, requested: 25 }
        ));
        assert_eq!(
            reload(a.id).stock.into_iter().find(|s| s.description == "Pipes").unwrap().quantity,
            10
        );
        assert!(StockTransfer::list_by_org(org.id).expect("list").is_empty());
    }

    #[test]
    fn test_execute_rejects_same_godown_and_missing_item() {
        let _db = TestDb::create();
        let org = org();
        let a = Godown::create(org.id, "A", "Addr A", None).expect("a");
        let b = Godown::create(org.id, "B", "Addr B", None).expect("b");

        assert!(matches!(
            StockTransfer::execute(&reload(a.id), &reload(a.id), "X", 1).unwrap_err(),
            TransferError::SameGodown
        ));
        assert!(matches!(
            StockTransfer::execute(&reload(a.id), &reload(b.id), "Nope", 1).unwrap_err(),
            TransferError::ItemNotInSource
        ));
    }

    #[test]
    fn test_execute_respects_destination_capacity() {
        let _db = TestDb::create();
        let org = org();
        let a = Godown::create(org.id, "A", "Addr A", None).expect("a");
        let b = Godown::create(org.id, "B", "Addr B", Some(100)).expect("b capped at 100");
        Stock::new(10, 50, "Tiles").add_to_godown(a.id).expect("seed"); // vol 10 each

        // Moving 11 units -> 110 volume at the destination, over the cap of 100.
        assert!(matches!(
            StockTransfer::execute(&reload(a.id), &reload(b.id), "Tiles", 11).unwrap_err(),
            TransferError::DestinationCapacity(_)
        ));
        // 10 units -> exactly 100, fits.
        StockTransfer::execute(&reload(a.id), &reload(b.id), "Tiles", 10).expect("fits exactly");
    }

    #[test]
    fn test_execute_rejects_cross_org_transfer() {
        let _db = TestDb::create();
        let org_a = org();
        let org_b = Organization::create_organization("Other Org", "2 Elsewhere").expect("org b");
        let ga = Godown::create(org_a.id, "GA", "A", None).expect("ga");
        let gb = Godown::create(org_b.id, "GB", "B", None).expect("gb");
        Stock::new(1, 5, "Widget").add_to_godown(ga.id).expect("seed");

        assert!(matches!(
            StockTransfer::execute(&reload(ga.id), &reload(gb.id), "Widget", 1).unwrap_err(),
            TransferError::DifferentOrg
        ));
    }

    #[test]
    fn test_deleting_org_cascades_to_transfers() {
        let _db = TestDb::create();
        let org = org();
        let a = Godown::create(org.id, "A", "Addr A", None).expect("a");
        let b = Godown::create(org.id, "B", "Addr B", None).expect("b");
        Stock::new(1, 10, "Sacks").add_to_godown(a.id).expect("seed");
        StockTransfer::execute(&reload(a.id), &reload(b.id), "Sacks", 3).expect("transfer");

        org.remove_organization().expect("remove org");
        assert!(StockTransfer::list_by_org(org.id).expect("list").is_empty());
    }
}
