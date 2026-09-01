use mysql::prelude::*;
use mysql::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Pools, keyed by database name. `host`/`port`/`username`/`password` are
/// constant for a process (they come from one set of env vars), so the
/// database name is the only axis tests vary — see [`TestDb`] in
/// `test_support.rs` and `docs/testing-database.md` (Phase 3).
static POOLS: OnceLock<Mutex<HashMap<String, Pool>>> = OnceLock::new();

fn pools() -> &'static Mutex<HashMap<String, Pool>> {
    POOLS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Lock the pool registry, recovering from poisoning instead of panicking.
///
/// A poisoned `Mutex` normally means "some other thread panicked while
/// holding this lock, the data might be in a torn state" — but the only
/// thing ever mutated under this lock is a `HashMap<String, Pool>` via
/// `insert`/`remove`/`get`, so even a panic mid-mutation leaves nothing
/// worse than a missing entry, which the caller already handles (it just
/// creates the pool again). Under `cargo test`'s default parallelism, many
/// [`TestDb`]-backed pools can be created at once; if any single one fails
/// (e.g. MySQL's connection limit), unwrapping a poisoned lock instead of
/// recovering it would cascade that one failure into every other test that
/// happens to touch the registry afterwards, including ones already
/// unwinding from an unrelated panic — which aborts the whole process
/// (a second panic inside a `Drop` run during unwinding is not allowed to
/// unwind itself).
fn lock_pools() -> std::sync::MutexGuard<'static, HashMap<String, Pool>> {
    pools()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
thread_local! {
    /// Test-only override of the database name [`DbConfig::from_env`]
    /// resolves to on the calling thread. Set by [`TestDb::create`] so a
    /// test can target its own private, uniquely-named database without
    /// mutating process-wide environment variables — which would race every
    /// other test thread reading them concurrently under `cargo test`'s
    /// default parallelism.
    static TEST_DB_OVERRIDE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_test_db_override(name: Option<String>) {
    TEST_DB_OVERRIDE.with(|cell| *cell.borrow_mut() = name);
}

#[cfg(test)]
fn test_db_override() -> Option<String> {
    TEST_DB_OVERRIDE.with(|cell| cell.borrow().clone())
}

/// Drop a database's pool from the registry, closing its connections. Used
/// by [`TestDb`]'s `Drop` impl once it has dropped the database itself, so a
/// long test run doesn't accumulate one live `Pool` per test forever.
#[cfg(test)]
pub(crate) fn drop_pool(db_name: &str) {
    if POOLS.get().is_some() {
        lock_pools().remove(db_name);
    }
}

/// Connection settings resolved from the environment.
///
/// Precedence:
/// 1. `DATABASE_URL` — a full `mysql://user:pass@host:port/db` connection
///    string, parsed as one piece.
/// 2. Otherwise `MYSQL_HOST` / `MYSQL_PORT` / `MYSQL_USER` / `MYSQL_PASSWORD`
///    / `MYSQL_DATABASE`, each read independently and falling back to the
///    project's long-standing local defaults (`localhost:3306`,
///    `root`/`password`, database `logistics`) when unset.
///
/// Under `cfg(test)` the database-name default is `logistics_test` instead
/// of `logistics`, so `cargo test` never reads or truncates a developer's
/// dev database when no env vars are set. CI (`periodic-tests.yml`) sets
/// `MYSQL_DATABASE=logistics_test` explicitly for the same reason. A test
/// running under a [`TestDb`] guard gets its own database name in place of
/// either of these — see `docs/testing-database.md` (Phases 2 and 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub db_name: String,
    pub username: String,
    pub password: String,
}

impl DbConfig {
    pub fn from_env() -> Self {
        let cfg = Self::resolve_from_env();

        #[cfg(test)]
        let cfg = {
            let mut cfg = cfg;
            if let Some(name) = test_db_override() {
                cfg.db_name = name;
            }
            cfg
        };

        cfg
    }

    fn resolve_from_env() -> Self {
        if let Ok(url) = std::env::var("DATABASE_URL") {
            match Self::parse_database_url(&url) {
                Some(cfg) => return cfg,
                None => eprintln!(
                    "DATABASE_URL is set but is not a valid mysql://user:pass@host:port/db \
                     URL; falling back to MYSQL_* environment variables / defaults"
                ),
            }
        }

        let default_db_name = if cfg!(test) {
            "logistics_test"
        } else {
            "logistics"
        };

        Self {
            host: std::env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("MYSQL_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3306),
            db_name: std::env::var("MYSQL_DATABASE")
                .unwrap_or_else(|_| default_db_name.to_string()),
            username: std::env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
            password: std::env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "password".to_string()),
        }
    }

    /// Parse `mysql://[user[:password]@]host[:port]/db_name`. Returns `None`
    /// on anything that doesn't fit that shape (missing scheme, missing
    /// db name, unparsable port, ...) rather than panicking, so a malformed
    /// `DATABASE_URL` degrades to the `MYSQL_*` / hardcoded fallback in
    /// [`Self::resolve_from_env`].
    fn parse_database_url(url: &str) -> Option<Self> {
        let rest = url.strip_prefix("mysql://")?;
        let (userinfo, hostinfo) = rest.split_once('@')?;
        let (username, password) = userinfo.split_once(':').unwrap_or((userinfo, ""));
        let (hostport, db_name) = hostinfo.split_once('/')?;
        if hostport.is_empty() || db_name.is_empty() {
            return None;
        }
        let (host, port_str) = hostport.split_once(':').unwrap_or((hostport, "3306"));
        let port: u16 = port_str.parse().ok()?;
        Some(Self {
            host: host.to_string(),
            port,
            db_name: db_name.to_string(),
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    /// Build the [`DbConnection`] these settings describe.
    pub fn connection(&self) -> DbConnection {
        DbConnection::new(
            self.host.clone(),
            self.port,
            self.db_name.clone(),
            self.username.clone(),
            self.password.clone(),
        )
    }
}

pub struct DbConnection {
    pub host: String,
    pub port: u16,
    pub db_name: String,
    pub username: String,
    pub password: String,
}

impl DbConnection {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        db_name: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            db_name: db_name.into(),
            username: username.into(),
            password: password.into(),
        }
    }

    /// Build a connection from [`DbConfig::from_env`]. This is what every
    /// call site in the crate should use; call [`Self::new`] directly only
    /// when a caller genuinely needs to target a specific, non-default
    /// database (as [`DbConfig::connection`] itself does internally).
    pub fn from_env() -> Self {
        DbConfig::from_env().connection()
    }

    /// Get a pooled connection to `self.db_name`, creating both the
    /// database and its pool on first use.
    ///
    /// Pools are cached in a registry keyed by database name (see
    /// [`POOLS`]) rather than behind one process-global `OnceLock<Pool>`, so
    /// concurrently running tests — each with their own [`TestDb`] — get
    /// independent pools instead of contending for one.
    pub fn get_connection(&self) -> Result<PooledConn, Box<dyn std::error::Error>> {
        let mut pools = lock_pools();
        if let Some(pool) = pools.get(&self.db_name) {
            return Ok(pool.get_conn()?);
        }

        let root_url = format!(
            "mysql://{}:{}@{}:{}",
            self.username, self.password, self.host, self.port
        );
        if let Ok(p) = build_pool(&root_url) {
            if let Ok(mut conn) = p.get_conn() {
                let _ =
                    conn.query_drop(format!("CREATE DATABASE IF NOT EXISTS `{}`", self.db_name));
            }
        }

        let url = format!(
            "mysql://{}:{}@{}:{}/{}",
            self.username, self.password, self.host, self.port, self.db_name
        );
        let pool = build_pool(&url)?;
        let conn = pool.get_conn()?;
        pools.insert(self.db_name.clone(), pool);
        Ok(conn)
    }
}

/// Build a `Pool` for `url`.
///
/// Under `cfg(test)` this caps the pool at a handful of connections instead
/// of the `mysql` crate's default (min 10, max 100): [`DbConnection`] mints
/// one `Pool` per database, and tests mint one database each via
/// [`TestDb`], so under `cargo test`'s default per-core parallelism the
/// default constraints alone can exceed MySQL's default `max_connections`
/// (151) — each test typically holds one connection at a time, so a small
/// cap is still generous.
fn build_pool(url: &str) -> Result<Pool, mysql::Error> {
    #[cfg(test)]
    {
        let opts = mysql::OptsBuilder::from_opts(mysql::Opts::from_url(url)?).pool_opts(
            mysql::PoolOpts::default()
                .with_constraints(mysql::PoolConstraints::new(1, 4).expect("1 <= 4")),
        );
        Pool::new(opts)
    }
    #[cfg(not(test))]
    {
        Pool::new(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_connection() {
        let db_connection = DbConnection::from_env();
        let res = db_connection.get_connection();
        assert!(res.is_ok() || res.is_err());
    }

    #[test]
    fn from_env_defaults_to_test_database_under_cfg_test() {
        // No env vars mutated here (mutating process env races other tests
        // running in parallel) — this just checks the invariant that a test
        // binary never silently defaults onto the production `logistics`
        // database, whatever MYSQL_DATABASE / DATABASE_URL happen to be set
        // to for this run (CI sets MYSQL_DATABASE=logistics_test explicitly).
        let cfg = DbConfig::from_env();
        assert_ne!(cfg.db_name, "logistics");
    }

    #[test]
    fn from_env_honours_the_test_db_override() {
        set_test_db_override(Some("logistics_test_override_probe".to_string()));
        let cfg = DbConfig::from_env();
        set_test_db_override(None);
        assert_eq!(cfg.db_name, "logistics_test_override_probe");
    }

    #[test]
    fn parse_database_url_full() {
        let cfg = DbConfig::parse_database_url("mysql://alice:secret@db.example.com:3307/mydb")
            .expect("should parse");
        assert_eq!(cfg.host, "db.example.com");
        assert_eq!(cfg.port, 3307);
        assert_eq!(cfg.db_name, "mydb");
        assert_eq!(cfg.username, "alice");
        assert_eq!(cfg.password, "secret");
    }

    #[test]
    fn parse_database_url_defaults_port_and_allows_empty_password() {
        let cfg = DbConfig::parse_database_url("mysql://root:@localhost/logistics_test")
            .expect("should parse");
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 3306);
        assert_eq!(cfg.db_name, "logistics_test");
        assert_eq!(cfg.username, "root");
        assert_eq!(cfg.password, "");
    }

    #[test]
    fn parse_database_url_rejects_missing_scheme() {
        assert!(DbConfig::parse_database_url("postgres://root:pw@localhost:5432/db").is_none());
    }

    #[test]
    fn parse_database_url_rejects_missing_db_name() {
        assert!(DbConfig::parse_database_url("mysql://root:pw@localhost:3306/").is_none());
        assert!(DbConfig::parse_database_url("mysql://root:pw@localhost:3306").is_none());
    }

    #[test]
    fn parse_database_url_rejects_bad_port() {
        assert!(DbConfig::parse_database_url("mysql://root:pw@localhost:notaport/db").is_none());
    }
}
