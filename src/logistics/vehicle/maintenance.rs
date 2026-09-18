//! Preventive maintenance scheduling for vehicles — a service due by date
//! and/or by odometer mileage, so the fleet can be warned before a truck's
//! next service is overdue. Mirrors the compliance-expiry pattern in
//! `vehicle::document` almost exactly (see `docs/vehicle-maintenance.md`):
//! a record with a due criterion, a status computed fresh on every read
//! rather than stored, and a warning window before it's actually overdue.
//!
//! The one real difference from compliance documents: there is no ambient
//! "today" equivalent for mileage the way there is for dates. This app has
//! no odometer telemetry, so `current_mileage_km` is simply the latest
//! reading ops entered on the record itself (updated the same way the
//! record's other fields are), not a live vehicle-wide value.
//!
//! Records are stored against a [`Vehicle`](super::vehicle::Vehicle) by its
//! registration number and cascade-deleted with it (and with the owning
//! org). Dates are stored as ISO `YYYY-MM-DD` strings, matching
//! `vehicle::document`.

use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// How many days ahead of a date-based due date it starts being reported as
/// [`MaintenanceStatus::DueSoon`] rather than [`MaintenanceStatus::UpToDate`].
pub const DUE_WARNING_DAYS: i64 = 14;
/// How many kilometres ahead of a mileage-based due point it starts being
/// reported as [`MaintenanceStatus::DueSoon`]. An arbitrary but reasonable
/// buffer for a heavy commercial vehicle, mirroring [`DUE_WARNING_DAYS`]'s
/// role for dates.
pub const DUE_WARNING_KM: i64 = 500;

/// Where a maintenance record sits relative to its due date/mileage,
/// computed on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum MaintenanceStatus {
    /// Neither due criterion is within its warning window yet.
    UpToDate,
    /// A due date is within [`DUE_WARNING_DAYS`], or a due mileage within
    /// [`DUE_WARNING_KM`] of the latest recorded reading — schedule it.
    DueSoon,
    /// Already past the due date, or the latest recorded mileage has already
    /// reached the due mileage.
    Overdue,
}

/// One scheduled or recorded piece of preventive maintenance for one
/// vehicle. At least one of `due_on` / `due_at_mileage_km` is required — a
/// record with neither has nothing to be "due" about.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VehicleMaintenance {
    pub id: Uuid,
    pub org_id: Uuid,
    pub vehicle_registration: String,
    /// Free text — maintenance items aren't a small fixed set the way
    /// compliance paperwork is (e.g. "Oil change", "Brake pad replacement",
    /// "Full service").
    pub description: String,
    /// ISO `YYYY-MM-DD`, or `None` if this item is scheduled by mileage only.
    pub due_on: Option<String>,
    pub due_at_mileage_km: Option<i64>,
    /// The latest odometer reading ops has recorded against this item, or
    /// `None` if never recorded. See the module doc for why this lives here
    /// rather than on `Vehicle` itself.
    pub current_mileage_km: Option<i64>,
    /// ISO `YYYY-MM-DD` — when this item was last actually serviced, if ever.
    pub last_service_on: Option<String>,
    pub notes: Option<String>,
    /// Server-computed: whole days from today until `due_on`. `None` when
    /// `due_on` isn't set. Never stored.
    pub days_until_due: Option<i64>,
    /// Server-computed: `due_at_mileage_km - current_mileage_km`. `None`
    /// unless both are set. Never stored.
    pub km_until_due: Option<i64>,
    /// Server-computed from the two fields above. Never stored.
    pub status: MaintenanceStatus,
}

/// Failure modes distinct enough for the route layer to map to status codes.
#[derive(Debug)]
pub enum VehicleMaintenanceError {
    /// A supplied date was not a valid ISO `YYYY-MM-DD` calendar date.
    InvalidDate(String),
    /// Neither `due_on` nor `due_at_mileage_km` was supplied.
    NoDueCriterion,
    /// A lower-level database error.
    Db(Box<dyn Error>),
}

impl fmt::Display for VehicleMaintenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VehicleMaintenanceError::InvalidDate(s) => {
                write!(f, "'{s}' is not a valid ISO date (expected YYYY-MM-DD)")
            }
            VehicleMaintenanceError::NoDueCriterion => write!(
                f,
                "a maintenance item needs a due date, a due mileage, or both"
            ),
            VehicleMaintenanceError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl Error for VehicleMaintenanceError {}

impl From<mysql::Error> for VehicleMaintenanceError {
    fn from(e: mysql::Error) -> Self {
        VehicleMaintenanceError::Db(Box::new(e))
    }
}

impl From<Box<dyn Error>> for VehicleMaintenanceError {
    fn from(e: Box<dyn Error>) -> Self {
        VehicleMaintenanceError::Db(e)
    }
}

/// Days from the Unix epoch (1970-01-01) to `y-m-d`, via Howard Hinnant's
/// `days_from_civil`. Duplicated locally rather than shared, matching
/// `vehicle::document` / `billing::invoice`'s existing convention of a
/// module-local date helper so the crate takes no date dependency.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn parse_iso_date(s: &str) -> Result<(i64, i64, i64), VehicleMaintenanceError> {
    let invalid = || VehicleMaintenanceError::InvalidDate(s.to_string());
    let parts: Vec<&str> = s.trim().split('-').collect();
    if parts.len() != 3 {
        return Err(invalid());
    }
    let y: i64 = parts[0].parse().map_err(|_| invalid())?;
    let m: i64 = parts[1].parse().map_err(|_| invalid())?;
    let d: i64 = parts[2].parse().map_err(|_| invalid())?;
    if !(1..=12).contains(&m) || d < 1 {
        return Err(invalid());
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(m - 1) as usize];
    if d > dim {
        return Err(invalid());
    }
    Ok((y, m, d))
}

fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .div_euclid(86_400)
}

/// Compute `(days_until_due, km_until_due, status)` for already-validated
/// input.
fn evaluate(
    due_on: Option<&str>,
    due_at_mileage_km: Option<i64>,
    current_mileage_km: Option<i64>,
) -> (Option<i64>, Option<i64>, MaintenanceStatus) {
    let days_until_due = due_on.map(|d| {
        parse_iso_date(d)
            .map(|(y, m, dd)| days_from_civil(y, m, dd) - today_days())
            .unwrap_or(0)
    });
    let km_until_due = match (due_at_mileage_km, current_mileage_km) {
        (Some(due), Some(current)) => Some(due - current),
        _ => None,
    };

    let overdue = days_until_due.is_some_and(|d| d < 0) || km_until_due.is_some_and(|k| k <= 0);
    let due_soon = !overdue
        && (days_until_due.is_some_and(|d| d <= DUE_WARNING_DAYS)
            || km_until_due.is_some_and(|k| k <= DUE_WARNING_KM));

    let status = if overdue {
        MaintenanceStatus::Overdue
    } else if due_soon {
        MaintenanceStatus::DueSoon
    } else {
        MaintenanceStatus::UpToDate
    };
    (days_until_due, km_until_due, status)
}

/// The columns every read selects, in the order [`VehicleMaintenance::hydrate`]
/// consumes them.
const SELECT_COLS: &str = "id, org_id, vehicle_registration, description, due_on, \
     due_at_mileage_km, current_mileage_km, last_service_on, notes";

/// One raw `VehicleMaintenance` row, matching [`SELECT_COLS`].
type MaintenanceRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
);

impl VehicleMaintenance {
    /// Create the `VehicleMaintenance` table if it does not exist. Kept in
    /// sync with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS VehicleMaintenance (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                vehicle_registration VARCHAR(255) NOT NULL,
                description VARCHAR(255) NOT NULL,
                due_on VARCHAR(10) DEFAULT NULL,
                due_at_mileage_km BIGINT DEFAULT NULL,
                current_mileage_km BIGINT DEFAULT NULL,
                last_service_on VARCHAR(10) DEFAULT NULL,
                notes TEXT DEFAULT NULL,
                CONSTRAINT fk_vehiclemaint_org
                    FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE,
                CONSTRAINT fk_vehiclemaint_vehicle
                    FOREIGN KEY (vehicle_registration)
                    REFERENCES Vehicle(registration_number) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    fn hydrate(row: MaintenanceRow) -> Self {
        let (
            id,
            org_id,
            vehicle_registration,
            description,
            due_on,
            due_at_mileage_km,
            current_mileage_km,
            last_service_on,
            notes,
        ) = row;
        let (days_until_due, km_until_due, status) =
            evaluate(due_on.as_deref(), due_at_mileage_km, current_mileage_km);
        VehicleMaintenance {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            vehicle_registration,
            description,
            due_on,
            due_at_mileage_km,
            current_mileage_km,
            last_service_on,
            notes,
            days_until_due,
            km_until_due,
            status,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        org_id: Uuid,
        vehicle_registration: impl Into<String>,
        description: impl Into<String>,
        due_on: Option<String>,
        due_at_mileage_km: Option<i64>,
        last_service_on: Option<String>,
        notes: Option<String>,
    ) -> Result<Self, VehicleMaintenanceError> {
        if due_on.is_none() && due_at_mileage_km.is_none() {
            return Err(VehicleMaintenanceError::NoDueCriterion);
        }
        if let Some(ref d) = due_on {
            parse_iso_date(d)?;
        }
        if let Some(ref d) = last_service_on {
            parse_iso_date(d)?;
        }

        let mut conn = DbConnection::from_env()
            .get_connection()
            .map_err(VehicleMaintenanceError::Db)?;
        Self::ensure_table(&mut conn).map_err(VehicleMaintenanceError::Db)?;

        let id = Uuid::new_v4();
        let vehicle_registration = vehicle_registration.into();
        let description = description.into();

        conn.exec_drop(
            "INSERT INTO VehicleMaintenance
               (id, org_id, vehicle_registration, description, due_on,
                due_at_mileage_km, current_mileage_km, last_service_on, notes)
             VALUES
               (:id, :org_id, :vehicle_registration, :description, :due_on,
                :due_at_mileage_km, NULL, :last_service_on, :notes)",
            params! {
                "id" => id.to_string(),
                "org_id" => org_id.to_string(),
                "vehicle_registration" => &vehicle_registration,
                "description" => &description,
                "due_on" => &due_on,
                "due_at_mileage_km" => due_at_mileage_km,
                "last_service_on" => &last_service_on,
                "notes" => &notes,
            },
        )?;

        let (days_until_due, km_until_due, status) = evaluate(due_on.as_deref(), due_at_mileage_km, None);
        let item = VehicleMaintenance {
            id,
            org_id,
            vehicle_registration,
            description,
            due_on,
            due_at_mileage_km,
            current_mileage_km: None,
            last_service_on,
            notes,
            days_until_due,
            km_until_due,
            status,
        };
        crate::logistics::ai::chunk::upsert_best_effort(
            item.org_id,
            crate::logistics::ai::chunk::ChunkKind::VehicleMaintenance,
            &item.id.to_string(),
            crate::logistics::ai::chunk::vehicle_maintenance_chunk_text(&item),
        );
        Ok(item)
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<MaintenanceRow> = conn.exec_first(
            format!("SELECT {SELECT_COLS} FROM VehicleMaintenance WHERE id = :id"),
            params! { "id" => id.to_string() },
        )?;
        Ok(row.map(Self::hydrate))
    }

    pub fn list_by_vehicle(vehicle_registration: &str) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<MaintenanceRow> = conn.exec(
            format!(
                "SELECT {SELECT_COLS} FROM VehicleMaintenance
                 WHERE vehicle_registration = :reg
                 ORDER BY due_on IS NULL, due_on ASC"
            ),
            params! { "reg" => vehicle_registration },
        )?;
        Ok(rows.into_iter().map(Self::hydrate).collect())
    }

    /// Every maintenance item for an org's whole fleet, soonest date-based
    /// due date first (mileage-only items sort after every dated one) — the
    /// list a "service due" view is built from.
    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<MaintenanceRow> = conn.exec(
            format!(
                "SELECT {SELECT_COLS} FROM VehicleMaintenance
                 WHERE org_id = :org_id
                 ORDER BY due_on IS NULL, due_on ASC"
            ),
            params! { "org_id" => org_id.to_string() },
        )?;
        Ok(rows.into_iter().map(Self::hydrate).collect())
    }

    /// Update the schedule itself — description, due date and/or due
    /// mileage, and the informational last-serviced date/notes. Does not
    /// touch `current_mileage_km`; see [`Self::record_mileage`] for that.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        description: impl Into<String>,
        due_on: Option<String>,
        due_at_mileage_km: Option<i64>,
        last_service_on: Option<String>,
        notes: Option<String>,
    ) -> Result<(), VehicleMaintenanceError> {
        if due_on.is_none() && due_at_mileage_km.is_none() {
            return Err(VehicleMaintenanceError::NoDueCriterion);
        }
        if let Some(ref d) = due_on {
            parse_iso_date(d)?;
        }
        if let Some(ref d) = last_service_on {
            parse_iso_date(d)?;
        }
        let description = description.into();

        let mut conn = DbConnection::from_env()
            .get_connection()
            .map_err(VehicleMaintenanceError::Db)?;
        conn.exec_drop(
            "UPDATE VehicleMaintenance SET
                 description = :description,
                 due_on = :due_on,
                 due_at_mileage_km = :due_at_mileage_km,
                 last_service_on = :last_service_on,
                 notes = :notes
             WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "description" => &description,
                "due_on" => &due_on,
                "due_at_mileage_km" => due_at_mileage_km,
                "last_service_on" => &last_service_on,
                "notes" => &notes,
            },
        )?;

        let (days_until_due, km_until_due, status) =
            evaluate(due_on.as_deref(), due_at_mileage_km, self.current_mileage_km);
        self.description = description;
        self.due_on = due_on;
        self.due_at_mileage_km = due_at_mileage_km;
        self.last_service_on = last_service_on;
        self.notes = notes;
        self.days_until_due = days_until_due;
        self.km_until_due = km_until_due;
        self.status = status;
        crate::logistics::ai::chunk::upsert_best_effort(
            self.org_id,
            crate::logistics::ai::chunk::ChunkKind::VehicleMaintenance,
            &self.id.to_string(),
            crate::logistics::ai::chunk::vehicle_maintenance_chunk_text(self),
        );
        Ok(())
    }

    /// Record the latest odometer reading against this item — the only way
    /// `current_mileage_km` (and so mileage-based due status) ever changes,
    /// since this app has no automatic odometer telemetry.
    pub fn record_mileage(&mut self, current_mileage_km: i64) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "UPDATE VehicleMaintenance SET current_mileage_km = :km WHERE id = :id",
            params! { "km" => current_mileage_km, "id" => self.id.to_string() },
        )?;

        let (days_until_due, km_until_due, status) =
            evaluate(self.due_on.as_deref(), self.due_at_mileage_km, Some(current_mileage_km));
        self.current_mileage_km = Some(current_mileage_km);
        self.days_until_due = days_until_due;
        self.km_until_due = km_until_due;
        self.status = status;
        crate::logistics::ai::chunk::upsert_best_effort(
            self.org_id,
            crate::logistics::ai::chunk::ChunkKind::VehicleMaintenance,
            &self.id.to_string(),
            crate::logistics::ai::chunk::vehicle_maintenance_chunk_text(self),
        );
        Ok(())
    }

    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "DELETE FROM VehicleMaintenance WHERE id = :id",
            params! { "id" => self.id.to_string() },
        )?;
        crate::logistics::ai::chunk::delete_by_source_best_effort(
            crate::logistics::ai::chunk::ChunkKind::VehicleMaintenance,
            &self.id.to_string(),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::TestDb;
    use crate::logistics::vehicle::vehicle::{Unit, Vehicle};

    fn org_with_vehicle(reg: &str) -> Organization {
        let org = Organization::create_organization("Maintenance Test Org", "1 Depot Road")
            .expect("create org");
        Vehicle::new(reg, 20, Unit::MetricTon)
            .add_new_vehicle_to_org(&org)
            .expect("add vehicle");
        org
    }

    fn date_offset(days: i64) -> String {
        let days_since_epoch = today_days() + days;
        // Reuse the module's own civil-date math in reverse via a tiny
        // brute-force search — fine at test scale, and avoids a second date
        // dependency just for the tests.
        let mut y = 1970i64;
        loop {
            let start_of_year = days_from_civil(y, 1, 1);
            let start_of_next = days_from_civil(y + 1, 1, 1);
            if days_since_epoch < start_of_next {
                let mut m = 1i64;
                loop {
                    let start_of_month = days_from_civil(y, m, 1);
                    let start_of_next_month = if m == 12 {
                        days_from_civil(y + 1, 1, 1)
                    } else {
                        days_from_civil(y, m + 1, 1)
                    };
                    if days_since_epoch < start_of_next_month {
                        let d = days_since_epoch - start_of_month + 1;
                        return format!("{y:04}-{m:02}-{d:02}");
                    }
                    m += 1;
                }
            }
            if days_since_epoch < start_of_year {
                y -= 1;
            } else {
                y += 1;
            }
        }
    }

    #[test]
    fn test_create_rejects_a_record_with_no_due_criterion() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0001");
        let err = VehicleMaintenance::create(org.id, "MH12MT0001", "Oil change", None, None, None, None)
            .unwrap_err();
        assert!(matches!(err, VehicleMaintenanceError::NoDueCriterion));
    }

    #[test]
    fn test_create_rejects_a_bad_due_date() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0002");
        let err = VehicleMaintenance::create(
            org.id,
            "MH12MT0002",
            "Oil change",
            Some("2026-02-30".to_string()),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, VehicleMaintenanceError::InvalidDate(_)));
    }

    #[test]
    fn test_status_tracks_the_date_based_due_window() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0003");

        let valid = VehicleMaintenance::create(
            org.id, "MH12MT0003", "Full service", Some(date_offset(120)), None, None, None,
        )
        .expect("create");
        assert_eq!(valid.status, MaintenanceStatus::UpToDate);
        assert!(valid.days_until_due.unwrap() >= 118);

        let soon = VehicleMaintenance::create(
            org.id, "MH12MT0003", "Full service", Some(date_offset(5)), None, None, None,
        )
        .expect("create");
        assert_eq!(soon.status, MaintenanceStatus::DueSoon);

        let overdue = VehicleMaintenance::create(
            org.id, "MH12MT0003", "Full service", Some(date_offset(-3)), None, None, None,
        )
        .expect("create");
        assert_eq!(overdue.status, MaintenanceStatus::Overdue);
        assert!(overdue.days_until_due.unwrap() < 0);
    }

    #[test]
    fn test_status_tracks_the_mileage_based_due_window_once_a_reading_is_recorded() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0004");

        let mut item = VehicleMaintenance::create(
            org.id, "MH12MT0004", "Tyre rotation", None, Some(50_000), None, None,
        )
        .expect("create");
        // No reading recorded yet -> nothing to compare against.
        assert_eq!(item.status, MaintenanceStatus::UpToDate);
        assert_eq!(item.km_until_due, None);

        item.record_mileage(49_000).expect("record");
        assert_eq!(item.status, MaintenanceStatus::UpToDate);
        assert_eq!(item.km_until_due, Some(1_000));

        item.record_mileage(49_600).expect("record");
        assert_eq!(item.status, MaintenanceStatus::DueSoon);

        item.record_mileage(50_200).expect("record");
        assert_eq!(item.status, MaintenanceStatus::Overdue);
    }

    #[test]
    fn test_either_trigger_alone_can_make_a_record_overdue() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0005");

        // Due date far off, but mileage already overdue.
        let mut item = VehicleMaintenance::create(
            org.id, "MH12MT0005", "Brake pads", Some(date_offset(300)), Some(40_000), None, None,
        )
        .expect("create");
        item.record_mileage(41_000).expect("record");
        assert_eq!(item.status, MaintenanceStatus::Overdue);
    }

    #[test]
    fn test_update_changes_the_schedule_and_recomputes_status() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0006");
        let mut item = VehicleMaintenance::create(
            org.id, "MH12MT0006", "Oil change", Some(date_offset(1)), None, None, None,
        )
        .expect("create");
        assert_eq!(item.status, MaintenanceStatus::DueSoon);

        item.update("Oil change", Some(date_offset(120)), None, Some(date_offset(-1)), Some("done".to_string()))
            .expect("update");
        assert_eq!(item.status, MaintenanceStatus::UpToDate);
        assert_eq!(item.notes, Some("done".to_string()));
    }

    #[test]
    fn test_update_rejects_dropping_both_due_criteria() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0007");
        let mut item = VehicleMaintenance::create(
            org.id, "MH12MT0007", "Oil change", Some(date_offset(30)), None, None, None,
        )
        .expect("create");
        let err = item.update("Oil change", None, None, None, None).unwrap_err();
        assert!(matches!(err, VehicleMaintenanceError::NoDueCriterion));
    }

    #[test]
    fn test_delete_removes_the_record() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0008");
        let item = VehicleMaintenance::create(
            org.id, "MH12MT0008", "Oil change", Some(date_offset(30)), None, None, None,
        )
        .expect("create");
        item.delete().expect("delete");
        assert!(VehicleMaintenance::get_by_id(item.id).expect("get").is_none());
    }

    #[test]
    fn test_deleting_the_vehicle_cascades_to_its_maintenance_records() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0009");
        VehicleMaintenance::create(
            org.id, "MH12MT0009", "Oil change", Some(date_offset(30)), None, None, None,
        )
        .expect("create");

        let vehicle = Vehicle::new("MH12MT0009", 20, Unit::MetricTon);
        vehicle.remove_vehicle().expect("remove vehicle");

        assert!(VehicleMaintenance::list_by_org(org.id).expect("list").is_empty());
    }

    #[test]
    fn test_list_by_vehicle_and_org_order_dated_items_before_mileage_only_ones() {
        let _db = TestDb::create();
        let org = org_with_vehicle("MH12MT0010");
        let mileage_only = VehicleMaintenance::create(
            org.id, "MH12MT0010", "Tyre check", None, Some(60_000), None, None,
        )
        .expect("create");
        let dated = VehicleMaintenance::create(
            org.id, "MH12MT0010", "Full service", Some(date_offset(10)), None, None, None,
        )
        .expect("create");

        let by_vehicle = VehicleMaintenance::list_by_vehicle("MH12MT0010").expect("list");
        assert_eq!(by_vehicle.iter().map(|m| m.id).collect::<Vec<_>>(), vec![dated.id, mileage_only.id]);

        let by_org = VehicleMaintenance::list_by_org(org.id).expect("list");
        assert_eq!(by_org.len(), 2);
    }
}
