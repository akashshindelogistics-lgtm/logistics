use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use uuid::Uuid;

/// A driver employed by an organization. A vehicle must have an **active**
/// driver assigned to it (see `Vehicle::assigned_driver_id`) before that
/// vehicle can be picked for a dispatch — see
/// [`Organization::dispatch_stock_to_customer`](crate::logistics::orgs::orgs::Organization::dispatch_stock_to_customer).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Driver {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub license_number: String,
    pub phone: String,
    /// Whether the driver is currently available to run a trip. Toggled via
    /// `PUT /api/drivers/{id}`. A vehicle whose assigned driver is inactive
    /// (or unset) is skipped when selecting a vehicle for dispatch.
    pub is_active: bool,
}

impl Driver {
    /// Create the `Drivers` table if it does not exist. Kept in sync with
    /// `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS Drivers (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                name VARCHAR(255) NOT NULL,
                license_number VARCHAR(255) NOT NULL,
                phone VARCHAR(64) NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                CONSTRAINT fk_driver_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    pub fn create(
        org_id: Uuid,
        name: impl Into<String>,
        license_number: impl Into<String>,
        phone: impl Into<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let driver = Driver {
            id: Uuid::new_v4(),
            org_id,
            name: name.into(),
            license_number: license_number.into(),
            phone: phone.into(),
            is_active: true,
        };

        conn.exec_drop(
            "INSERT INTO Drivers (id, org_id, name, license_number, phone, is_active)
             VALUES (:id, :org_id, :name, :license_number, :phone, :is_active)",
            params! {
                "id" => driver.id.to_string(),
                "org_id" => driver.org_id.to_string(),
                "name" => &driver.name,
                "license_number" => &driver.license_number,
                "phone" => &driver.phone,
                "is_active" => driver.is_active,
            },
        )?;

        Ok(driver)
    }

    fn row_to_driver(
        id: String,
        org_id: String,
        name: String,
        license_number: String,
        phone: String,
        is_active: bool,
    ) -> Self {
        Driver {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            name,
            license_number,
            phone,
            is_active,
        }
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<(String, String, String, String, String, bool)> = conn.exec_first(
            "SELECT id, org_id, name, license_number, phone, is_active FROM Drivers WHERE id = :id",
            params! { "id" => id.to_string() },
        )?;

        Ok(row.map(|(id, org_id, name, license_number, phone, is_active)| {
            Self::row_to_driver(id, org_id, name, license_number, phone, is_active)
        }))
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<(String, String, String, String, String, bool)> = conn.exec_map(
            "SELECT id, org_id, name, license_number, phone, is_active FROM Drivers WHERE org_id = :org_id ORDER BY name",
            params! { "org_id" => org_id.to_string() },
            |(id, org_id, name, license_number, phone, is_active)| {
                (id, org_id, name, license_number, phone, is_active)
            },
        )?;

        Ok(rows
            .into_iter()
            .map(|(id, org_id, name, license_number, phone, is_active)| {
                Self::row_to_driver(id, org_id, name, license_number, phone, is_active)
            })
            .collect())
    }

    pub fn update(
        &mut self,
        name: impl Into<String>,
        license_number: impl Into<String>,
        phone: impl Into<String>,
        is_active: bool,
    ) -> Result<(), Box<dyn Error>> {
        let name = name.into();
        let license_number = license_number.into();
        let phone = phone.into();

        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "UPDATE Drivers SET name = :name, license_number = :license_number, phone = :phone, is_active = :is_active WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "name" => &name,
                "license_number" => &license_number,
                "phone" => &phone,
                "is_active" => is_active,
            },
        )?;

        self.name = name;
        self.license_number = license_number;
        self.phone = phone;
        self.is_active = is_active;
        Ok(())
    }

    /// Delete the driver and clear the assignment from any vehicle that
    /// pointed at them (there is no DB-level FK from `Vehicle` to `Drivers`).
    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "UPDATE Vehicle SET assigned_driver_id = NULL WHERE assigned_driver_id = :id",
            params! { "id" => self.id.to_string() },
        )?;
        conn.exec_drop(
            "DELETE FROM Drivers WHERE id = :id",
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

    fn make_org() -> Organization {
        Organization::create_organization("Driver Test Org", "1 Depot Road").expect("create org")
    }

    #[test]
    fn test_create_get_and_list_driver() {
        let _db = TestDb::create();
        let org = make_org();

        let d = Driver::create(org.id, "Ravi Kumar", "DL-1420110012345", "+91 98100 00000")
            .expect("create driver");
        assert!(d.is_active);

        let fetched = Driver::get_by_id(d.id).expect("get").expect("exists");
        assert_eq!(fetched.name, "Ravi Kumar");
        assert_eq!(fetched.license_number, "DL-1420110012345");
        assert_eq!(fetched.org_id, org.id);

        Driver::create(org.id, "Sunita Rao", "MH-1220220054321", "+91 99000 11111")
            .expect("create second driver");
        let listed = Driver::list_by_org(org.id).expect("list");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "Ravi Kumar"); // ORDER BY name
    }

    #[test]
    fn test_update_driver_toggles_active() {
        let _db = TestDb::create();
        let org = make_org();
        let mut d = Driver::create(org.id, "Old Name", "OLD-LIC", "000").expect("create");

        d.update("New Name", "NEW-LIC", "+91 90000 90000", false)
            .expect("update");

        let fetched = Driver::get_by_id(d.id).expect("get").expect("exists");
        assert_eq!(fetched.name, "New Name");
        assert_eq!(fetched.license_number, "NEW-LIC");
        assert!(!fetched.is_active);
    }

    #[test]
    fn test_delete_driver_clears_vehicle_assignment() {
        let _db = TestDb::create();
        let org = make_org();
        let d = Driver::create(org.id, "Assigned Driver", "LIC-1", "111").expect("create driver");

        let mut v = Vehicle::new("KA01 AA 0001", 10, Unit::MetricTon);
        v.add_new_vehicle_to_org(&org).expect("add vehicle");
        v.assign_driver(Some(d.id)).expect("assign driver");

        d.delete().expect("delete driver");

        assert!(Driver::get_by_id(d.id).expect("get").is_none());
        let reloaded = Organization::get_by_id(org.id)
            .expect("get org")
            .expect("org")
            .vehicles
            .into_iter()
            .find(|x| x.registration_number == "KA01 AA 0001")
            .expect("vehicle present");
        assert_eq!(reloaded.assigned_driver_id, None);
    }

    #[test]
    fn test_deleting_org_cascades_to_drivers() {
        let _db = TestDb::create();
        let org = make_org();
        Driver::create(org.id, "Doomed", "LIC", "000").expect("create");
        org.remove_organization().expect("remove org");
        assert!(Driver::list_by_org(org.id).expect("list").is_empty());
    }
}
