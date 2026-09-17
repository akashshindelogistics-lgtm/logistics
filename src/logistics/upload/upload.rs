//! Real file storage for uploaded images — today, proof-of-delivery photos
//! and signatures (`ProofOfDeliveryInput::signature_or_photo_url`, previously
//! a free-form URL/`data:` URI with nothing behind it). Stored on local disk
//! under a configurable directory rather than a cloud object-storage vendor;
//! see `docs/file-uploads.md` for the rationale.

use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Content types this pipeline accepts — it exists for proof-of-delivery
/// photos and signatures, not arbitrary file upload.
pub const ALLOWED_CONTENT_TYPES: [&str; 3] = ["image/jpeg", "image/png", "image/webp"];
/// Hard cap on an uploaded file's size, in bytes (5 MiB).
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Where uploaded files are written: `UPLOAD_DIR`, or `./uploads` by default.
fn upload_dir() -> PathBuf {
    PathBuf::from(std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "./uploads".to_string()))
}

#[derive(Debug)]
pub enum UploadError {
    UnsupportedContentType(String),
    Empty,
    TooLarge(usize),
    Io(std::io::Error),
    Db(Box<dyn std::error::Error>),
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UploadError::UnsupportedContentType(ct) => write!(
                f,
                "Unsupported content type '{ct}'; only {} are accepted",
                ALLOWED_CONTENT_TYPES.join(", ")
            ),
            UploadError::Empty => write!(f, "Uploaded file is empty"),
            UploadError::TooLarge(size) => write!(
                f,
                "File is {size} bytes, which exceeds the {MAX_UPLOAD_BYTES}-byte limit"
            ),
            UploadError::Io(err) => write!(f, "Failed to store the uploaded file: {err}"),
            UploadError::Db(err) => write!(f, "Failed to record the uploaded file: {err}"),
        }
    }
}

impl std::error::Error for UploadError {}

/// A file stored on local disk, tracked so it can be served back org-scoped
/// rather than trusting a bare filesystem path from the client.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UploadedFile {
    pub id: Uuid,
    pub org_id: Uuid,
    pub content_type: String,
    pub byte_size: i64,
    #[serde(skip)]
    pub storage_path: String,
    pub uploaded_at: i64,
}

impl UploadedFile {
    /// Create the `UploadedFiles` table if it does not exist. Kept in sync
    /// with `test_support::migrate`.
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn std::error::Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS UploadedFiles (
                id VARCHAR(36) PRIMARY KEY,
                org_id VARCHAR(36) NOT NULL,
                content_type VARCHAR(64) NOT NULL,
                byte_size BIGINT NOT NULL,
                storage_path VARCHAR(512) NOT NULL,
                uploaded_at BIGINT NOT NULL,
                CONSTRAINT fk_uploaded_file_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        Ok(())
    }

    /// Validate, write to disk under a UUID-derived path, and record a new
    /// uploaded file. `content_type` must be one of [`ALLOWED_CONTENT_TYPES`]
    /// and `bytes` non-empty and no larger than [`MAX_UPLOAD_BYTES`].
    pub fn store(org_id: Uuid, content_type: &str, bytes: &[u8]) -> Result<Self, UploadError> {
        if !ALLOWED_CONTENT_TYPES.contains(&content_type) {
            return Err(UploadError::UnsupportedContentType(content_type.to_string()));
        }
        if bytes.is_empty() {
            return Err(UploadError::Empty);
        }
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(UploadError::TooLarge(bytes.len()));
        }

        let mut conn = DbConnection::from_env().get_connection().map_err(UploadError::Db)?;
        Self::ensure_table(&mut conn).map_err(UploadError::Db)?;

        let id = Uuid::new_v4();
        let ext = match content_type {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            // Unreachable: `content_type` was checked against
            // ALLOWED_CONTENT_TYPES above, which only has these three.
            _ => "bin",
        };
        let dir = upload_dir();
        std::fs::create_dir_all(&dir).map_err(UploadError::Io)?;
        let storage_path = dir.join(format!("{id}.{ext}"));
        std::fs::write(&storage_path, bytes).map_err(UploadError::Io)?;

        let file = UploadedFile {
            id,
            org_id,
            content_type: content_type.to_string(),
            byte_size: bytes.len() as i64,
            storage_path: storage_path.to_string_lossy().into_owned(),
            uploaded_at: now_secs(),
        };

        conn.exec_drop(
            "INSERT INTO UploadedFiles (id, org_id, content_type, byte_size, storage_path, uploaded_at)
             VALUES (:id, :org_id, :content_type, :byte_size, :storage_path, :uploaded_at)",
            params! {
                "id" => file.id.to_string(),
                "org_id" => file.org_id.to_string(),
                "content_type" => &file.content_type,
                "byte_size" => file.byte_size,
                "storage_path" => &file.storage_path,
                "uploaded_at" => file.uploaded_at,
            },
        )
        .map_err(|e| UploadError::Db(e.into()))?;

        Ok(file)
    }

    pub fn get_by_id(id: Uuid) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let row: Option<(String, String, i64, String, i64)> = conn.exec_first(
            "SELECT org_id, content_type, byte_size, storage_path, uploaded_at
             FROM UploadedFiles WHERE id = :id",
            params! { "id" => id.to_string() },
        )?;
        Ok(row.map(|(org_id, content_type, byte_size, storage_path, uploaded_at)| UploadedFile {
            id,
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            content_type,
            byte_size,
            storage_path,
            uploaded_at,
        }))
    }

    /// Read the stored bytes back off disk.
    pub fn read_bytes(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(&self.storage_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::TestDb;

    #[test]
    fn test_store_rejects_unsupported_content_type() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Upload Co", "HQ").expect("org");
        let err = UploadedFile::store(org.id, "application/pdf", b"not an image").unwrap_err();
        assert!(matches!(err, UploadError::UnsupportedContentType(_)));
        assert!(err.to_string().contains("Unsupported content type"));
    }

    #[test]
    fn test_store_rejects_empty_bytes() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Upload Co", "HQ").expect("org");
        let err = UploadedFile::store(org.id, "image/png", &[]).unwrap_err();
        assert!(matches!(err, UploadError::Empty));
    }

    #[test]
    fn test_store_rejects_oversized_bytes() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Upload Co", "HQ").expect("org");
        let too_big = vec![0u8; MAX_UPLOAD_BYTES + 1];
        let err = UploadedFile::store(org.id, "image/jpeg", &too_big).unwrap_err();
        assert!(matches!(err, UploadError::TooLarge(_)));
    }

    #[test]
    fn test_store_writes_to_disk_and_round_trips_through_get_by_id() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Upload Co", "HQ").expect("org");

        let bytes = b"fake png bytes".to_vec();
        let file = UploadedFile::store(org.id, "image/png", &bytes).expect("store");
        assert_eq!(file.org_id, org.id);
        assert_eq!(file.content_type, "image/png");
        assert_eq!(file.byte_size, bytes.len() as i64);
        assert!(std::path::Path::new(&file.storage_path).exists());
        assert_eq!(file.read_bytes().expect("read"), bytes);

        let reloaded = UploadedFile::get_by_id(file.id).expect("get").expect("exists");
        assert_eq!(reloaded.id, file.id);
        assert_eq!(reloaded.org_id, org.id);
        assert_eq!(reloaded.content_type, "image/png");
        assert_eq!(reloaded.byte_size, bytes.len() as i64);
        assert_eq!(reloaded.read_bytes().expect("read"), bytes);
    }

    #[test]
    fn test_get_by_id_returns_none_for_unknown_id() {
        let _db = TestDb::create();
        assert!(UploadedFile::get_by_id(Uuid::new_v4()).expect("get").is_none());
    }
}
