//! Role-scoped user accounts within an organisation.
//!
//! Until now an org had a single shared password ([`OrgCredentials`]). That
//! login still works and is treated as the org's **Admin**. On top of it an
//! Admin can create named team members, each with their own email + password
//! and one of three roles:
//!
//! - **Admin** — everything, including managing other users and deleting the org.
//! - **Dispatcher** — run dispatches and billing; manage customers, drivers, vehicles.
//! - **WarehouseStaff** — manage godowns and their stock.
//!
//! Every role can *read* everything in its own org; the roles only gate writes.
//! Enforcement lives in `require_role` in the server routes.

use crate::logistics::db::connection::DbConnection;
use bcrypt::{hash, verify, DEFAULT_COST};
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use uuid::Uuid;

/// A team member's role within their organisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrgRole {
    Admin,
    Dispatcher,
    WarehouseStaff,
}

impl OrgRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrgRole::Admin => "ADMIN",
            OrgRole::Dispatcher => "DISPATCHER",
            OrgRole::WarehouseStaff => "WAREHOUSE_STAFF",
        }
    }

    /// Parse a stored / claimed role string. Unknown input falls back to the
    /// **least**-privileged role so a bad value can never widen access.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().replace([' ', '-'], "_").as_str() {
            "ADMIN" | "OWNER" => OrgRole::Admin,
            "DISPATCHER" | "DISPATCH" => OrgRole::Dispatcher,
            _ => OrgRole::WarehouseStaff,
        }
    }
}

impl fmt::Display for OrgRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub enum UserError {
    /// Another user (in any org) already uses this email.
    EmailTaken,
    /// `email` was blank or `password` shorter than 8 characters.
    InvalidInput(&'static str),
    Db(Box<dyn Error>),
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserError::EmailTaken => write!(f, "that email address is already registered"),
            UserError::InvalidInput(why) => write!(f, "{why}"),
            UserError::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl std::error::Error for UserError {}

impl From<mysql::Error> for UserError {
    fn from(e: mysql::Error) -> Self {
        UserError::Db(Box::new(e))
    }
}
impl From<Box<dyn Error>> for UserError {
    fn from(e: Box<dyn Error>) -> Self {
        UserError::Db(e)
    }
}
impl From<bcrypt::BcryptError> for UserError {
    fn from(e: bcrypt::BcryptError) -> Self {
        UserError::Db(Box::new(e))
    }
}

/// A named team member. `password_hash` is never serialised.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OrgUser {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub email: String,
    pub role: OrgRole,
    pub is_active: bool,
}

const SELECT_COLS: &str = "id, org_id, name, email, role, is_active";

fn normalise_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

impl OrgUser {
    /// Create the `OrgUsers` table. Kept in sync with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS OrgUsers (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                name VARCHAR(255) NOT NULL,
                email VARCHAR(255) NOT NULL UNIQUE,
                password_hash VARCHAR(255) NOT NULL,
                role VARCHAR(32) NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                CONSTRAINT fk_org_user_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    fn row_to_user(
        (id, org_id, name, email, role, is_active): (String, String, String, String, String, bool),
    ) -> Self {
        OrgUser {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            name,
            email,
            role: OrgRole::from_str(&role),
            is_active,
        }
    }

    pub fn create(
        org_id: Uuid,
        name: impl Into<String>,
        email: impl AsRef<str>,
        password: impl AsRef<str>,
        role: OrgRole,
    ) -> Result<Self, UserError> {
        let email = normalise_email(email.as_ref());
        if email.is_empty() || !email.contains('@') {
            return Err(UserError::InvalidInput("a valid email address is required"));
        }
        if password.as_ref().len() < 8 {
            return Err(UserError::InvalidInput("password must be at least 8 characters"));
        }

        let mut conn = DbConnection::from_env().get_connection().map_err(UserError::from)?;
        Self::ensure_table(&mut conn).map_err(UserError::from)?;

        let taken: Option<i64> = conn.exec_first(
            "SELECT 1 FROM OrgUsers WHERE email = :email",
            params! { "email" => &email },
        )?;
        if taken.is_some() {
            return Err(UserError::EmailTaken);
        }

        let user = OrgUser {
            id: Uuid::new_v4(),
            org_id,
            name: name.into(),
            email,
            role,
            is_active: true,
        };
        let password_hash = hash(password.as_ref(), DEFAULT_COST)?;

        conn.exec_drop(
            "INSERT INTO OrgUsers (id, org_id, name, email, password_hash, role, is_active)
             VALUES (:id, :org_id, :name, :email, :password_hash, :role, :is_active)",
            params! {
                "id" => user.id.to_string(),
                "org_id" => user.org_id.to_string(),
                "name" => &user.name,
                "email" => &user.email,
                "password_hash" => &password_hash,
                "role" => user.role.as_str(),
                "is_active" => user.is_active,
            },
        )?;

        Ok(user)
    }

    pub fn list_by_org(org_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let rows: Vec<(String, String, String, String, String, bool)> = conn.exec(
            format!("SELECT {SELECT_COLS} FROM OrgUsers WHERE org_id = :org_id ORDER BY name"),
            params! { "org_id" => org_id.to_string() },
        )?;
        Ok(rows.into_iter().map(Self::row_to_user).collect())
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let row: Option<(String, String, String, String, String, bool)> = conn.exec_first(
            format!("SELECT {SELECT_COLS} FROM OrgUsers WHERE id = :id"),
            params! { "id" => id.to_string() },
        )?;
        Ok(row.map(Self::row_to_user))
    }

    /// Authenticate by email + password. Returns the user only when the
    /// password matches **and** the account is active.
    pub fn verify_login(email: &str, password: &str) -> Result<Option<Self>, Box<dyn Error>> {
        let email = normalise_email(email);
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<(String, String, String, String, String, bool, String)> = conn.exec_first(
            format!("SELECT {SELECT_COLS}, password_hash FROM OrgUsers WHERE email = :email"),
            params! { "email" => &email },
        )?;

        match row {
            Some((id, org_id, name, email, role, is_active, password_hash))
                if is_active && verify(password, &password_hash)? =>
            {
                Ok(Some(Self::row_to_user((id, org_id, name, email, role, is_active))))
            }
            _ => Ok(None),
        }
    }

    /// Update the mutable fields (name, role, active flag). Email and password
    /// are managed separately.
    pub fn update(
        &mut self,
        name: impl Into<String>,
        role: OrgRole,
        is_active: bool,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        let name = name.into();
        conn.exec_drop(
            "UPDATE OrgUsers SET name = :name, role = :role, is_active = :is_active WHERE id = :id",
            params! {
                "id" => self.id.to_string(),
                "name" => &name,
                "role" => role.as_str(),
                "is_active" => is_active,
            },
        )?;
        self.name = name;
        self.role = role;
        self.is_active = is_active;
        Ok(())
    }

    pub fn delete(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "DELETE FROM OrgUsers WHERE id = :id",
            params! { "id" => self.id.to_string() },
        )?;
        Ok(())
    }
}
