//! Test-only helpers for provisioning an isolated MySQL database per test.
//!
//! Every unit and integration test in this crate talks to a real MySQL
//! server. [`TestDb::create`] gives each test its own uniquely-named
//! database (`logistics_test_<uuid>`), migrated and ready to use, and
//! overrides [`DbConfig::from_env`] on the calling thread so every
//! `DbConnection::from_env()` call made by that test resolves to it —
//! without mutating process-wide environment variables, which would race
//! every other test thread reading them concurrently. Dropping the guard
//! drops the database and clears the override.
//!
//! ```ignore
//! #[test]
//! fn test_something() {
//!     let _db = TestDb::create();
//!     // ... exercise the app; every DB call in this test's thread lands
//!     // in this test's private database ...
//! }
//! ```
//!
//! Because each test owns its own database, tests no longer need to
//! serialize against each other (no more `#[serial_test::serial(db)]`) and
//! `cargo test` runs at full parallelism again. See
//! `docs/testing-database.md` (Phase 3) for the history: an earlier phase
//! shared one `logistics_test` database across all tests, serialized with
//! `serial_test` and a `reset_database()` truncate-everything call at the
//! top of each test.

use crate::logistics::db::connection::{self, DbConfig, DbConnection};
use mysql::prelude::*;
use uuid::Uuid;

/// Create every table the application uses, if it does not already exist.
///
/// This is the single source of truth for the schema as tests expect it.
/// Production code still creates each table lazily on first write (the
/// `ensure_*` / inline `CREATE TABLE IF NOT EXISTS` helpers in the domain
/// modules); those definitions and this one must stay in sync.
pub fn migrate(conn: &mut mysql::PooledConn) {
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Orgs (
            id VARCHAR(36) PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            address VARCHAR(255) NOT NULL,
            latitude DOUBLE DEFAULT NULL,
            longitude DOUBLE DEFAULT NULL,
            last_updated_at BIGINT DEFAULT NULL,
            location_address VARCHAR(255) DEFAULT NULL
        )",
    )
    .expect("migrate: create Orgs");

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
    )
    .expect("migrate: create Drivers");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Vehicle (
            registration_number VARCHAR(255) PRIMARY KEY,
            capacity BIGINT NOT NULL,
            unit VARCHAR(50) NOT NULL,
            org_id VARCHAR(36) NOT NULL,
            assigned_driver_id VARCHAR(36) DEFAULT NULL,
            latitude DOUBLE DEFAULT NULL,
            longitude DOUBLE DEFAULT NULL,
            last_updated_at BIGINT DEFAULT NULL,
            location_address VARCHAR(255) DEFAULT NULL,
            CONSTRAINT fk_vehicle_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create Vehicle");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Godowns (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            name VARCHAR(255) NOT NULL,
            address VARCHAR(255) NOT NULL,
            max_capacity BIGINT DEFAULT NULL,
            latitude DOUBLE DEFAULT NULL,
            longitude DOUBLE DEFAULT NULL,
            last_updated_at BIGINT DEFAULT NULL,
            location_address VARCHAR(255) DEFAULT NULL,
            CONSTRAINT fk_godown_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create Godowns");

    // Transitional: stock used to reference Orgs directly. Databases created
    // before godowns still have that column; there is no hosted backend and
    // dev/test stock is disposable, so drop and recreate the table rather than
    // carry a real data migration. See docs/godowns.md.
    let legacy_stock: Option<i64> = conn
        .exec_first(
            "SELECT 1 FROM information_schema.columns
             WHERE table_schema = DATABASE() AND table_name = 'Stock' AND column_name = 'org_id'",
            (),
        )
        .expect("migrate: probe legacy Stock schema");
    if legacy_stock.is_some() {
        conn.query_drop("DROP TABLE IF EXISTS Stock")
            .expect("migrate: drop legacy Stock");
    }

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
    )
    .expect("migrate: create Stock");

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
    )
    .expect("migrate: create StockTransfers");

    // Transitional: customers used to be a flat, org-less table. Databases
    // created before customers were scoped to an org still have that column
    // set; dev/test customers are disposable, so drop and recreate rather than
    // carry a real data migration. See docs/customers.md.
    let legacy_customers: Option<i64> = conn
        .exec_first(
            "SELECT 1 FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name = 'Customers'",
            (),
        )
        .expect("migrate: probe Customers table");
    if legacy_customers.is_some() {
        let has_org_id: Option<i64> = conn
            .exec_first(
                "SELECT 1 FROM information_schema.columns
                 WHERE table_schema = DATABASE() AND table_name = 'Customers' AND column_name = 'org_id'",
                (),
            )
            .expect("migrate: probe legacy Customers schema");
        if has_org_id.is_none() {
            conn.query_drop("DROP TABLE Customers")
                .expect("migrate: drop legacy Customers");
        }
    }

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
    )
    .expect("migrate: create VehicleDocuments");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Customers (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            name VARCHAR(255) NOT NULL,
            address VARCHAR(255) NOT NULL,
            latitude DOUBLE DEFAULT NULL,
            longitude DOUBLE DEFAULT NULL,
            last_updated_at BIGINT DEFAULT NULL,
            location_address VARCHAR(255) DEFAULT NULL,
            CONSTRAINT fk_customer_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create Customers");

    // Transitional: a dispatch used to carry exactly one stock line
    // (`Dispatches.stock_description` + `quantity`); it now carries a list of
    // line items in `DispatchLineItems`. Dev/test dispatches are disposable,
    // so drop and recreate the dispatch tables when the old columns are
    // present rather than carry a data migration. See docs/dispatch-lifecycle.md.
    crate::logistics::dispatch::dispatch::drop_legacy_single_line_dispatch_tables(conn)
        .expect("migrate: drop legacy single-line dispatch tables");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Dispatches (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            customer_id VARCHAR(36) NOT NULL,
            vehicle_registration_number VARCHAR(255) NOT NULL,
            status VARCHAR(50) NOT NULL,
            dispatched_at BIGINT NOT NULL
        )",
    )
    .expect("migrate: create Dispatches");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS DispatchLineItems (
            id INT AUTO_INCREMENT PRIMARY KEY,
            dispatch_id VARCHAR(36) NOT NULL,
            stock_description VARCHAR(255) NOT NULL,
            quantity BIGINT NOT NULL,
            volume_in_size BIGINT NOT NULL,
            CONSTRAINT fk_dispatch_line_item_dispatch
                FOREIGN KEY (dispatch_id) REFERENCES Dispatches(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create DispatchLineItems");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS DispatchStatusHistory (
            id INT AUTO_INCREMENT PRIMARY KEY,
            dispatch_id VARCHAR(36) NOT NULL,
            status VARCHAR(50) NOT NULL,
            changed_at BIGINT NOT NULL,
            CONSTRAINT fk_dispatch_status_history_dispatch
                FOREIGN KEY (dispatch_id) REFERENCES Dispatches(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create DispatchStatusHistory");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS DispatchProofOfDelivery (
            dispatch_id VARCHAR(36) PRIMARY KEY,
            receiver_name VARCHAR(255) NOT NULL,
            signature_or_photo_url TEXT NOT NULL,
            delivered_at BIGINT NOT NULL,
            CONSTRAINT fk_dispatch_pod_dispatch
                FOREIGN KEY (dispatch_id) REFERENCES Dispatches(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create DispatchProofOfDelivery");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Invoices (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            dispatch_id VARCHAR(36) NOT NULL UNIQUE,
            customer_id VARCHAR(36) NOT NULL,
            amount BIGINT NOT NULL,
            issued_on VARCHAR(10) NOT NULL,
            due_on VARCHAR(10) NOT NULL,
            paid_on VARCHAR(10) DEFAULT NULL,
            CONSTRAINT fk_invoice_org
                FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE,
            CONSTRAINT fk_invoice_dispatch
                FOREIGN KEY (dispatch_id) REFERENCES Dispatches(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create Invoices");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS OrgCredentials (
            org_id VARCHAR(36) PRIMARY KEY,
            org_name VARCHAR(255) NOT NULL,
            password_hash VARCHAR(255) NOT NULL
        )",
    )
    .expect("migrate: create OrgCredentials");
}

/// A private, uniquely-named MySQL database scoped to exactly one test.
///
/// Hold the guard for the duration of the test (`let _db = TestDb::create();`
/// as the first line is the usual pattern). While it is alive, every
/// `DbConnection::from_env()` call made on the *same thread* resolves to
/// this database — safe under `cargo test`'s default parallelism because
/// libtest gives each test its own OS thread, and `#[actix_web::test]`
/// (`actix_rt::System::new().block_on(...)`) runs its whole async body on a
/// single-threaded executor pinned to that same thread, so the override
/// stays valid across every `.await` point too.
///
/// Dropping the guard drops the database and clears the override. A test
/// that panics still runs `Drop` (the database is dropped as the panic
/// unwinds), so a failure doesn't leak it; only a hard process abort (e.g. a
/// segfault, or `cargo test` being killed) would leave one behind — a
/// `logistics_test_%` database left over from a crashed run is safe to drop
/// by hand.
pub struct TestDb {
    name: String,
}

impl TestDb {
    /// Provision a fresh `logistics_test_<uuid>` database, migrate it, and
    /// start overriding `DbConfig::from_env()` on this thread to target it.
    pub fn create() -> Self {
        let name = format!("logistics_test_{}", Uuid::new_v4().simple());
        connection::set_test_db_override(Some(name.clone()));

        let mut conn = DbConnection::from_env()
            .get_connection()
            .expect("TestDb::create: connect to fresh test database");
        migrate(&mut conn);

        Self { name }
    }

    /// The database's name, e.g. for a diagnostic message.
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // The override is still active for this thread at this point, so
        // `from_env()` resolves back to `self.name` — reuse it rather than
        // hand-building a config, then tear everything down.
        if let Ok(mut conn) = DbConnection::from_env().get_connection() {
            let _ = conn.query_drop(format!("DROP DATABASE IF EXISTS `{}`", self.name));
        }
        connection::drop_pool(&self.name);
        connection::set_test_db_override(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_provisions_a_reachable_migrated_database() {
        let db = TestDb::create();
        assert!(db.name().starts_with("logistics_test_"));

        let mut conn = DbConnection::from_env()
            .get_connection()
            .expect("should connect to the TestDb-provisioned database");
        let count: Option<i64> = conn
            .exec_first("SELECT COUNT(*) FROM Orgs", ())
            .expect("Orgs table should exist after TestDb::create");
        assert_eq!(count, Some(0));
    }

    #[test]
    fn drop_removes_the_database_and_the_override() {
        let db = TestDb::create();
        let name = db.name().to_string();
        drop(db);

        assert_eq!(DbConfig::from_env().db_name, "logistics_test");

        let mut admin = DbConnection::from_env()
            .get_connection()
            .expect("connect to default test database to check cleanup");
        let exists: Option<String> = admin
            .exec_first(
                "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?",
                (name,),
            )
            .expect("query information_schema for the dropped database");
        assert!(exists.is_none(), "dropped TestDb database should be gone");
    }

    #[test]
    fn two_test_dbs_on_the_same_thread_are_independent() {
        let db_a = TestDb::create();
        let mut conn_a = DbConnection::from_env().get_connection().unwrap();
        conn_a
            .query_drop(
                "INSERT INTO Orgs (id, name, address) VALUES ('11111111-1111-1111-1111-111111111111', 'A', 'A')",
            )
            .unwrap();
        drop(db_a);

        // A fresh TestDb, still on this same thread, must not see the row
        // written to the previous one.
        let _db_b = TestDb::create();
        let mut conn_b = DbConnection::from_env().get_connection().unwrap();
        let count: Option<i64> = conn_b.exec_first("SELECT COUNT(*) FROM Orgs", ()).unwrap();
        assert_eq!(count, Some(0));
    }
}
