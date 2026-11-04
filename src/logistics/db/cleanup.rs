//! Reduce a copy of the database to one organization. Run through
//! `src/bin/cleanup_db.rs`. See `docs/database-cleanup.md`.
//!
//! Everything happens in one transaction, and the checks run before it commits,
//! so a failed check leaves the database exactly as it was.

use mysql::prelude::*;
use mysql::*;

/// The default database name, which test runs and `cargo run` also write to.
/// This tool only runs on a copy, so that the original stays as the way back.
const REFUSED_DATABASE: &str = "logistics";

/// Tables without an `org_id`, reached through a parent: (table, column, parent).
const CHILD_TABLES: [(&str, &str, &str); 5] = [
    ("DispatchLineItems", "dispatch_id", "Dispatches"),
    ("DispatchStatusHistory", "dispatch_id", "Dispatches"),
    ("DispatchProofOfDelivery", "dispatch_id", "Dispatches"),
    ("Stock", "godown_id", "Godowns"),
    ("VendorPayments", "hire_id", "VehicleHires"),
];

#[derive(Debug, PartialEq, Eq)]
pub struct TableCount {
    pub table: String,
    pub kept: i64,
    pub removed: i64,
}

#[derive(Debug)]
pub struct CleanupReport {
    pub database: String,
    pub kept_org_id: String,
    pub tables: Vec<TableCount>,
    /// Files belonging to removed organizations, to delete once the result has
    /// proved itself. Never the uploads folder as a whole: it also holds the
    /// published Android app.
    pub upload_paths: Vec<String>,
    pub applied: bool,
}

fn sql_err(context: &str, e: impl std::fmt::Display) -> String {
    format!("{context}: {e}")
}

fn refuse_original(database: &str) -> Result<(), String> {
    if database == REFUSED_DATABASE {
        return Err(format!(
            "Refusing to run on `{REFUSED_DATABASE}`. Restore a dump under another name and run it there."
        ));
    }
    Ok(())
}

fn count(tx: &mut impl Queryable, sql: &str) -> Result<i64, String> {
    let n: Option<i64> = tx.query_first(sql).map_err(|e| sql_err(sql, e))?;
    Ok(n.unwrap_or(0))
}

/// Keep the organization named exactly `keep_name` and remove all others. Only
/// commits when `apply` is set; otherwise reports what would happen.
pub fn keep_only_org(
    conn: &mut PooledConn,
    keep_name: &str,
    apply: bool,
) -> Result<CleanupReport, String> {
    let database: Option<String> = conn
        .query_first("SELECT DATABASE()")
        .map_err(|e| sql_err("reading the database name", e))?;
    let database = database.ok_or("No database selected")?;
    refuse_original(&database)?;

    let matches: Vec<String> = conn
        .exec(
            "SELECT id FROM Orgs WHERE name = :name",
            params! { "name" => keep_name },
        )
        .map_err(|e| sql_err("finding the organization", e))?;
    let [keep_id] = matches.as_slice() else {
        return Err(format!(
            "Expected exactly one organization named {keep_name:?}, found {}. Nothing changed.",
            matches.len()
        ));
    };
    // Only ever a UUID from our own table, and these statements can't take a
    // table name as a parameter anyway.
    let keep_id = keep_id.clone();

    let all_tables: Vec<String> = conn
        .query(
            "SELECT table_name FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE'",
        )
        .map_err(|e| sql_err("listing tables", e))?;
    let org_tables: Vec<String> = conn
        .query(
            "SELECT c.table_name FROM information_schema.columns c
             JOIN information_schema.tables t
               ON t.table_schema = c.table_schema AND t.table_name = c.table_name
             WHERE c.table_schema = DATABASE() AND c.column_name = 'org_id'
               AND t.table_type = 'BASE TABLE'
             ORDER BY c.table_name",
        )
        .map_err(|e| sql_err("listing org tables", e))?;
    let child_tables: Vec<(&str, &str, &str)> = CHILD_TABLES
        .into_iter()
        .filter(|(table, _, parent)| {
            all_tables.iter().any(|t| t == table)
                && all_tables.iter().any(|t| t == parent)
                // A newer schema may give it an org_id, and then it's covered above.
                && !org_tables.iter().any(|t| t == table)
        })
        .collect();

    let upload_paths: Vec<String> = if all_tables.iter().any(|t| t == "UploadedFiles") {
        conn.exec(
            "SELECT storage_path FROM UploadedFiles WHERE org_id <> :keep",
            params! { "keep" => &keep_id },
        )
        .map_err(|e| sql_err("listing uploads", e))?
    } else {
        Vec::new()
    };

    let mut tx = conn
        .start_transaction(TxOpts::default())
        .map_err(|e| sql_err("starting the transaction", e))?;

    // What the kept org has now, to compare afterwards. Totals as well, for the report.
    let mut before: Vec<(String, i64, i64)> = Vec::new();
    for table in &org_tables {
        let kept = count(
            &mut tx,
            &format!("SELECT COUNT(*) FROM `{table}` WHERE org_id = '{keep_id}'"),
        )?;
        let total = count(&mut tx, &format!("SELECT COUNT(*) FROM `{table}`"))?;
        before.push((table.clone(), kept, total));
    }
    let mut children_kept: Vec<(&str, i64, i64)> = Vec::new();
    for (table, column, parent) in &child_tables {
        let kept = count(
            &mut tx,
            &format!(
                "SELECT COUNT(*) FROM `{table}` c JOIN `{parent}` p ON c.`{column}` = p.id
                 WHERE p.org_id = '{keep_id}'"
            ),
        )?;
        let total = count(&mut tx, &format!("SELECT COUNT(*) FROM `{table}`"))?;
        children_kept.push((table, kept, total));
    }
    let orgs_total = count(&mut tx, "SELECT COUNT(*) FROM Orgs")?;

    // Every table directly: some have no foreign key to Orgs, and a long-lived
    // database may lack keys that a fresh one has.
    for table in &org_tables {
        let sql = format!("DELETE FROM `{table}` WHERE org_id <> '{keep_id}'");
        tx.query_drop(&sql).map_err(|e| sql_err(&sql, e))?;
    }
    for (table, column, parent) in &child_tables {
        let sql =
            format!("DELETE FROM `{table}` WHERE `{column}` NOT IN (SELECT id FROM `{parent}`)");
        tx.query_drop(&sql).map_err(|e| sql_err(&sql, e))?;
    }
    let sql = format!("DELETE FROM Orgs WHERE id <> '{keep_id}'");
    tx.query_drop(&sql).map_err(|e| sql_err(&sql, e))?;

    // Checks. Returning early drops the transaction, which rolls it back.
    if count(&mut tx, "SELECT COUNT(*) FROM Orgs")? != 1 {
        return Err("Check failed: more than one organization left. Nothing changed.".into());
    }
    let mut tables = vec![TableCount {
        table: "Orgs".to_string(),
        kept: 1,
        removed: orgs_total - 1,
    }];
    for (table, kept, total) in before {
        let after = count(&mut tx, &format!("SELECT COUNT(*) FROM `{table}`"))?;
        if after != kept {
            return Err(format!(
                "Check failed: {table} should hold {kept} rows for the kept organization, \
                 but holds {after}. Nothing changed."
            ));
        }
        tables.push(TableCount {
            table,
            kept,
            removed: total - kept,
        });
    }
    for (table, kept, total) in children_kept {
        let after = count(&mut tx, &format!("SELECT COUNT(*) FROM `{table}`"))?;
        if after != kept {
            return Err(format!(
                "Check failed: {table} should hold {kept} rows for the kept organization, \
                 but holds {after}. Nothing changed."
            ));
        }
        tables.push(TableCount {
            table: table.to_string(),
            kept,
            removed: total - kept,
        });
    }

    if apply {
        tx.commit().map_err(|e| sql_err("committing", e))?;
    } else {
        tx.rollback().map_err(|e| sql_err("rolling back", e))?;
    }
    Ok(CleanupReport {
        database,
        kept_org_id: keep_id,
        tables,
        upload_paths,
        applied: apply,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::auth::auth::OrgCredentials;
    use crate::logistics::db::connection::DbConnection;
    use crate::logistics::dispatch::dispatch::{
        DispatchOrder, DispatchStatus, DispatchStatusEvent,
    };
    use crate::logistics::godown::godown::Godown;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::TestDb;
    use uuid::Uuid;

    fn connect() -> PooledConn {
        DbConnection::from_env().get_connection().expect("connect")
    }

    fn rows(conn: &mut PooledConn, sql: &str) -> i64 {
        conn.query_first::<i64, _>(sql).unwrap().unwrap()
    }

    fn dispatch_for(org_id: Uuid) -> DispatchOrder {
        let mut dispatch = DispatchOrder {
            id: Uuid::new_v4(),
            org_id,
            customer_id: Uuid::new_v4(),
            vehicle_registration_number: None,
            vehicle_source: Default::default(),
            hire_id: None,
            line_items: Vec::new(),
            status: DispatchStatus::Pending,
            dispatched_at: 1,
            status_history: vec![DispatchStatusEvent {
                status: DispatchStatus::Pending,
                changed_at: 1,
            }],
            proof_of_delivery: None,
            trip_id: None,
            stop_sequence: None,
        };
        dispatch.save().expect("dispatch");
        dispatch
    }

    fn org_with_data(name: &str) -> Organization {
        let org = Organization::create_organization(name, "1 Depot Rd").expect("org");
        OrgCredentials::create(org.id, name, "password123").expect("credentials");
        dispatch_for(org.id);
        Godown::create(org.id, "Godown", "1 Rd", None).expect("godown");
        org
    }

    fn add_upload(conn: &mut PooledConn, org_id: Uuid, path: &str) {
        conn.exec_drop(
            "INSERT INTO UploadedFiles (id, org_id, content_type, byte_size, storage_path, uploaded_at)
             VALUES (:id, :org_id, 'image/png', 1, :path, 1)",
            params! { "id" => Uuid::new_v4().to_string(), "org_id" => org_id.to_string(), "path" => path },
        )
        .unwrap();
    }

    #[test]
    fn keeps_the_named_org_and_removes_every_trace_of_the_others() {
        let _db = TestDb::create();
        let keep = org_with_data("Keep Me");
        let other = org_with_data("Test Org abc");
        let mut conn = connect();
        add_upload(&mut conn, other.id, "/uploads/other.png");
        add_upload(&mut conn, keep.id, "/uploads/keep.png");

        let report = keep_only_org(&mut conn, "Keep Me", true).expect("cleanup");

        assert!(report.applied);
        assert_eq!(report.kept_org_id, keep.id.to_string());
        assert_eq!(report.upload_paths, ["/uploads/other.png"]);
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Orgs"), 1);
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Dispatches"), 1);
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM OrgCredentials"), 1);
        assert_eq!(
            rows(&mut conn, "SELECT COUNT(*) FROM DispatchStatusHistory"),
            1
        );
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Godowns"), 1);
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM UploadedFiles"), 1);
        assert_eq!(
            OrgCredentials::verify_login(keep.id, "password123")
                .unwrap()
                .as_deref(),
            Some("Keep Me")
        );
        let dispatches = report
            .tables
            .iter()
            .find(|t| t.table == "Dispatches")
            .unwrap();
        assert_eq!((dispatches.kept, dispatches.removed), (1, 1));
    }

    #[test]
    fn also_removes_rows_left_behind_by_earlier_org_deletions() {
        let _db = TestDb::create();
        org_with_data("Keep Me");
        // What the old delete route left: rows for an org that no longer exists.
        let gone = Uuid::new_v4();
        dispatch_for(gone);
        OrgCredentials::create(gone, "Deleted Org", "password123").unwrap();
        let mut conn = connect();

        keep_only_org(&mut conn, "Keep Me", true).expect("cleanup");

        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Dispatches"), 1);
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM OrgCredentials"), 1);
        assert_eq!(
            rows(&mut conn, "SELECT COUNT(*) FROM DispatchStatusHistory"),
            1
        );
    }

    #[test]
    fn without_apply_it_reports_and_changes_nothing() {
        let _db = TestDb::create();
        org_with_data("Keep Me");
        org_with_data("Other");
        let mut conn = connect();

        let report = keep_only_org(&mut conn, "Keep Me", false).expect("dry run");

        assert!(!report.applied);
        let orgs = report.tables.iter().find(|t| t.table == "Orgs").unwrap();
        assert_eq!((orgs.kept, orgs.removed), (1, 1));
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Orgs"), 2);
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Dispatches"), 2);
    }

    #[test]
    fn refuses_a_name_that_matches_nothing_or_more_than_one_org() {
        let _db = TestDb::create();
        org_with_data("Twin");
        org_with_data("Twin");
        let mut conn = connect();

        for name in ["Twin", "Nobody"] {
            let err = keep_only_org(&mut conn, name, true).expect_err("should refuse");
            assert!(err.contains("Nothing changed"), "{err}");
        }
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Orgs"), 2);
    }

    #[test]
    fn the_name_must_match_exactly() {
        let _db = TestDb::create();
        org_with_data("Mauli Brickvision AAC");
        let mut conn = connect();

        assert!(keep_only_org(&mut conn, "Mauli", true).is_err());
        assert_eq!(rows(&mut conn, "SELECT COUNT(*) FROM Orgs"), 1);
    }

    #[test]
    fn refuses_the_original_database() {
        assert!(refuse_original("logistics").is_err());
        assert!(refuse_original("logistics_live").is_ok());
    }
}
