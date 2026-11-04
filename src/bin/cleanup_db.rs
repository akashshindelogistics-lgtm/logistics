//! Reduce a restored copy of the database to one organization. Reports what it
//! would do unless given `--apply`. See docs/database-cleanup.md.
//!
//!     DATABASE_URL=mysql://user:pass@host:3306/copy_name cargo run --bin cleanup_db -- \
//!         --keep "Org Name" --uploads-list /path/to/files.txt [--apply]

use logistics_system::logistics::db::cleanup::keep_only_org;
use logistics_system::logistics::db::connection::DbConnection;
use std::process::ExitCode;

fn value_of(args: &[String], flag: &str) -> Option<String> {
    let position = args.iter().position(|a| a == flag)?;
    args.get(position + 1).cloned()
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let keep = value_of(&args, "--keep").ok_or("--keep \"Org Name\" is required")?;
    let uploads_list =
        value_of(&args, "--uploads-list").ok_or("--uploads-list PATH is required")?;
    let apply = args.iter().any(|a| a == "--apply");
    // No falling back to the default database for something this destructive.
    if std::env::var("DATABASE_URL").is_err() {
        return Err("DATABASE_URL must name the copy to clean".to_string());
    }

    let mut conn = DbConnection::from_env()
        .get_connection()
        .map_err(|e| format!("connecting: {e}"))?;
    let report = keep_only_org(&mut conn, &keep, apply)?;

    println!("Database: {}", report.database);
    println!("Keeping:  {keep} ({})", report.kept_org_id);
    println!("{:<28}{:>10}{:>10}", "Table", "Kept", "Removed");
    for t in &report.tables {
        println!("{:<28}{:>10}{:>10}", t.table, t.kept, t.removed);
    }
    std::fs::write(&uploads_list, report.upload_paths.join("\n"))
        .map_err(|e| format!("writing {uploads_list}: {e}"))?;
    println!(
        "{} upload paths written to {uploads_list}",
        report.upload_paths.len()
    );
    println!(
        "{}",
        if report.applied {
            "Checks passed. Applied."
        } else {
            "Checks passed. Nothing changed: run again with --apply."
        }
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
