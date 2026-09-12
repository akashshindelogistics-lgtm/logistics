//! The retrieval half of the org-scoped "ask your data" assistant
//! (`crate::logistics::ai::assistant`): a flat table of short narrative
//! text chunks drawn from an org's own free-text data, searched with
//! MySQL's built-in `FULLTEXT` index rather than embeddings — this app has
//! no vector store and deliberately avoids adding a second AI vendor just
//! for embeddings, so lexical relevance ranking stands in for semantic
//! similarity. See `todo.org`'s "AI opportunities" section for the full
//! design and phasing.
//!
//! v1 indexes two chunk kinds, one row per source row: [`Notification`] and
//! [`VehicleDocument`]. Indexing is write-through and best-effort — see
//! [`upsert_best_effort`] — never a background job, and never allowed to
//! fail the write it's attached to.

use crate::logistics::db::connection::DbConnection;
use crate::logistics::notification::notification::Notification;
use crate::logistics::vehicle::document::VehicleDocument;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// What kind of source row a chunk was built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChunkKind {
    Notification,
    VehicleDocument,
}

impl ChunkKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkKind::Notification => "notification",
            ChunkKind::VehicleDocument => "vehicle_document",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "vehicle_document" => ChunkKind::VehicleDocument,
            _ => ChunkKind::Notification,
        }
    }
}

/// One retrievable fact about an org, drawn from one source row.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AiChunk {
    pub id: Uuid,
    pub org_id: Uuid,
    pub kind: ChunkKind,
    /// The id of the row this chunk was built from (e.g. a `Notification.id`).
    pub source_id: String,
    pub text: String,
    pub updated_at: i64,
}

/// Create the `AiChunks` table if it does not exist. Kept in sync with
/// `test_support::migrate`.
pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
    conn.exec_drop(
        "CREATE TABLE IF NOT EXISTS AiChunks (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            kind VARCHAR(32) NOT NULL,
            source_id VARCHAR(191) NOT NULL,
            text TEXT NOT NULL,
            updated_at BIGINT NOT NULL,
            UNIQUE KEY uq_ai_chunk_source (org_id, kind, source_id),
            FULLTEXT INDEX ft_ai_chunk_text (text),
            CONSTRAINT fk_ai_chunk_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
        (),
    )?;
    Ok(())
}

/// Build or refresh the chunk for one source row. Idempotent — re-indexing
/// the same `(org_id, kind, source_id)` overwrites the previous text.
pub fn upsert(
    org_id: Uuid,
    kind: ChunkKind,
    source_id: &str,
    text: String,
) -> Result<(), Box<dyn Error>> {
    let mut conn = DbConnection::from_env().get_connection()?;
    ensure_table(&mut conn)?;
    conn.exec_drop(
        "INSERT INTO AiChunks (id, org_id, kind, source_id, text, updated_at)
         VALUES (:id, :org_id, :kind, :source_id, :text, :updated_at)
         ON DUPLICATE KEY UPDATE text = VALUES(text), updated_at = VALUES(updated_at)",
        params! {
            "id" => Uuid::new_v4().to_string(),
            "org_id" => org_id.to_string(),
            "kind" => kind.as_str(),
            "source_id" => source_id,
            "text" => &text,
            "updated_at" => now_unix(),
        },
    )?;
    Ok(())
}

/// [`upsert`], but swallows any failure (logging it) instead of propagating
/// it. Every write-through indexing call site uses this — indexing is
/// additive and must never fail the core write (a dispatch notification, a
/// compliance document) it's attached to.
pub fn upsert_best_effort(org_id: Uuid, kind: ChunkKind, source_id: &str, text: String) {
    if let Err(e) = upsert(org_id, kind, source_id, text) {
        eprintln!("ai chunk index: failed to index {kind:?} {source_id}: {e}");
    }
}

/// Remove a chunk for a source row that no longer exists. Best-effort, same
/// rationale as [`upsert_best_effort`].
pub fn delete_by_source_best_effort(kind: ChunkKind, source_id: &str) {
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        ensure_table(&mut conn)?;
        conn.exec_drop(
            "DELETE FROM AiChunks WHERE kind = :kind AND source_id = :source_id",
            params! { "kind" => kind.as_str(), "source_id" => source_id },
        )?;
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("ai chunk index: failed to remove {kind:?} {source_id}: {e}");
    }
}

/// The org's chunks that lexically match `query`, most relevant first.
/// `org_id` is the tenant-isolation boundary here — the same role it plays
/// in every other `list_by_org`.
pub fn search_by_org(org_id: Uuid, query: &str, limit: u32) -> Result<Vec<AiChunk>, Box<dyn Error>> {
    let mut conn = DbConnection::from_env().get_connection()?;
    ensure_table(&mut conn)?;

    let rows: Vec<(String, String, String, String, String, i64)> = conn.exec(
        "SELECT id, org_id, kind, source_id, text, updated_at
         FROM AiChunks
         WHERE org_id = :org_id
           AND MATCH(text) AGAINST (:query IN NATURAL LANGUAGE MODE)
         ORDER BY MATCH(text) AGAINST (:query IN NATURAL LANGUAGE MODE) DESC
         LIMIT :limit",
        params! {
            "org_id" => org_id.to_string(),
            "query" => query,
            "limit" => limit,
        },
    )?;

    Ok(rows
        .into_iter()
        .map(|(id, org_id, kind, source_id, text, updated_at)| AiChunk {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            kind: ChunkKind::from_str(&kind),
            source_id,
            text,
            updated_at,
        })
        .collect())
}

/// The narrative text for a [`Notification`] chunk.
pub fn notification_chunk_text(n: &Notification) -> String {
    format!(
        "Notification to {} via {:?} ({:?}): {}",
        n.recipient_kind, n.channel, n.status, n.body
    )
}

/// The narrative text for a [`VehicleDocument`] chunk.
pub fn vehicle_document_chunk_text(d: &VehicleDocument) -> String {
    let notes_part = d
        .notes
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|n| format!(" Notes: {n}"))
        .unwrap_or_default();
    format!(
        "Vehicle {}'s {} (document #{}) expires on {} (status: {:?}).{}",
        d.vehicle_registration,
        d.doc_type.as_str(),
        d.document_number,
        d.expires_on,
        d.status,
        notes_part
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::notification::notification::{
        NotificationChannel, NotificationEvent, NotificationStatus,
    };
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::test_support::TestDb;
    use crate::logistics::vehicle::document::{ComplianceDocType, ComplianceStatus};

    fn sample_notification() -> Notification {
        Notification {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            dispatch_id: Uuid::new_v4(),
            event: NotificationEvent::DispatchCreated,
            channel: NotificationChannel::Email,
            recipient_kind: "customer".to_string(),
            recipient: "priya@example.com".to_string(),
            body: "Hi Priya Sharma, your order (ref a1b2c3d4) has been dispatched.".to_string(),
            status: NotificationStatus::Queued,
            created_at: 1_700_000_000,
        }
    }

    fn sample_document() -> VehicleDocument {
        VehicleDocument {
            id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            vehicle_registration: "MH12AB1234".to_string(),
            doc_type: ComplianceDocType::Insurance,
            document_number: "INS-9911".to_string(),
            issued_on: None,
            expires_on: "2026-10-02".to_string(),
            notes: None,
            days_until_expiry: 20,
            status: ComplianceStatus::ExpiringSoon,
        }
    }

    #[test]
    fn notification_chunk_text_includes_recipient_channel_status_and_body() {
        let n = sample_notification();
        let text = notification_chunk_text(&n);
        assert!(text.contains("customer"));
        assert!(text.contains("Email"));
        assert!(text.contains("Queued"));
        assert!(text.contains("Priya Sharma"));
    }

    #[test]
    fn vehicle_document_chunk_text_includes_vehicle_type_number_expiry_and_status() {
        let d = sample_document();
        let text = vehicle_document_chunk_text(&d);
        assert!(text.contains("MH12AB1234"));
        assert!(text.contains("Insurance"));
        assert!(text.contains("INS-9911"));
        assert!(text.contains("2026-10-02"));
        assert!(text.contains("ExpiringSoon"));
        assert!(!text.contains("Notes:"), "no Notes: suffix when notes is None");
    }

    #[test]
    fn vehicle_document_chunk_text_appends_notes_when_present() {
        let mut d = sample_document();
        d.notes = Some("renewed at RTO".to_string());
        let text = vehicle_document_chunk_text(&d);
        assert!(text.contains("Notes: renewed at RTO"));
    }

    #[test]
    fn vehicle_document_chunk_text_omits_notes_when_blank() {
        let mut d = sample_document();
        d.notes = Some("   ".to_string());
        let text = vehicle_document_chunk_text(&d);
        assert!(!text.contains("Notes:"));
    }

    #[test]
    fn search_by_org_is_scoped_and_ranks_by_relevance() {
        let _db = TestDb::create();
        let org_a = Organization::create_organization("Org A", "1 Rd").unwrap();
        let org_b = Organization::create_organization("Org B", "2 Rd").unwrap();

        upsert(
            org_a.id,
            ChunkKind::Notification,
            "n1",
            "Vehicle MH12AB1234 insurance expires soon".to_string(),
        )
        .unwrap();
        upsert(
            org_b.id,
            ChunkKind::Notification,
            "n2",
            "Vehicle MH12AB1234 insurance expires soon".to_string(),
        )
        .unwrap();

        let results = search_by_org(org_a.id, "insurance expires", 8).unwrap();
        assert_eq!(results.len(), 1, "org B's matching chunk must not leak into org A's results");
        assert_eq!(results[0].source_id, "n1");
    }

    #[test]
    fn upsert_overwrites_the_existing_chunk_for_the_same_source() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Org C", "3 Rd").unwrap();

        upsert(org.id, ChunkKind::Notification, "n1", "original wording".to_string()).unwrap();
        upsert(org.id, ChunkKind::Notification, "n1", "updated wording".to_string()).unwrap();

        let results = search_by_org(org.id, "wording", 8).unwrap();
        assert_eq!(results.len(), 1, "re-indexing the same source updates in place, not appends");
        assert_eq!(results[0].text, "updated wording");
    }

    #[test]
    fn search_by_org_returns_nothing_for_an_org_with_no_indexed_data() {
        let _db = TestDb::create();
        let org = Organization::create_organization("Org D", "4 Rd").unwrap();
        let results = search_by_org(org.id, "anything", 8).unwrap();
        assert!(results.is_empty());
    }
}
