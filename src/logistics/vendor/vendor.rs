//! Vehicle vendors: the transporters / brokers an organisation phones to hire
//! a "market truck" when it has no free vehicle of its own.
//!
//! This is phase 1 of [`docs/vehicle-vendors.md`]: the vendor directory only.
//! Hiring a vehicle from a vendor for a dispatch comes in phase 2.

use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use uuid::Uuid;

/// A transporter or broker the organisation hires vehicles from.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VehicleVendor {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    /// The person the dispatcher usually speaks to.
    pub contact_person: Option<String>,
    /// Required: the dispatcher books a vehicle by phoning this number.
    pub phone: String,
    /// 15-character GST identification number, stored upper-case.
    pub gstin: Option<String>,
    /// Free text: rate terms, lanes served, vehicle types on offer.
    pub notes: Option<String>,
    /// Inactive vendors are kept for history but hidden from the hire picker.
    pub is_active: bool,
}

/// The editable fields of a vendor, shared by create and update.
#[derive(Debug, Clone, Default)]
pub struct VendorInput {
    pub name: String,
    pub contact_person: Option<String>,
    pub phone: String,
    pub gstin: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug)]
pub enum VendorError {
    /// A required field was blank or the GSTIN was malformed.
    InvalidInput(String),
    Db(Box<dyn Error>),
}

impl fmt::Display for VendorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VendorError::InvalidInput(why) => write!(f, "{why}"),
            VendorError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for VendorError {}

impl From<mysql::Error> for VendorError {
    fn from(e: mysql::Error) -> Self {
        VendorError::Db(Box::new(e))
    }
}

impl From<Box<dyn Error>> for VendorError {
    fn from(e: Box<dyn Error>) -> Self {
        VendorError::Db(e)
    }
}

/// Trim an optional field, turning a blank string into `None`.
fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

impl VendorInput {
    /// Trim every field and check the required ones. A GSTIN, when given, must
    /// be 15 letters/digits; it is upper-cased so lookups aren't case-sensitive.
    fn validated(self) -> Result<Self, VendorError> {
        let name = self.name.trim().to_string();
        if name.is_empty() {
            return Err(VendorError::InvalidInput("vendor name is required".into()));
        }
        let phone = self.phone.trim().to_string();
        if phone.is_empty() {
            return Err(VendorError::InvalidInput("vendor phone is required".into()));
        }
        let gstin = clean_optional(self.gstin).map(|g| g.to_ascii_uppercase());
        if let Some(g) = &gstin
            && (g.len() != 15 || !g.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return Err(VendorError::InvalidInput(
                "GSTIN must be 15 letters and digits".into(),
            ));
        }
        Ok(VendorInput {
            name,
            contact_person: clean_optional(self.contact_person),
            phone,
            gstin,
            notes: clean_optional(self.notes),
        })
    }
}

type VendorRow = (
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    bool,
);

const SELECT_COLUMNS: &str =
    "SELECT id, org_id, name, contact_person, phone, gstin, notes, is_active FROM VehicleVendors";

impl VehicleVendor {
    /// Create the `VehicleVendors` table if it does not exist. Kept in sync
    /// with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS VehicleVendors (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                name VARCHAR(255) NOT NULL,
                contact_person VARCHAR(255) DEFAULT NULL,
                phone VARCHAR(64) NOT NULL,
                gstin VARCHAR(15) DEFAULT NULL,
                notes TEXT DEFAULT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                CONSTRAINT fk_vendor_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    pub fn create(org_id: Uuid, input: VendorInput) -> Result<Self, VendorError> {
        let input = input.validated()?;
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let vendor = VehicleVendor {
            id: Uuid::new_v4(),
            org_id,
            name: input.name,
            contact_person: input.contact_person,
            phone: input.phone,
            gstin: input.gstin,
            notes: input.notes,
            is_active: true,
        };

        conn.exec_drop(
            "INSERT INTO VehicleVendors (id, org_id, name, contact_person, phone, gstin, notes, is_active)
             VALUES (:id, :org_id, :name, :contact_person, :phone, :gstin, :notes, :is_active)",
            params! {
                "id" => vendor.id.to_string(),
                "org_id" => vendor.org_id.to_string(),
                "name" => &vendor.name,
                "contact_person" => &vendor.contact_person,
                "phone" => &vendor.phone,
                "gstin" => &vendor.gstin,
                "notes" => &vendor.notes,
                "is_active" => vendor.is_active,
            },
        )?;

        Ok(vendor)
    }

    fn from_row(
        (id, org_id, name, contact_person, phone, gstin, notes, is_active): VendorRow,
    ) -> Self {
        VehicleVendor {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            name,
            contact_person,
            phone,
            gstin,
            notes,
            is_active,
        }
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<VendorRow> = conn.exec_first(
            format!("{SELECT_COLUMNS} WHERE id = :id"),
            params! { "id" => id.to_string() },
        )?;
        Ok(row.map(Self::from_row))
    }

    /// The org's vendors, active ones first, then by name.
    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<VendorRow> = conn.exec(
            format!("{SELECT_COLUMNS} WHERE org_id = :org_id ORDER BY is_active DESC, name"),
            params! { "org_id" => org_id.to_string() },
        )?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }

    pub fn update(&mut self, input: VendorInput, is_active: bool) -> Result<(), VendorError> {
        let input = input.validated()?;
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "UPDATE VehicleVendors
             SET name = :name, contact_person = :contact_person, phone = :phone,
                 gstin = :gstin, notes = :notes, is_active = :is_active
             WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "name" => &input.name,
                "contact_person" => &input.contact_person,
                "phone" => &input.phone,
                "gstin" => &input.gstin,
                "notes" => &input.notes,
                "is_active" => is_active,
            },
        )?;

        self.name = input.name;
        self.contact_person = input.contact_person;
        self.phone = input.phone;
        self.gstin = input.gstin;
        self.notes = input.notes;
        self.is_active = is_active;
        Ok(())
    }

    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "DELETE FROM VehicleVendors WHERE id = :id",
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

    fn make_org() -> Organization {
        Organization::create_organization("Vendor Test Org", "1 Depot Road").expect("create org")
    }

    fn input(name: &str, phone: &str) -> VendorInput {
        VendorInput {
            name: name.into(),
            phone: phone.into(),
            ..Default::default()
        }
    }

    #[test]
    fn test_create_get_and_list_vendor() {
        let _db = TestDb::create();
        let org = make_org();

        let v = VehicleVendor::create(
            org.id,
            VendorInput {
                name: "  Sharma Roadlines ".into(),
                contact_person: Some("Anil Sharma".into()),
                phone: "+91 98200 00000".into(),
                gstin: Some("27aapfu0939f1zv".into()),
                notes: Some("   ".into()),
            },
        )
        .expect("create vendor");
        assert!(v.is_active);
        assert_eq!(v.name, "Sharma Roadlines", "name is trimmed");
        assert_eq!(v.gstin.as_deref(), Some("27AAPFU0939F1ZV"), "GSTIN upper-cased");
        assert_eq!(v.notes, None, "blank notes stored as NULL");

        let fetched = VehicleVendor::get_by_id(v.id).expect("get").expect("exists");
        assert_eq!(fetched.contact_person.as_deref(), Some("Anil Sharma"));
        assert_eq!(fetched.org_id, org.id);

        VehicleVendor::create(org.id, input("Balaji Transport", "022 1234")).expect("second");
        let listed = VehicleVendor::list_by_org(org.id).expect("list");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "Balaji Transport"); // ORDER BY name
    }

    #[test]
    fn test_create_rejects_blank_name_phone_and_bad_gstin() {
        let _db = TestDb::create();
        let org = make_org();

        for (bad, why) in [
            (input(" ", "123"), "name"),
            (input("Vendor", "  "), "phone"),
            (
                VendorInput {
                    gstin: Some("SHORT".into()),
                    ..input("Vendor", "123")
                },
                "GSTIN",
            ),
        ] {
            match VehicleVendor::create(org.id, bad) {
                Err(VendorError::InvalidInput(msg)) => assert!(msg.contains(why), "{msg}"),
                other => panic!("expected InvalidInput for {why}, got {other:?}"),
            }
        }
        assert!(VehicleVendor::list_by_org(org.id).expect("list").is_empty());
    }

    #[test]
    fn test_update_vendor_and_inactive_sort_last() {
        let _db = TestDb::create();
        let org = make_org();
        let mut a = VehicleVendor::create(org.id, input("Alpha Carriers", "1")).expect("a");
        VehicleVendor::create(org.id, input("Zeta Logistics", "2")).expect("z");

        a.update(
            VendorInput {
                contact_person: Some("Priya".into()),
                ..input("Alpha Carriers Pvt", "+91 1")
            },
            false,
        )
        .expect("update");

        let fetched = VehicleVendor::get_by_id(a.id).expect("get").expect("exists");
        assert_eq!(fetched.name, "Alpha Carriers Pvt");
        assert_eq!(fetched.contact_person.as_deref(), Some("Priya"));
        assert!(!fetched.is_active);

        let names: Vec<String> = VehicleVendor::list_by_org(org.id)
            .expect("list")
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(names, ["Zeta Logistics", "Alpha Carriers Pvt"]);

        assert!(matches!(
            a.update(input("", "1"), true),
            Err(VendorError::InvalidInput(_))
        ));
    }

    #[test]
    fn test_delete_vendor_and_org_cascade() {
        let _db = TestDb::create();
        let org = make_org();
        let v = VehicleVendor::create(org.id, input("Gone", "1")).expect("create");
        v.delete().expect("delete");
        assert!(VehicleVendor::get_by_id(v.id).expect("get").is_none());

        VehicleVendor::create(org.id, input("Doomed", "2")).expect("create");
        org.remove_organization().expect("remove org");
        assert!(VehicleVendor::list_by_org(org.id).expect("list").is_empty());
    }
}
