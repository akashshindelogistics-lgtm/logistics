use crate::logistics::db::connection::DbConnection;
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const JWT_SECRET: &[u8] = b"logistics_jwt_secret_2024_changeme_in_prod";
const TOKEN_EXPIRY_SECS: u64 = 86400; // 24 hours

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub org_id: String,
    pub org_name: String,
    pub exp: usize,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OrgSummary {
    pub id: String,
    pub name: String,
}

pub struct OrgCredentials;

impl OrgCredentials {
    fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.exec_drop(
            "CREATE TABLE IF NOT EXISTS OrgCredentials (
                org_id VARCHAR(36) PRIMARY KEY,
                org_name VARCHAR(255) NOT NULL,
                password_hash VARCHAR(255) NOT NULL
            )",
            (),
        )?;
        Ok(())
    }

    pub fn create(org_id: Uuid, org_name: &str, password: &str) -> Result<(), Box<dyn Error>> {
        let password_hash = hash(password, DEFAULT_COST)?;
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;
        Self::ensure_table(&mut conn)?;

        conn.exec_drop(
            "INSERT INTO OrgCredentials (org_id, org_name, password_hash)
             VALUES (:org_id, :org_name, :password_hash)
             ON DUPLICATE KEY UPDATE org_name = :org_name, password_hash = :password_hash",
            params! {
                "org_id" => org_id.to_string(),
                "org_name" => org_name,
                "password_hash" => &password_hash,
            },
        )?;

        Ok(())
    }

    /// Returns org_name on successful verification, None if org not found or password wrong.
    pub fn verify_login(org_id: Uuid, password: &str) -> Result<Option<String>, Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;
        Self::ensure_table(&mut conn)?;

        let row: Option<(String, String)> = conn.exec_first(
            "SELECT org_name, password_hash FROM OrgCredentials WHERE org_id = :org_id",
            params! { "org_id" => org_id.to_string() },
        )?;

        match row {
            None => Ok(None),
            Some((org_name, stored_hash)) => {
                if verify(password, &stored_hash)? {
                    Ok(Some(org_name))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn list_summaries() -> Result<Vec<OrgSummary>, Box<dyn Error>> {
        let db_connection = DbConnection::new("localhost", 3306, "logistics", "root", "password");
        let mut conn = db_connection.get_connection()?;
        Self::ensure_table(&mut conn)?;

        let rows: Vec<(String, String)> = conn.exec_map(
            "SELECT org_id, org_name FROM OrgCredentials ORDER BY org_name",
            (),
            |(id, name)| (id, name),
        )?;

        Ok(rows
            .into_iter()
            .map(|(id, name)| OrgSummary { id, name })
            .collect())
    }
}

pub fn generate_token(org_id: Uuid, org_name: &str) -> Result<String, Box<dyn Error>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let claims = Claims {
        org_id: org_id.to_string(),
        org_name: org_name.to_string(),
        exp: (now + TOKEN_EXPIRY_SECS) as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )?;

    Ok(token)
}

pub fn decode_token(token: &str) -> Result<Claims, Box<dyn Error>> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_decode_token() {
        let org_id = Uuid::new_v4();
        let org_name = "Test Organization";

        let token = generate_token(org_id, org_name).expect("Failed to generate token");
        assert!(!token.is_empty());

        let claims = decode_token(&token).expect("Failed to decode token");
        assert_eq!(claims.org_id, org_id.to_string());
        assert_eq!(claims.org_name, org_name);
    }

    #[test]
    fn test_decode_invalid_token_returns_error() {
        let result = decode_token("not.a.valid.jwt");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_verify_credentials() {
        let org_id = Uuid::new_v4();
        let org_name = "Credentials Test Org";
        let password = "super_secure_pass_123";

        OrgCredentials::create(org_id, org_name, password).expect("Failed to create credentials");

        let result =
            OrgCredentials::verify_login(org_id, password).expect("DB error on verify_login");
        assert_eq!(result.as_deref(), Some(org_name));
    }

    #[test]
    fn test_verify_wrong_password_returns_none() {
        let org_id = Uuid::new_v4();
        OrgCredentials::create(org_id, "Wrong Pass Org", "correct_password")
            .expect("Failed to create credentials");

        let result = OrgCredentials::verify_login(org_id, "wrong_password")
            .expect("DB error on verify_login");
        assert!(result.is_none());
    }

    #[test]
    fn test_verify_nonexistent_org_returns_none() {
        let org_id = Uuid::new_v4(); // Never saved to DB
        let result =
            OrgCredentials::verify_login(org_id, "any_password").expect("DB error on verify");
        assert!(result.is_none());
    }

    #[test]
    fn test_list_summaries_returns_registered_orgs() {
        let org_id = Uuid::new_v4();
        OrgCredentials::create(org_id, "Summary List Org", "pass123")
            .expect("Failed to create credentials");

        let summaries = OrgCredentials::list_summaries().expect("Failed to list summaries");
        let found = summaries.iter().any(|s| s.id == org_id.to_string());
        assert!(found, "Created org should appear in summaries list");
    }
}
