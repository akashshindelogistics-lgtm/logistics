use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn ensure_dispatches_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        "CREATE TABLE IF NOT EXISTS Dispatches (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            customer_id VARCHAR(36) NOT NULL,
            vehicle_registration_number VARCHAR(255) NOT NULL,
            stock_description VARCHAR(255) NOT NULL,
            quantity BIGINT NOT NULL,
            status VARCHAR(50) NOT NULL,
            dispatched_at BIGINT NOT NULL
        )",
        (),
    )?;
    Ok(())
}

/// Kept in sync with `test_support::migrate`.
fn ensure_status_history_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        "CREATE TABLE IF NOT EXISTS DispatchStatusHistory (
            id INT AUTO_INCREMENT PRIMARY KEY,
            dispatch_id VARCHAR(36) NOT NULL,
            status VARCHAR(50) NOT NULL,
            changed_at BIGINT NOT NULL,
            CONSTRAINT fk_dispatch_status_history_dispatch
                FOREIGN KEY (dispatch_id) REFERENCES Dispatches(id) ON DELETE CASCADE
        )",
        (),
    )?;
    Ok(())
}

/// Kept in sync with `test_support::migrate`. One row per dispatch — a
/// dispatch can only reach `DELIVERED` once (it's terminal), so this is
/// never updated, only inserted.
fn ensure_proof_of_delivery_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        "CREATE TABLE IF NOT EXISTS DispatchProofOfDelivery (
            dispatch_id VARCHAR(36) PRIMARY KEY,
            receiver_name VARCHAR(255) NOT NULL,
            signature_or_photo_url TEXT NOT NULL,
            delivered_at BIGINT NOT NULL,
            CONSTRAINT fk_dispatch_pod_dispatch
                FOREIGN KEY (dispatch_id) REFERENCES Dispatches(id) ON DELETE CASCADE
        )",
        (),
    )?;
    Ok(())
}

/// Create every dispatch-related table if it does not exist. Useful for
/// callers that query `Dispatches` before any dispatch has ever been saved
/// (e.g. the vehicle-availability check in
/// [`Organization::dispatch_stock_to_customer`](crate::logistics::orgs::orgs::Organization::dispatch_stock_to_customer)).
pub fn ensure_tables(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    ensure_dispatches_table(conn)?;
    ensure_status_history_table(conn)?;
    ensure_proof_of_delivery_table(conn)?;
    Ok(())
}

/// A dispatch's place in its lifecycle, from order creation to a final
/// outcome. Modeled as a linear-with-branches state machine (see
/// [`Self::can_transition_to`]) rather than a free-form string so an invalid
/// jump (e.g. `PENDING` straight to `DELIVERED`) is rejected instead of
/// silently accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DispatchStatus {
    /// Order recorded, stock reserved and a vehicle selected — nothing has
    /// physically moved yet.
    Pending,
    /// Warehouse/ops has confirmed the order will be fulfilled as booked.
    Confirmed,
    /// Stock has been physically loaded onto the assigned vehicle.
    Loaded,
    /// The vehicle has left for the delivery address.
    InTransit,
    /// Delivered to the customer. Terminal. [`DispatchOrder::transition_to`]
    /// requires a [`ProofOfDeliveryInput`] to reach this status.
    Delivered,
    /// Sent out but came back undelivered (e.g. customer refused, address
    /// unreachable). Terminal. No proof-of-delivery requirement — nothing
    /// was handed over.
    Returned,
    /// Called off before it left the warehouse. Terminal.
    Cancelled,
}

impl DispatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Confirmed => "CONFIRMED",
            Self::Loaded => "LOADED",
            Self::InTransit => "IN_TRANSIT",
            Self::Delivered => "DELIVERED",
            Self::Returned => "RETURNED",
            Self::Cancelled => "CANCELLED",
        }
    }

    /// A terminal status has no further transitions.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Delivered | Self::Returned | Self::Cancelled)
    }

    /// Whether moving from `self` directly to `next` is a legal transition.
    ///
    /// ```text
    /// PENDING -> CONFIRMED -> LOADED -> IN_TRANSIT -> DELIVERED
    ///    |           |          |            \-----> RETURNED
    ///     \-----------\----------\-> CANCELLED
    /// ```
    ///
    /// `RETURNED` is reachable only from `IN_TRANSIT` (a delivery attempt
    /// that didn't land); `CANCELLED` is reachable from any pre-transit
    /// state but not once the vehicle is already out. No transition is
    /// legal out of a terminal status.
    pub fn can_transition_to(&self, next: DispatchStatus) -> bool {
        use DispatchStatus::*;
        matches!(
            (self, next),
            (Pending, Confirmed)
                | (Confirmed, Loaded)
                | (Loaded, InTransit)
                | (InTransit, Delivered)
                | (InTransit, Returned)
                | (Pending, Cancelled)
                | (Confirmed, Cancelled)
                | (Loaded, Cancelled)
        )
    }
}

impl fmt::Display for DispatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DispatchStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PENDING" => Ok(Self::Pending),
            "CONFIRMED" => Ok(Self::Confirmed),
            "LOADED" => Ok(Self::Loaded),
            "IN_TRANSIT" => Ok(Self::InTransit),
            "DELIVERED" => Ok(Self::Delivered),
            "RETURNED" => Ok(Self::Returned),
            "CANCELLED" => Ok(Self::Cancelled),
            other => Err(format!("Unknown dispatch status: {other}")),
        }
    }
}

/// One entry in a dispatch's status history: the status it moved to, and
/// when. The first entry (written by [`DispatchOrder::save`]) records
/// creation at [`DispatchOrder::Pending`]; every later entry comes from
/// [`DispatchOrder::transition_to`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DispatchStatusEvent {
    pub status: DispatchStatus,
    pub changed_at: i64,
}

impl DispatchStatusEvent {
    fn list_by_dispatch(
        conn: &mut mysql::PooledConn,
        dispatch_id: Uuid,
    ) -> Result<Vec<Self>, Box<dyn Error>> {
        let rows: Vec<(String, i64)> = conn.exec_map(
            "SELECT status, changed_at FROM DispatchStatusHistory
             WHERE dispatch_id = :dispatch_id ORDER BY changed_at ASC, id ASC",
            params! { "dispatch_id" => dispatch_id.to_string() },
            |(status, changed_at)| (status, changed_at),
        )?;

        Ok(rows
            .into_iter()
            .map(|(status, changed_at)| DispatchStatusEvent {
                status: status.parse().unwrap_or(DispatchStatus::Pending),
                changed_at,
            })
            .collect())
    }
}

/// Evidence a dispatch was actually handed over, recorded when it's marked
/// [`DispatchStatus::Delivered`]. Written once (a dispatch can only reach
/// `DELIVERED` once) by [`DispatchOrder::transition_to`], never updated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ProofOfDelivery {
    pub receiver_name: String,
    /// URL or data URI for a signature image or delivery photo. Free-form
    /// text column — there is no file-upload/storage backing this yet, so
    /// it's on the caller to host the image and pass a link, or inline a
    /// small `data:` URI.
    pub signature_or_photo_url: String,
    /// Stamped by `transition_to` at the moment of the `DELIVERED`
    /// transition (same instant as that status-history entry), not
    /// supplied by the caller.
    pub delivered_at: i64,
}

impl ProofOfDelivery {
    fn get_by_dispatch(
        conn: &mut mysql::PooledConn,
        dispatch_id: Uuid,
    ) -> Result<Option<Self>, Box<dyn Error>> {
        let row: Option<(String, String, i64)> = conn.exec_first(
            "SELECT receiver_name, signature_or_photo_url, delivered_at
             FROM DispatchProofOfDelivery WHERE dispatch_id = :dispatch_id",
            params! { "dispatch_id" => dispatch_id.to_string() },
        )?;

        Ok(row.map(
            |(receiver_name, signature_or_photo_url, delivered_at)| ProofOfDelivery {
                receiver_name,
                signature_or_photo_url,
                delivered_at,
            },
        ))
    }
}

/// The receiver-supplied half of a [`ProofOfDelivery`] — everything a caller
/// of [`DispatchOrder::transition_to`] provides; `delivered_at` is stamped
/// by `transition_to` itself, not part of the input.
#[derive(Debug, Clone)]
pub struct ProofOfDeliveryInput {
    pub receiver_name: String,
    pub signature_or_photo_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DispatchOrder {
    pub id: Uuid,
    pub org_id: Uuid,
    pub customer_id: Uuid,
    pub vehicle_registration_number: String,
    pub stock_description: String,
    pub quantity: i64,
    pub status: DispatchStatus,
    pub dispatched_at: i64,
    /// Every status this dispatch has passed through, oldest first.
    /// Populated by [`Self::get_by_id`] / [`Self::list_by_org`] /
    /// [`Self::list_all`]; empty on a value that hasn't been saved yet.
    #[serde(default)]
    pub status_history: Vec<DispatchStatusEvent>,
    /// Set once the dispatch is marked [`DispatchStatus::Delivered`] via
    /// [`Self::transition_to`]. Populated by [`Self::get_by_id`] /
    /// [`Self::list_by_org`] / [`Self::list_all`].
    #[serde(default)]
    pub proof_of_delivery: Option<ProofOfDelivery>,
}

impl DispatchOrder {
    /// Persist a newly created dispatch and record its first status-history
    /// entry (`self.status` at `self.dispatched_at`), appending it to
    /// `self.status_history` so the returned value matches what a
    /// subsequent [`Self::get_by_id`] would return without a re-fetch.
    pub fn save(&mut self) -> Result<(), Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;
        ensure_status_history_table(&mut conn)?;
        ensure_proof_of_delivery_table(&mut conn)?;

        conn.exec_drop(
            "INSERT INTO Dispatches (id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at)
             VALUES (:id, :org_id, :customer_id, :vehicle_registration_number, :stock_description, :quantity, :status, :dispatched_at)",
            params! {
                "id" => self.id.to_string(),
                "org_id" => self.org_id.to_string(),
                "customer_id" => self.customer_id.to_string(),
                "vehicle_registration_number" => &self.vehicle_registration_number,
                "stock_description" => &self.stock_description,
                "quantity" => self.quantity,
                "status" => self.status.as_str(),
                "dispatched_at" => self.dispatched_at,
            },
        )?;

        conn.exec_drop(
            "INSERT INTO DispatchStatusHistory (dispatch_id, status, changed_at)
             VALUES (:dispatch_id, :status, :changed_at)",
            params! {
                "dispatch_id" => self.id.to_string(),
                "status" => self.status.as_str(),
                "changed_at" => self.dispatched_at,
            },
        )?;

        self.status_history.push(DispatchStatusEvent {
            status: self.status,
            changed_at: self.dispatched_at,
        });

        Ok(())
    }

    /// Move this dispatch to `next`, validating against the lifecycle state
    /// machine ([`DispatchStatus::can_transition_to`]). Moving to
    /// [`DispatchStatus::Delivered`] additionally requires `proof_of_delivery`
    /// — pass `None` for every other status.
    ///
    /// On success, updates `status` on the `Dispatches` row, appends a
    /// `DispatchStatusHistory` entry timestamped now (and, for a delivery, a
    /// `DispatchProofOfDelivery` row stamped with that same timestamp), and
    /// updates `self.status` / `self.status_history` /
    /// `self.proof_of_delivery` in place so the caller doesn't need to
    /// re-fetch. On an illegal transition or a missing delivery proof,
    /// returns an `Err` describing why and leaves `self` untouched.
    pub fn transition_to(
        &mut self,
        next: DispatchStatus,
        proof_of_delivery: Option<ProofOfDeliveryInput>,
    ) -> Result<(), Box<dyn Error>> {
        if !self.status.can_transition_to(next) {
            return Err(format!(
                "Cannot transition dispatch from {} to {}",
                self.status, next
            )
            .into());
        }

        if next == DispatchStatus::Delivered && proof_of_delivery.is_none() {
            return Err(
                "Proof of delivery (receiver name and a signature or photo) is required to mark a dispatch DELIVERED"
                    .into(),
            );
        }

        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;
        ensure_status_history_table(&mut conn)?;
        ensure_proof_of_delivery_table(&mut conn)?;

        let changed_at = now_unix();

        conn.exec_drop(
            "UPDATE Dispatches SET status = :status WHERE id = :id",
            params! {
                "status" => next.as_str(),
                "id" => self.id.to_string(),
            },
        )?;

        conn.exec_drop(
            "INSERT INTO DispatchStatusHistory (dispatch_id, status, changed_at)
             VALUES (:dispatch_id, :status, :changed_at)",
            params! {
                "dispatch_id" => self.id.to_string(),
                "status" => next.as_str(),
                "changed_at" => changed_at,
            },
        )?;

        if let Some(proof) = proof_of_delivery {
            conn.exec_drop(
                "INSERT INTO DispatchProofOfDelivery (dispatch_id, receiver_name, signature_or_photo_url, delivered_at)
                 VALUES (:dispatch_id, :receiver_name, :signature_or_photo_url, :delivered_at)",
                params! {
                    "dispatch_id" => self.id.to_string(),
                    "receiver_name" => &proof.receiver_name,
                    "signature_or_photo_url" => &proof.signature_or_photo_url,
                    "delivered_at" => changed_at,
                },
            )?;
            self.proof_of_delivery = Some(ProofOfDelivery {
                receiver_name: proof.receiver_name,
                signature_or_photo_url: proof.signature_or_photo_url,
                delivered_at: changed_at,
            });
        }

        self.status = next;
        self.status_history.push(DispatchStatusEvent {
            status: next,
            changed_at,
        });

        Ok(())
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;
        ensure_status_history_table(&mut conn)?;
        ensure_proof_of_delivery_table(&mut conn)?;

        let row: Option<(String, String, String, String, String, i64, String, i64)> = conn
            .exec_first(
                "SELECT id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at FROM Dispatches WHERE id = :id",
                params! { "id" => id.to_string() },
            )?;

        let Some(row) = row else {
            return Ok(None);
        };
        let history = DispatchStatusEvent::list_by_dispatch(&mut conn, id)?;
        let proof = ProofOfDelivery::get_by_dispatch(&mut conn, id)?;
        Ok(Some(Self::row_to_order(row, history, proof)))
    }

    pub fn list_all() -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;
        ensure_status_history_table(&mut conn)?;
        ensure_proof_of_delivery_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, String, i64, String, i64)> = conn.exec_map(
            "SELECT id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at FROM Dispatches",
            (),
            |(id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)| {
                (id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)
            },
        )?;

        Self::rows_to_orders(&mut conn, rows)
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let db_connection = DbConnection::from_env();
        let mut conn = db_connection.get_connection()?;
        ensure_dispatches_table(&mut conn)?;
        ensure_status_history_table(&mut conn)?;
        ensure_proof_of_delivery_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, String, i64, String, i64)> = conn.exec_map(
            "SELECT id, org_id, customer_id, vehicle_registration_number, stock_description, quantity, status, dispatched_at FROM Dispatches WHERE org_id = :org_id",
            params! { "org_id" => org_id.to_string() },
            |(id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)| {
                (id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at)
            },
        )?;

        Self::rows_to_orders(&mut conn, rows)
    }

    fn row_to_order(
        row: (String, String, String, String, String, i64, String, i64),
        status_history: Vec<DispatchStatusEvent>,
        proof_of_delivery: Option<ProofOfDelivery>,
    ) -> Self {
        let (id, org_id, customer_id, vehicle_reg, stock_desc, qty, status, dispatched_at) = row;
        DispatchOrder {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            customer_id: Uuid::parse_str(&customer_id).unwrap_or_else(|_| Uuid::new_v4()),
            vehicle_registration_number: vehicle_reg,
            stock_description: stock_desc,
            quantity: qty,
            status: status.parse().unwrap_or(DispatchStatus::Pending),
            dispatched_at,
            status_history,
            proof_of_delivery,
        }
    }

    /// Fetch each row's status history and proof of delivery (if any) and
    /// assemble the full `DispatchOrder` list. Two extra queries per
    /// dispatch, matching the N+1-per-parent pattern `Godown::list_by_org`
    /// already uses for its stock.
    fn rows_to_orders(
        conn: &mut mysql::PooledConn,
        rows: Vec<(String, String, String, String, String, i64, String, i64)>,
    ) -> Result<Vec<Self>, Box<dyn Error>> {
        rows.into_iter()
            .map(|row| {
                let id = Uuid::parse_str(&row.0).unwrap_or_else(|_| Uuid::new_v4());
                let history = DispatchStatusEvent::list_by_dispatch(conn, id)?;
                let proof = ProofOfDelivery::get_by_dispatch(conn, id)?;
                Ok(Self::row_to_order(row, history, proof))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::test_support::TestDb;
    use DispatchStatus::*;

    fn sample_order(status: DispatchStatus) -> DispatchOrder {
        DispatchOrder {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            vehicle_registration_number: "TEST-001".to_string(),
            stock_description: "Cement".to_string(),
            quantity: 10,
            status,
            dispatched_at: 1_700_000_000,
            status_history: Vec::new(),
            proof_of_delivery: None,
        }
    }

    fn sample_proof() -> ProofOfDeliveryInput {
        ProofOfDeliveryInput {
            receiver_name: "Priya Sharma".to_string(),
            signature_or_photo_url: "https://example.com/pod/sig123.png".to_string(),
        }
    }

    #[test]
    fn test_get_by_id_returns_none_for_nonexistent_dispatch() {
        let _db = TestDb::create();
        let result = DispatchOrder::get_by_id(Uuid::new_v4());
        assert!(
            result.is_ok(),
            "get_by_id should not error for a missing UUID"
        );
        assert!(
            result.unwrap().is_none(),
            "should return None for a UUID that was never saved"
        );
    }

    #[test]
    fn test_list_by_org_returns_empty_for_unknown_org() {
        let _db = TestDb::create();
        let result = DispatchOrder::list_by_org(Uuid::new_v4());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_list_all_succeeds() {
        let _db = TestDb::create();
        let result = DispatchOrder::list_all();
        assert!(
            result.is_ok(),
            "list_all should succeed even when the table is empty"
        );
    }

    #[test]
    fn test_save_records_initial_status_history_entry() {
        let _db = TestDb::create();
        let mut order = sample_order(Pending);
        order.save().expect("save");

        let fetched = DispatchOrder::get_by_id(order.id)
            .expect("get_by_id should succeed")
            .expect("dispatch should exist after save");
        assert_eq!(fetched.status, Pending);
        assert_eq!(fetched.status_history.len(), 1);
        assert_eq!(fetched.status_history[0].status, Pending);
        assert_eq!(fetched.status_history[0].changed_at, 1_700_000_000);
    }

    #[test]
    fn test_transition_to_updates_status_and_appends_history() {
        let _db = TestDb::create();
        let mut order = sample_order(Pending);
        order.save().expect("save");

        order
            .transition_to(Confirmed, None)
            .expect("PENDING -> CONFIRMED is legal");
        assert_eq!(order.status, Confirmed);
        assert_eq!(order.status_history.len(), 2);
        assert_eq!(order.status_history[1].status, Confirmed);

        let fetched = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
        assert_eq!(fetched.status, Confirmed);
        assert_eq!(fetched.status_history.len(), 2);
        assert_eq!(fetched.status_history[0].status, Pending);
        assert_eq!(fetched.status_history[1].status, Confirmed);
    }

    #[test]
    fn test_transition_to_rejects_illegal_jump_and_leaves_status_unchanged() {
        let _db = TestDb::create();
        let mut order = sample_order(Pending);
        order.save().expect("save");

        let result = order.transition_to(Delivered, Some(sample_proof()));
        assert!(result.is_err(), "PENDING -> DELIVERED should be rejected");
        assert_eq!(
            order.status, Pending,
            "status must not change on a rejected transition"
        );
        assert_eq!(
            order.status_history.len(),
            1,
            "only the save()-time entry should be present; no entry for the rejected transition"
        );

        let fetched = DispatchOrder::get_by_id(order.id).unwrap().unwrap();
        assert_eq!(fetched.status, Pending);
        assert_eq!(fetched.status_history.len(), 1);
    }

    #[test]
    fn test_transition_to_rejects_moves_out_of_a_terminal_state() {
        let _db = TestDb::create();
        let mut order = sample_order(Cancelled);
        order.save().expect("save");

        assert!(order.transition_to(Confirmed, None).is_err());
        assert!(order.transition_to(Pending, None).is_err());
        assert_eq!(order.status, Cancelled);
    }

    #[test]
    fn test_full_happy_path_lifecycle() {
        let _db = TestDb::create();
        let mut order = sample_order(Pending);
        order.save().expect("save");

        for next in [Confirmed, Loaded, InTransit, Delivered] {
            let proof = (next == Delivered).then(sample_proof);
            order
                .transition_to(next, proof)
                .unwrap_or_else(|e| panic!("{} should be legal: {e}", next));
        }
        assert_eq!(order.status, Delivered);
        assert!(order.status.is_terminal());
        assert_eq!(order.status_history.len(), 5); // PENDING + 4 transitions
        let proof = order
            .proof_of_delivery
            .as_ref()
            .expect("proof should be set");
        assert_eq!(proof.receiver_name, "Priya Sharma");
    }

    #[test]
    fn dispatch_status_transitions_follow_the_lifecycle() {
        assert!(Pending.can_transition_to(Confirmed));
        assert!(Confirmed.can_transition_to(Loaded));
        assert!(Loaded.can_transition_to(InTransit));
        assert!(InTransit.can_transition_to(Delivered));
        assert!(InTransit.can_transition_to(Returned));
        assert!(Pending.can_transition_to(Cancelled));
        assert!(Confirmed.can_transition_to(Cancelled));
        assert!(Loaded.can_transition_to(Cancelled));

        assert!(!Pending.can_transition_to(Loaded), "can't skip CONFIRMED");
        assert!(
            !Pending.can_transition_to(Delivered),
            "can't skip straight to DELIVERED"
        );
        assert!(
            !InTransit.can_transition_to(Cancelled),
            "can't cancel once it's out"
        );
        assert!(
            !Loaded.can_transition_to(Returned),
            "RETURNED only follows IN_TRANSIT"
        );
        for terminal in [Delivered, Returned, Cancelled] {
            assert!(
                !terminal.can_transition_to(Confirmed),
                "{terminal} is terminal"
            );
        }
    }

    #[test]
    fn dispatch_status_terminal_states() {
        assert!(Delivered.is_terminal());
        assert!(Returned.is_terminal());
        assert!(Cancelled.is_terminal());
        assert!(!Pending.is_terminal());
        assert!(!Confirmed.is_terminal());
        assert!(!Loaded.is_terminal());
        assert!(!InTransit.is_terminal());
    }

    #[test]
    fn dispatch_status_string_round_trips() {
        for s in [
            Pending, Confirmed, Loaded, InTransit, Delivered, Returned, Cancelled,
        ] {
            let parsed: DispatchStatus =
                s.as_str().parse().expect("as_str output should parse back");
            assert_eq!(parsed, s);
        }
    }

    #[test]
    fn dispatch_status_from_str_rejects_unknown_values() {
        assert!("DISPATCHED".parse::<DispatchStatus>().is_err());
        assert!("bogus".parse::<DispatchStatus>().is_err());
    }
}
