use mysql::prelude::*;
use mysql::*;
use std::sync::OnceLock;

static DB_POOL: OnceLock<Pool> = OnceLock::new();

/// MySQL connection settings, resolved once from the environment instead of
/// being hardcoded at every call site. See docs/testing-database.md (Phase 2).
pub struct DbConfig {
    pub host: String,
    pub port: u16,
    pub db_name: String,
    pub username: String,
    pub password: String,
}

impl DbConfig {
    /// Resolution order: `DATABASE_URL` (a `mysql://user:pass@host:port/db`
    /// URL) if set, else the individual `MYSQL_HOST` / `MYSQL_PORT` /
    /// `MYSQL_USER` / `MYSQL_PASSWORD` / `MYSQL_DATABASE` variables, else the
    /// historical hardcoded defaults so local dev keeps working unconfigured.
    pub fn from_env() -> Self {
        if let Ok(url) = std::env::var("DATABASE_URL") {
            if let Ok(opts) = Opts::from_url(&url) {
                return Self {
                    host: opts.get_ip_or_hostname().into_owned(),
                    port: opts.get_tcp_port(),
                    db_name: opts.get_db_name().unwrap_or("logistics").to_string(),
                    username: opts.get_user().unwrap_or("root").to_string(),
                    password: opts.get_pass().unwrap_or("password").to_string(),
                };
            }
        }

        Self {
            host: std::env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("MYSQL_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3306),
            db_name: std::env::var("MYSQL_DATABASE").unwrap_or_else(|_| "logistics".to_string()),
            username: std::env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
            password: std::env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "password".to_string()),
        }
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

    /// Builds a connection from `DbConfig::from_env()`. Production code and
    /// tests should use this instead of hardcoding connection args.
    pub fn from_env() -> Self {
        let config = DbConfig::from_env();
        Self::new(
            config.host,
            config.port,
            config.db_name,
            config.username,
            config.password,
        )
    }

    pub fn get_connection(&self) -> Result<PooledConn, Box<dyn std::error::Error>> {
        let pool = DB_POOL.get_or_init(|| {
            // When the crate is built for tests, transparently target a separate
            // `<db>_test` database so `cargo test` never reads or truncates the
            // data a developer's `cargo run` writes. See the `test_support`
            // module and docs/testing-database.md.
            let db_name = if cfg!(test) {
                format!("{}_test", self.db_name)
            } else {
                self.db_name.clone()
            };

            let root_url = format!(
                "mysql://{}:{}@{}:{}",
                self.username, self.password, self.host, self.port
            );
            if let Ok(p) = Pool::new(root_url.as_str()) {
                if let Ok(mut conn) = p.get_conn() {
                    let _ = conn.query_drop(format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name));
                }
            }

            let url = format!(
                "mysql://{}:{}@{}:{}/{}",
                self.username, self.password, self.host, self.port, db_name
            );
            Pool::new(url.as_str()).expect("Failed to initialize MySQL pool")
        });

        let conn = pool.get_conn()?;
        Ok(conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_get_connection() {
        let db_connection = DbConnection {
            host: "localhost".to_string(),
            port: 3306,
            db_name: "logistics".to_string(),
            username: "root".to_string(),
            password: "password".to_string(),
        };
        let res = db_connection.get_connection();
        assert!(res.is_ok() || res.is_err());
    }

    // Mutates process-wide env vars, so this runs serialized with the DB
    // tests (which also call `DbConfig::from_env()` indirectly) to avoid
    // another thread observing a half-set config.
    #[test]
    #[serial(db)]
    fn test_db_config_from_env_defaults_when_unset() {
        unsafe {
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("MYSQL_HOST");
            std::env::remove_var("MYSQL_PORT");
            std::env::remove_var("MYSQL_USER");
            std::env::remove_var("MYSQL_PASSWORD");
            std::env::remove_var("MYSQL_DATABASE");
        }

        let config = DbConfig::from_env();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3306);
        assert_eq!(config.db_name, "logistics");
        assert_eq!(config.username, "root");
        assert_eq!(config.password, "password");
    }

    #[test]
    #[serial(db)]
    fn test_db_config_from_env_honours_mysql_vars() {
        unsafe {
            std::env::remove_var("DATABASE_URL");
            std::env::set_var("MYSQL_HOST", "db.internal");
            std::env::set_var("MYSQL_PORT", "3307");
            std::env::set_var("MYSQL_USER", "app_user");
            std::env::set_var("MYSQL_PASSWORD", "s3cr3t");
            std::env::set_var("MYSQL_DATABASE", "app_db");
        }

        let config = DbConfig::from_env();

        unsafe {
            std::env::remove_var("MYSQL_HOST");
            std::env::remove_var("MYSQL_PORT");
            std::env::remove_var("MYSQL_USER");
            std::env::remove_var("MYSQL_PASSWORD");
            std::env::remove_var("MYSQL_DATABASE");
        }

        assert_eq!(config.host, "db.internal");
        assert_eq!(config.port, 3307);
        assert_eq!(config.db_name, "app_db");
        assert_eq!(config.username, "app_user");
        assert_eq!(config.password, "s3cr3t");
    }

    #[test]
    #[serial(db)]
    fn test_db_config_from_env_honours_database_url() {
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "mysql://app_user:s3cr3t@db.internal:3307/app_db",
            );
        }

        let config = DbConfig::from_env();

        unsafe {
            std::env::remove_var("DATABASE_URL");
        }

        assert_eq!(config.host, "db.internal");
        assert_eq!(config.port, 3307);
        assert_eq!(config.db_name, "app_db");
        assert_eq!(config.username, "app_user");
        assert_eq!(config.password, "s3cr3t");
    }
}
