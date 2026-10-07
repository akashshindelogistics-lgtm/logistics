//! A **multi-stop trip**: one vehicle carrying several customers' orders in a
//! sequence. Created by
//! [`Organization::dispatch_trip_to_customers`](crate::logistics::orgs::orgs::Organization::dispatch_trip_to_customers).
//!
//! A trip is a thin grouping over ordinary [`DispatchOrder`]s — each stop is a
//! full dispatch with its own lifecycle, invoice and proof of delivery, linked
//! by `DispatchOrder::trip_id` and ordered by `DispatchOrder::stop_sequence`.
//! The trip itself stores only which truck and when; its status is *derived*
//! from its stops.

use crate::logistics::db::connection::DbConnection;
use crate::logistics::dispatch::dispatch::{DispatchOrder, DispatchStatus, VehicleSource};
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

/// Where a trip is in its life, worked out from its stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TripStatus {
    /// Every stop is still `PENDING` (or `AWAITING_VEHICLE` on a hired
    /// trip) — nothing has started.
    Planned,
    /// At least one stop has moved on, but not all stops are finished.
    InProgress,
    /// Every stop has reached a terminal status (delivered / returned /
    /// cancelled).
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Trip {
    pub id: Uuid,
    pub org_id: Uuid,
    /// `None` only while a hired trip is waiting for the vendor's truck.
    pub vehicle_registration_number: Option<String>,
    #[serde(default)]
    pub vehicle_source: VehicleSource,
    /// The `VehicleHire` behind a hired trip.
    #[serde(default)]
    pub hire_id: Option<Uuid>,
    pub created_at: i64,
    /// Derived from the stops (see [`Trip::compute_status`]); set by every
    /// path that produces a `Trip`.
    #[serde(default = "default_planned")]
    pub status: TripStatus,
    /// The trip's stops, ordered by `stop_sequence`. Populated by the read
    /// methods; the value returned straight from
    /// `dispatch_trip_to_customers` also carries them.
    #[serde(default)]
    pub stops: Vec<DispatchOrder>,
}

/// `(id, org_id, vehicle_registration_number, created_at, vehicle_source, hire_id)`.
type TripRow = (String, String, Option<String>, i64, String, Option<String>);

fn default_planned() -> TripStatus {
    TripStatus::Planned
}

impl Trip {
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS Trips (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                vehicle_registration_number VARCHAR(255) DEFAULT NULL,
                created_at BIGINT NOT NULL,
                vehicle_source VARCHAR(10) NOT NULL DEFAULT 'OWN',
                hire_id VARCHAR(36) DEFAULT NULL,
                CONSTRAINT fk_trip_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Self::ensure_hire_columns(conn)
    }

    /// Same upgrade as `dispatch::ensure_hire_columns`, for `Trips`.
    fn ensure_hire_columns(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        for (col, ddl) in [
            ("vehicle_source", "VARCHAR(10) NOT NULL DEFAULT 'OWN'"),
            ("hire_id", "VARCHAR(36) DEFAULT NULL"),
        ] {
            let present: Option<i64> = conn.exec_first(
                "SELECT 1 FROM information_schema.columns
                 WHERE table_schema = DATABASE() AND table_name = 'Trips' AND column_name = :col",
                params! { "col" => col },
            )?;
            if present.is_none() {
                conn.query_drop(format!("ALTER TABLE Trips ADD COLUMN {col} {ddl}"))?;
            }
        }
        let reg_not_null: Option<i64> = conn.exec_first(
            "SELECT 1 FROM information_schema.columns
             WHERE table_schema = DATABASE() AND table_name = 'Trips'
               AND column_name = 'vehicle_registration_number' AND is_nullable = 'NO'",
            (),
        )?;
        if reg_not_null.is_some() {
            conn.query_drop("ALTER TABLE Trips MODIFY vehicle_registration_number VARCHAR(255) DEFAULT NULL")?;
        }
        Ok(())
    }

    /// Work the trip's status out from its stops. An empty trip counts as
    /// `Planned`.
    pub fn compute_status(stops: &[DispatchOrder]) -> TripStatus {
        if stops.is_empty()
            || stops.iter().all(|s| {
                matches!(s.status, DispatchStatus::Pending | DispatchStatus::AwaitingVehicle)
            })
        {
            TripStatus::Planned
        } else if stops.iter().all(|s| s.status.is_terminal()) {
            TripStatus::Completed
        } else {
            TripStatus::InProgress
        }
    }

    fn row_to_trip((id, org_id, veh, created_at, source, hire_id): TripRow) -> Self {
        Trip {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            vehicle_registration_number: veh,
            vehicle_source: VehicleSource::from_db(&source),
            hire_id: hire_id.and_then(|h| Uuid::parse_str(&h).ok()),
            created_at,
            status: TripStatus::Planned,
            stops: Vec::new(),
        }
    }

    /// One trip with its stops filled in, or `None` if there's no such trip.
    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let row: Option<TripRow> = conn.exec_first(
            "SELECT id, org_id, vehicle_registration_number, created_at, vehicle_source, hire_id FROM Trips WHERE id = :id",
            params! { "id" => id.to_string() },
        )?;
        let Some(row) = row else { return Ok(None) };
        let mut trip = Self::row_to_trip(row);
        trip.stops = Self::stops_of(id)?;
        trip.status = Self::compute_status(&trip.stops);
        Ok(Some(trip))
    }

    /// Every trip for an org, newest first, each with its stops.
    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let rows: Vec<TripRow> = conn.exec(
            "SELECT id, org_id, vehicle_registration_number, created_at, vehicle_source, hire_id FROM Trips
             WHERE org_id = :org_id ORDER BY created_at DESC, id DESC",
            params! { "org_id" => org_id.to_string() },
        )?;
        rows.into_iter()
            .map(|row| {
                let mut trip = Self::row_to_trip(row);
                trip.stops = Self::stops_of(trip.id)?;
                trip.status = Self::compute_status(&trip.stops);
                Ok(trip)
            })
            .collect()
    }

    fn stops_of(trip_id: Uuid) -> Result<Vec<DispatchOrder>, Box<dyn Error>> {
        DispatchOrder::list_by_trip(trip_id)
    }
}
