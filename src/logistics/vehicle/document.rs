//! Vehicle compliance paperwork — insurance, registration certificate (RC),
//! national/state permit, pollution-under-control (PUC) certificate and
//! fitness certificate — each with an expiry date so the fleet can be warned
//! before a truck's papers lapse. Every commercial vehicle on Indian roads
//! must carry valid copies of these; an expired one grounds the vehicle.
//!
//! Records are stored against a [`Vehicle`](super::vehicle::Vehicle) by its
//! registration number and cascade-deleted with it (and with the owning org).
//! Dates are stored as ISO `YYYY-MM-DD` strings; `days_until_expiry` and
//! `status` are recomputed from "today" on every read, never persisted.

use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// How many days ahead of a document's expiry it starts being reported as
/// [`ComplianceStatus::ExpiringSoon`] rather than [`ComplianceStatus::Valid`].
pub const EXPIRY_WARNING_DAYS: i64 = 30;

/// The kinds of compliance paperwork a commercial vehicle carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum ComplianceDocType {
    Insurance,
    RegistrationCertificate,
    Permit,
    PollutionCertificate,
    FitnessCertificate,
}

impl ComplianceDocType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComplianceDocType::Insurance => "Insurance",
            ComplianceDocType::RegistrationCertificate => "RegistrationCertificate",
            ComplianceDocType::Permit => "Permit",
            ComplianceDocType::PollutionCertificate => "PollutionCertificate",
            ComplianceDocType::FitnessCertificate => "FitnessCertificate",
        }
    }

    /// Parse a stored/incoming type string. Accepts the canonical names
    /// ([`ComplianceDocType::as_str`]) plus common shorthands (`RC`, `PUC`,
    /// `FC`), case-insensitively and ignoring spaces/dashes/underscores.
    /// Unknown input falls back to `Insurance` so a bad value never fails a
    /// request outright — mirrors [`Unit::from_str`](super::vehicle::Unit::from_str).
    pub fn from_str(s: &str) -> Self {
        let normalized = s
            .trim()
            .to_ascii_lowercase()
            .replace([' ', '-', '_'], "");
        match normalized.as_str() {
            "registrationcertificate" | "rc" | "registration" => {
                ComplianceDocType::RegistrationCertificate
            }
            "permit" => ComplianceDocType::Permit,
            "pollutioncertificate" | "puc" | "pollution" | "pucc" => {
                ComplianceDocType::PollutionCertificate
            }
            "fitnesscertificate" | "fitness" | "fc" => ComplianceDocType::FitnessCertificate,
            "insurance" => ComplianceDocType::Insurance,
            _ => ComplianceDocType::Insurance,
        }
    }
}

/// Where a document sits relative to its expiry date, computed on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum ComplianceStatus {
    /// More than [`EXPIRY_WARNING_DAYS`] days of validity left.
    Valid,
    /// Expires within [`EXPIRY_WARNING_DAYS`] days — renew it.
    ExpiringSoon,
    /// Already past its expiry date.
    Expired,
}

/// A single piece of compliance paperwork for one vehicle.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VehicleDocument {
    pub id: Uuid,
    pub org_id: Uuid,
    pub vehicle_registration: String,
    pub doc_type: ComplianceDocType,
    /// The policy / certificate number as printed on the document.
    pub document_number: String,
    /// ISO `YYYY-MM-DD`, or `null` — some paperwork only records an expiry.
    pub issued_on: Option<String>,
    /// ISO `YYYY-MM-DD`. Required — the reason the record exists.
    pub expires_on: String,
    pub notes: Option<String>,
    /// Server-computed: whole days from today until `expires_on`, negative if
    /// the date is in the past. Never stored.
    pub days_until_expiry: i64,
    /// Server-computed from `days_until_expiry`. Never stored.
    pub status: ComplianceStatus,
}

/// Failure modes distinct enough for the route layer to map to status codes.
#[derive(Debug)]
pub enum VehicleDocumentError {
    /// A supplied date was not a valid ISO `YYYY-MM-DD` calendar date.
    InvalidDate(String),
    /// A lower-level database error.
    Db(Box<dyn Error>),
}

impl fmt::Display for VehicleDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VehicleDocumentError::InvalidDate(s) => {
                write!(f, "'{s}' is not a valid ISO date (expected YYYY-MM-DD)")
            }
            VehicleDocumentError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl Error for VehicleDocumentError {}

impl From<mysql::Error> for VehicleDocumentError {
    fn from(e: mysql::Error) -> Self {
        VehicleDocumentError::Db(Box::new(e))
    }
}

impl From<Box<dyn Error>> for VehicleDocumentError {
    fn from(e: Box<dyn Error>) -> Self {
        VehicleDocumentError::Db(e)
    }
}

/// Days from the Unix epoch (1970-01-01) to `y-m-d`, via Howard Hinnant's
/// `days_from_civil`. Correct for any proleptic-Gregorian date, no date crate.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Validate and parse an ISO `YYYY-MM-DD` string into `(year, month, day)`,
/// rejecting out-of-range months/days and calendar-invalid dates like
/// `2026-02-30`.
fn parse_iso_date(s: &str) -> Result<(i64, i64, i64), VehicleDocumentError> {
    let invalid = || VehicleDocumentError::InvalidDate(s.to_string());
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
    let dim = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][(m - 1) as usize];
    if d > dim {
        return Err(invalid());
    }
    Ok((y, m, d))
}

/// Today as whole days since the Unix epoch, in UTC.
fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .div_euclid(86_400)
}

/// The eight columns every read selects, in the order [`VehicleDocument::hydrate`]
/// consumes them.
const SELECT_COLS: &str = "id, org_id, vehicle_registration, doc_type, \
     document_number, issued_on, expires_on, notes";

/// One raw `VehicleDocuments` row, matching [`SELECT_COLS`].
type DocRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
);

/// Compute `(days_until_expiry, status)` for an already-validated ISO date.
fn evaluate(expires_on: &str) -> (i64, ComplianceStatus) {
    let days = parse_iso_date(expires_on)
        .map(|(y, m, d)| days_from_civil(y, m, d) - today_days())
        .unwrap_or(0);
    let status = if days < 0 {
        ComplianceStatus::Expired
    } else if days <= EXPIRY_WARNING_DAYS {
        ComplianceStatus::ExpiringSoon
    } else {
        ComplianceStatus::Valid
    };
    (days, status)
}

impl VehicleDocument {
    /// Create the `VehicleDocuments` table if it does not exist. Kept in sync
    /// with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS VehicleDocuments (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                vehicle_registration VARCHAR(255) NOT NULL,
                doc_type VARCHAR(48) NOT NULL,
                document_number VARCHAR(255) NOT NULL,
                issued_on VARCHAR(10) DEFAULT NULL,
                expires_on VARCHAR(10) NOT NULL,
                notes TEXT DEFAULT NULL,
                CONSTRAINT fk_vehicledoc_org
                    FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE,
                CONSTRAINT fk_vehicledoc_vehicle
                    FOREIGN KEY (vehicle_registration)
                    REFERENCES Vehicle(registration_number) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    /// Build a `VehicleDocument` from one raw [`DocRow`], computing the
    /// derived `days_until_expiry` / `status` fields.
    fn hydrate(row: DocRow) -> Self {
        let (id, org_id, vehicle_registration, doc_type, document_number, issued_on, expires_on, notes) =
            row;
        let (days_until_expiry, status) = evaluate(&expires_on);
        VehicleDocument {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            vehicle_registration,
            doc_type: ComplianceDocType::from_str(&doc_type),
            document_number,
            issued_on,
            expires_on,
            notes,
            days_until_expiry,
            status,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        org_id: Uuid,
        vehicle_registration: impl Into<String>,
        doc_type: ComplianceDocType,
        document_number: impl Into<String>,
        issued_on: Option<String>,
        expires_on: impl Into<String>,
        notes: Option<String>,
    ) -> Result<Self, VehicleDocumentError> {
        let expires_on = expires_on.into();
        parse_iso_date(&expires_on)?;
        if let Some(ref issued) = issued_on {
            parse_iso_date(issued)?;
        }

        let mut conn = DbConnection::from_env()
            .get_connection()
            .map_err(VehicleDocumentError::Db)?;
        Self::ensure_table(&mut conn).map_err(VehicleDocumentError::Db)?;

        let id = Uuid::new_v4();
        let vehicle_registration = vehicle_registration.into();
        let document_number = document_number.into();

        conn.exec_drop(
            "INSERT INTO VehicleDocuments
               (id, org_id, vehicle_registration, doc_type, document_number,
                issued_on, expires_on, notes)
             VALUES
               (:id, :org_id, :vehicle_registration, :doc_type, :document_number,
                :issued_on, :expires_on, :notes)",
            params! {
                "id" => id.to_string(),
                "org_id" => org_id.to_string(),
                "vehicle_registration" => &vehicle_registration,
                "doc_type" => doc_type.as_str(),
                "document_number" => &document_number,
                "issued_on" => &issued_on,
                "expires_on" => &expires_on,
                "notes" => &notes,
            },
        )?;

        let (days_until_expiry, status) = evaluate(&expires_on);
        Ok(VehicleDocument {
            id,
            org_id,
            vehicle_registration,
            doc_type,
            document_number,
            issued_on,
            expires_on,
            notes,
            days_until_expiry,
            status,
        })
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<DocRow> = conn.exec_first(
            format!(
                "SELECT {} FROM VehicleDocuments WHERE id = :id",
                SELECT_COLS
            ),
            params! { "id" => id.to_string() },
        )?;

        Ok(row.map(Self::hydrate))
    }

    pub fn list_by_vehicle(
        vehicle_registration: &str,
    ) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<DocRow> = conn.exec(
            format!(
                "SELECT {} FROM VehicleDocuments
                 WHERE vehicle_registration = :reg
                 ORDER BY expires_on ASC",
                SELECT_COLS
            ),
            params! { "reg" => vehicle_registration },
        )?;

        Ok(rows
            .into_iter()
            .map(Self::hydrate)
            .collect())
    }

    /// Every compliance document for an org's whole fleet, soonest expiry
    /// first — the list a "renewals due" view is built from.
    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<DocRow> = conn.exec(
            format!(
                "SELECT {} FROM VehicleDocuments
                 WHERE org_id = :org_id
                 ORDER BY expires_on ASC",
                SELECT_COLS
            ),
            params! { "org_id" => org_id.to_string() },
        )?;

        Ok(rows
            .into_iter()
            .map(Self::hydrate)
            .collect())
    }

    /// Update the mutable fields — in practice this is how a document gets
    /// "renewed": push `expires_on` forward and update the number.
    pub fn update(
        &mut self,
        doc_type: ComplianceDocType,
        document_number: impl Into<String>,
        issued_on: Option<String>,
        expires_on: impl Into<String>,
        notes: Option<String>,
    ) -> Result<(), VehicleDocumentError> {
        let expires_on = expires_on.into();
        parse_iso_date(&expires_on)?;
        if let Some(ref issued) = issued_on {
            parse_iso_date(issued)?;
        }
        let document_number = document_number.into();

        let mut conn = DbConnection::from_env()
            .get_connection()
            .map_err(VehicleDocumentError::Db)?;
        conn.exec_drop(
            "UPDATE VehicleDocuments SET
                 doc_type = :doc_type,
                 document_number = :document_number,
                 issued_on = :issued_on,
                 expires_on = :expires_on,
                 notes = :notes
             WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "doc_type" => doc_type.as_str(),
                "document_number" => &document_number,
                "issued_on" => &issued_on,
                "expires_on" => &expires_on,
                "notes" => &notes,
            },
        )?;

        let (days_until_expiry, status) = evaluate(&expires_on);
        self.doc_type = doc_type;
        self.document_number = document_number;
        self.issued_on = issued_on;
        self.expires_on = expires_on;
        self.notes = notes;
        self.days_until_expiry = days_until_expiry;
        self.status = status;
        Ok(())
    }

    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "DELETE FROM VehicleDocuments WHERE id = :id",
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
    use crate::logistics::vehicle::vehicle::{Unit, Vehicle};

    fn org_with_vehicle(reg: &str) -> Organization {
        let org = Organization::create_organization("Compliance Test Org", "1 Depot Road")
            .expect("create org");
        Vehicle::new(reg, 20, Unit::MetricTon)
            .add_new_vehicle_to_org(&org)
            .expect("add vehicle");
        org
    }

    /// An ISO date `offset` days from today, for expiry-window tests.
    fn date_offset(offset: i64) -> String {
        let target = today_days() + offset;
        // invert days_from_civil (Hinnant civil_from_days)
        let z = target + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{y:04}-{m:02}-{d:02}")
    }

    #[test]
    fn test_parse_iso_date_rejects_nonsense() {
        assert!(parse_iso_date("2026-09-03").is_ok());
        assert!(parse_iso_date("2026-02-30").is_err());
        assert!(parse_iso_date("2026-13-01").is_err());
        assert!(parse_iso_date("not-a-date").is_err());
        assert!(parse_iso_date("2026/09/03").is_err());
        assert!(parse_iso_date("2024-02-29").is_ok()); // leap year
    }

    #[test]
    fn test_doc_type_from_str_aliases() {
        assert_eq!(
            ComplianceDocType::from_str("RC"),
            ComplianceDocType::RegistrationCertificate
        );
        assert_eq!(
            ComplianceDocType::from_str("puc"),
            ComplianceDocType::PollutionCertificate
        );
        assert_eq!(
            ComplianceDocType::from_str(" Fitness Certificate "),
            ComplianceDocType::FitnessCertificate
        );
        assert_eq!(
            ComplianceDocType::from_str("garbage"),
            ComplianceDocType::Insurance
        );
    }

    #[test]
    fn test_status_tracks_the_expiry_window() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 1111");

        let valid = VehicleDocument::create(
            org.id,
            "KA01 AA 1111",
            ComplianceDocType::Insurance,
            "POL-1",
            None,
            date_offset(120),
            None,
        )
        .expect("create valid");
        assert_eq!(valid.status, ComplianceStatus::Valid);
        assert!(valid.days_until_expiry >= 118);

        let soon = VehicleDocument::create(
            org.id,
            "KA01 AA 1111",
            ComplianceDocType::Permit,
            "PMT-1",
            None,
            date_offset(10),
            None,
        )
        .expect("create soon");
        assert_eq!(soon.status, ComplianceStatus::ExpiringSoon);

        let expired = VehicleDocument::create(
            org.id,
            "KA01 AA 1111",
            ComplianceDocType::PollutionCertificate,
            "PUC-1",
            None,
            date_offset(-5),
            None,
        )
        .expect("create expired");
        assert_eq!(expired.status, ComplianceStatus::Expired);
        assert!(expired.days_until_expiry < 0);
    }

    #[test]
    fn test_create_rejects_a_bad_expiry_date() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 2222");
        let err = VehicleDocument::create(
            org.id,
            "KA01 AA 2222",
            ComplianceDocType::Insurance,
            "POL-X",
            None,
            "2026-99-99",
            None,
        )
        .expect_err("bad date should be rejected");
        assert!(matches!(err, VehicleDocumentError::InvalidDate(_)));
    }

    #[test]
    fn test_list_by_vehicle_and_org_order_by_soonest_expiry() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 3333");
        Vehicle::new("KA01 AA 4444", 15, Unit::MetricTon)
            .add_new_vehicle_to_org(&Organization::get_by_id(org.id).unwrap().unwrap())
            .expect("second vehicle");

        VehicleDocument::create(
            org.id,
            "KA01 AA 3333",
            ComplianceDocType::Insurance,
            "LATE",
            None,
            date_offset(200),
            None,
        )
        .unwrap();
        VehicleDocument::create(
            org.id,
            "KA01 AA 4444",
            ComplianceDocType::Permit,
            "EARLY",
            None,
            date_offset(5),
            None,
        )
        .unwrap();

        let for_vehicle = VehicleDocument::list_by_vehicle("KA01 AA 3333").unwrap();
        assert_eq!(for_vehicle.len(), 1);
        assert_eq!(for_vehicle[0].document_number, "LATE");

        let for_org = VehicleDocument::list_by_org(org.id).unwrap();
        assert_eq!(for_org.len(), 2);
        assert_eq!(for_org[0].document_number, "EARLY"); // soonest expiry first
    }

    #[test]
    fn test_update_renews_and_recomputes_status() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 5555");
        let mut doc = VehicleDocument::create(
            org.id,
            "KA01 AA 5555",
            ComplianceDocType::FitnessCertificate,
            "FC-OLD",
            None,
            date_offset(-3),
            None,
        )
        .unwrap();
        assert_eq!(doc.status, ComplianceStatus::Expired);

        doc.update(
            ComplianceDocType::FitnessCertificate,
            "FC-NEW",
            Some(date_offset(0)),
            date_offset(365),
            Some("renewed at RTO".to_string()),
        )
        .expect("renew");

        let reloaded = VehicleDocument::get_by_id(doc.id).unwrap().unwrap();
        assert_eq!(reloaded.document_number, "FC-NEW");
        assert_eq!(reloaded.status, ComplianceStatus::Valid);
        assert_eq!(reloaded.notes.as_deref(), Some("renewed at RTO"));
    }

    #[test]
    fn test_delete_removes_the_document() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 6666");
        let doc = VehicleDocument::create(
            org.id,
            "KA01 AA 6666",
            ComplianceDocType::Insurance,
            "POL-DEL",
            None,
            date_offset(30),
            None,
        )
        .unwrap();
        doc.delete().expect("delete");
        assert!(VehicleDocument::get_by_id(doc.id).unwrap().is_none());
    }

    #[test]
    fn test_deleting_the_vehicle_cascades_to_its_documents() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 7777");
        VehicleDocument::create(
            org.id,
            "KA01 AA 7777",
            ComplianceDocType::Permit,
            "PMT-CASCADE",
            None,
            date_offset(60),
            None,
        )
        .unwrap();

        let vehicle = Vehicle::new("KA01 AA 7777", 20, Unit::MetricTon);
        vehicle.remove_vehicle().expect("remove vehicle");

        assert!(VehicleDocument::list_by_org(org.id).unwrap().is_empty());
    }

    #[test]
    fn test_deleting_the_org_cascades_to_documents() {
        let _db = TestDb::create();
        let org = org_with_vehicle("KA01 AA 8888");
        VehicleDocument::create(
            org.id,
            "KA01 AA 8888",
            ComplianceDocType::Insurance,
            "POL-ORG",
            None,
            date_offset(90),
            None,
        )
        .unwrap();

        org.remove_organization().expect("remove org");
        assert!(VehicleDocument::list_by_org(org.id).unwrap().is_empty());
    }
}
