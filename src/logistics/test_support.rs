//! Test-only helpers for provisioning and flushing the MySQL test database.
//!
//! Every unit and integration test in this crate runs against a real MySQL
//! server, and they all share one database. To keep tests deterministic and
//! independent of each other and of previous runs:
//!
//! * When the crate is built for tests (`cfg(test)`), [`DbConnection`] targets
//!   the `logistics_test` database instead of `logistics`, so `cargo test`
//!   never touches the data a developer's `cargo run` writes.
//! * Every test that reads or writes MySQL is annotated
//!   `#[serial_test::serial(db)]` and calls [`reset_database`] as its first
//!   line, which recreates the schema and truncates every table back to empty.
//!
//! See `docs/testing-database.md` for the rationale and the planned follow-up
//! phases (env-driven config, database-per-test).

use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;

/// Every table in the schema. Order is irrelevant because [`reset_database`]
/// truncates with `FOREIGN_KEY_CHECKS` disabled.
const TABLES: &[&str] = &[
    "Dispatches",
    "OrgCredentials",
    "Stock",
    "Vehicle",
    "Customers",
    "Orgs",
];

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
        "CREATE TABLE IF NOT EXISTS Vehicle (
            registration_number VARCHAR(255) PRIMARY KEY,
            capacity BIGINT NOT NULL,
            unit VARCHAR(50) NOT NULL,
            org_id VARCHAR(36) NOT NULL,
            latitude DOUBLE DEFAULT NULL,
            longitude DOUBLE DEFAULT NULL,
            last_updated_at BIGINT DEFAULT NULL,
            location_address VARCHAR(255) DEFAULT NULL,
            CONSTRAINT fk_vehicle_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create Vehicle");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Stock (
            id INT AUTO_INCREMENT PRIMARY KEY,
            volume_in_size BIGINT NOT NULL,
            quantity BIGINT NOT NULL,
            description VARCHAR(255) NOT NULL,
            org_id VARCHAR(36) NOT NULL,
            CONSTRAINT fk_stock_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
    )
    .expect("migrate: create Stock");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS Customers (
            id VARCHAR(36) PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            address VARCHAR(255) NOT NULL,
            latitude DOUBLE DEFAULT NULL,
            longitude DOUBLE DEFAULT NULL,
            last_updated_at BIGINT DEFAULT NULL,
            location_address VARCHAR(255) DEFAULT NULL
        )",
    )
    .expect("migrate: create Customers");

    conn.query_drop(
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
    )
    .expect("migrate: create Dispatches");

    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS OrgCredentials (
            org_id VARCHAR(36) PRIMARY KEY,
            org_name VARCHAR(255) NOT NULL,
            password_hash VARCHAR(255) NOT NULL
        )",
    )
    .expect("migrate: create OrgCredentials");
}

/// Truncate every table so the calling test starts from a known-empty database.
///
/// Call this as the very first line of every test that reads or writes MySQL,
/// and annotate that test with `#[serial_test::serial(db)]` so a reset never
/// races a concurrent test sharing the same database.
pub fn reset_database() {
    let mut conn = DbConnection::new("localhost", 3306, "logistics", "root", "password")
        .get_connection()
        .expect("reset_database: connect to test database");

    migrate(&mut conn);

    conn.query_drop("SET FOREIGN_KEY_CHECKS = 0")
        .expect("reset_database: disable FK checks");
    for table in TABLES {
        conn.query_drop(format!("TRUNCATE TABLE `{table}`"))
            .unwrap_or_else(|e| panic!("reset_database: truncate {table}: {e}"));
    }
    conn.query_drop("SET FOREIGN_KEY_CHECKS = 1")
        .expect("reset_database: re-enable FK checks");
}
