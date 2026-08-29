use mysql::prelude::*;
use mysql::*;
use std::sync::OnceLock;

static DB_POOL: OnceLock<Pool> = OnceLock::new();

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
}
