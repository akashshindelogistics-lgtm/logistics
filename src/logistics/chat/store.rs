//! Conversations and confirmation cards. A conversation belongs to the person
//! who started it, not the whole org.
//!
//! `ChatMessages.content` is the Messages API content exactly as sent or
//! received. The history only ever grows, because the model's thinking blocks
//! stop being valid if earlier turns are edited.

use crate::logistics::agent::store::now_unix;
use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// Who is talking: conversations are private to this pair. `user_id` is `None`
/// for the org-owner login, which has no user row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
}

impl Owner {
    fn user_key(&self) -> String {
        self.user_id.map(|u| u.to_string()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatConversation {
    pub id: Uuid,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Confirmation cards still waiting for an answer.
    pub pending_count: i64,
}

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: i64,
    pub role: String,
    pub content: Value,
    /// Hidden rows are for the model: tool results and "the user confirmed".
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProposalStatus {
    Pending,
    Executing,
    Executed,
    Failed,
    Cancelled,
}

impl ProposalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalStatus::Pending => "PENDING",
            ProposalStatus::Executing => "EXECUTING",
            ProposalStatus::Executed => "EXECUTED",
            ProposalStatus::Failed => "FAILED",
            ProposalStatus::Cancelled => "CANCELLED",
        }
    }

    fn from_db(s: &str) -> Self {
        match s {
            "PENDING" => ProposalStatus::Pending,
            "EXECUTING" => ProposalStatus::Executing,
            "EXECUTED" => ProposalStatus::Executed,
            "CANCELLED" => ProposalStatus::Cancelled,
            _ => ProposalStatus::Failed,
        }
    }
}

/// One line on a confirmation card. Written by the server from the real
/// request, never by the model.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProposalField {
    pub label: String,
    pub value: String,
}

/// A write waiting for a person, and what became of it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatProposal {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub operation_id: String,
    pub title: String,
    /// Deletes and money: shown as a warning.
    pub dangerous: bool,
    pub fields: Vec<ProposalField>,
    /// The model's own explanation. Shown as such, since it's the one part of
    /// the card the server can't vouch for.
    pub reason: String,
    pub status: ProposalStatus,
    /// What the API said when this ran.
    pub result_message: Option<String>,
    /// The `data` the API returned, such as the updated dispatch.
    #[schema(value_type = Option<Object>)]
    pub result_data: Option<Value>,
    pub created_at: i64,
    #[serde(skip)]
    pub tool_use_id: String,
    #[serde(skip)]
    pub method: String,
    #[serde(skip)]
    pub uri: String,
    #[serde(skip)]
    pub body: Option<Value>,
}

pub fn ensure_tables(conn: &mut PooledConn) -> Result<(), String> {
    for ddl in [
        "CREATE TABLE IF NOT EXISTS ChatConversations (
            id VARCHAR(36) PRIMARY KEY,
            org_id VARCHAR(36) NOT NULL,
            user_id VARCHAR(36) NOT NULL DEFAULT '',
            title VARCHAR(120) NOT NULL DEFAULT '',
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL,
            INDEX idx_chat_owner (org_id, user_id, updated_at),
            CONSTRAINT fk_chat_conversation_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
        "CREATE TABLE IF NOT EXISTS ChatMessages (
            id BIGINT PRIMARY KEY AUTO_INCREMENT,
            conversation_id VARCHAR(36) NOT NULL,
            org_id VARCHAR(36) NOT NULL,
            role VARCHAR(16) NOT NULL,
            content LONGTEXT NOT NULL,
            visible BOOLEAN NOT NULL,
            tokens BIGINT NOT NULL DEFAULT 0,
            created_at BIGINT NOT NULL,
            INDEX idx_chat_messages (conversation_id, id),
            INDEX idx_chat_usage (org_id, created_at),
            CONSTRAINT fk_chat_message_conversation FOREIGN KEY (conversation_id) REFERENCES ChatConversations(id) ON DELETE CASCADE
        )",
        "CREATE TABLE IF NOT EXISTS ChatProposals (
            id VARCHAR(36) PRIMARY KEY,
            seq BIGINT NOT NULL AUTO_INCREMENT UNIQUE,
            conversation_id VARCHAR(36) NOT NULL,
            tool_use_id VARCHAR(64) NOT NULL,
            operation_id VARCHAR(64) NOT NULL,
            title VARCHAR(120) NOT NULL,
            dangerous BOOLEAN NOT NULL,
            method VARCHAR(8) NOT NULL,
            uri VARCHAR(512) NOT NULL,
            body LONGTEXT NULL,
            fields LONGTEXT NOT NULL,
            reason TEXT NOT NULL,
            status VARCHAR(16) NOT NULL,
            result_message TEXT NULL,
            result_data LONGTEXT NULL,
            created_at BIGINT NOT NULL,
            decided_at BIGINT NULL,
            INDEX idx_chat_proposals (conversation_id, status),
            CONSTRAINT fk_chat_proposal_conversation FOREIGN KEY (conversation_id) REFERENCES ChatConversations(id) ON DELETE CASCADE
        )",
    ] {
        conn.query_drop(ddl).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn connect() -> Result<PooledConn, String> {
    let mut conn = DbConnection::from_env()
        .get_connection()
        .map_err(|e| e.to_string())?;
    ensure_tables(&mut conn)?;
    Ok(conn)
}

fn err(e: mysql::Error) -> String {
    e.to_string()
}

pub fn create_conversation(owner: &Owner) -> Result<ChatConversation, String> {
    let mut conn = connect()?;
    let id = Uuid::new_v4();
    let now = now_unix();
    conn.exec_drop(
        "INSERT INTO ChatConversations (id, org_id, user_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?)",
        (
            id.to_string(),
            owner.org_id.to_string(),
            owner.user_key(),
            now,
            now,
        ),
    )
    .map_err(err)?;
    Ok(ChatConversation {
        id,
        title: String::new(),
        created_at: now,
        updated_at: now,
        pending_count: 0,
    })
}

const CONVERSATION_COLUMNS: &str = "c.id, c.title, c.created_at, c.updated_at,
    (SELECT COUNT(*) FROM ChatProposals p WHERE p.conversation_id = c.id AND p.status = 'PENDING')";

type ConversationRow = (String, String, i64, i64, i64);

fn conversation_from_row(row: ConversationRow) -> Option<ChatConversation> {
    Some(ChatConversation {
        id: Uuid::parse_str(&row.0).ok()?,
        title: row.1,
        created_at: row.2,
        updated_at: row.3,
        pending_count: row.4,
    })
}

pub fn list_conversations(owner: &Owner, limit: u32) -> Result<Vec<ChatConversation>, String> {
    let mut conn = connect()?;
    let rows: Vec<ConversationRow> = conn
        .exec(
            format!(
                "SELECT {CONVERSATION_COLUMNS} FROM ChatConversations c
                 WHERE c.org_id = ? AND c.user_id = ?
                 ORDER BY c.updated_at DESC, c.id LIMIT ?"
            ),
            (owner.org_id.to_string(), owner.user_key(), limit),
        )
        .map_err(err)?;
    Ok(rows.into_iter().filter_map(conversation_from_row).collect())
}

/// `None` when it doesn't exist or isn't this person's.
pub fn get_conversation(owner: &Owner, id: Uuid) -> Result<Option<ChatConversation>, String> {
    let mut conn = connect()?;
    let row: Option<ConversationRow> = conn
        .exec_first(
            format!(
                "SELECT {CONVERSATION_COLUMNS} FROM ChatConversations c
                 WHERE c.id = ? AND c.org_id = ? AND c.user_id = ?"
            ),
            (id.to_string(), owner.org_id.to_string(), owner.user_key()),
        )
        .map_err(err)?;
    Ok(row.and_then(conversation_from_row))
}

pub fn delete_conversation(owner: &Owner, id: Uuid) -> Result<bool, String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "DELETE FROM ChatConversations WHERE id = ? AND org_id = ? AND user_id = ?",
        (id.to_string(), owner.org_id.to_string(), owner.user_key()),
    )
    .map_err(err)?;
    Ok(conn.affected_rows() > 0)
}

/// Name a new conversation after its first message.
pub fn set_title_if_empty(id: Uuid, title: &str) -> Result<(), String> {
    let title: String = title.chars().take(60).collect();
    let mut conn = connect()?;
    conn.exec_drop(
        "UPDATE ChatConversations SET title = ? WHERE id = ? AND title = ''",
        (title, id.to_string()),
    )
    .map_err(err)
}

pub struct NewMessage {
    pub role: &'static str,
    pub content: Value,
    pub visible: bool,
    pub tokens: i64,
}

/// Append rows together, so a tool call is never stored without its result.
pub fn append_messages(owner: &Owner, id: Uuid, messages: Vec<NewMessage>) -> Result<(), String> {
    let mut conn = connect()?;
    let now = now_unix();
    let mut tx = conn.start_transaction(TxOpts::default()).map_err(err)?;
    for message in messages {
        tx.exec_drop(
            "INSERT INTO ChatMessages (conversation_id, org_id, role, content, visible, tokens, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                id.to_string(),
                owner.org_id.to_string(),
                message.role,
                message.content.to_string(),
                message.visible,
                message.tokens,
                now,
            ),
        )
        .map_err(err)?;
    }
    tx.exec_drop(
        "UPDATE ChatConversations SET updated_at = ? WHERE id = ?",
        (now, id.to_string()),
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

pub fn list_messages(id: Uuid) -> Result<Vec<StoredMessage>, String> {
    let mut conn = connect()?;
    let rows: Vec<(i64, String, String, bool)> = conn
        .exec(
            "SELECT id, role, content, visible FROM ChatMessages
             WHERE conversation_id = ? ORDER BY id",
            (id.to_string(),),
        )
        .map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|(id, role, content, visible)| StoredMessage {
            id,
            role,
            content: serde_json::from_str(&content).unwrap_or(Value::Null),
            visible,
        })
        .collect())
}

/// How many characters of history this conversation sends on every turn.
pub fn history_chars(id: Uuid) -> Result<i64, String> {
    let mut conn = connect()?;
    let total: Option<Option<i64>> = conn
        .exec_first(
            "SELECT CAST(SUM(CHAR_LENGTH(content)) AS SIGNED) FROM ChatMessages WHERE conversation_id = ?",
            (id.to_string(),),
        )
        .map_err(err)?;
    Ok(total.flatten().unwrap_or(0))
}

pub fn tokens_used_last_day(org_id: Uuid) -> Result<i64, String> {
    let mut conn = connect()?;
    let total: Option<Option<i64>> = conn
        .exec_first(
            "SELECT CAST(SUM(tokens) AS SIGNED) FROM ChatMessages WHERE org_id = ? AND created_at >= ?",
            (org_id.to_string(), now_unix() - 86_400),
        )
        .map_err(err)?;
    Ok(total.flatten().unwrap_or(0))
}

pub struct NewProposal<'a> {
    pub conversation_id: Uuid,
    pub tool_use_id: &'a str,
    pub operation_id: &'a str,
    pub title: &'a str,
    pub dangerous: bool,
    pub method: &'a str,
    pub uri: &'a str,
    pub body: Option<&'a Value>,
    pub fields: &'a [ProposalField],
    pub reason: &'a str,
}

pub fn create_proposal(new: NewProposal) -> Result<Uuid, String> {
    let mut conn = connect()?;
    let id = Uuid::new_v4();
    let fields = serde_json::to_string(new.fields).map_err(|e| e.to_string())?;
    conn.exec_drop(
        "INSERT INTO ChatProposals (id, conversation_id, tool_use_id, operation_id, title, dangerous,
            method, uri, body, fields, reason, status, created_at)
         VALUES (:id, :conversation_id, :tool_use_id, :operation_id, :title, :dangerous,
            :method, :uri, :body, :fields, :reason, 'PENDING', :created_at)",
        params! {
            "id" => id.to_string(),
            "conversation_id" => new.conversation_id.to_string(),
            "tool_use_id" => new.tool_use_id,
            "operation_id" => new.operation_id,
            "title" => new.title,
            "dangerous" => new.dangerous,
            "method" => new.method,
            "uri" => new.uri,
            "body" => new.body.map(Value::to_string),
            "fields" => fields,
            "reason" => new.reason,
            "created_at" => now_unix(),
        },
    )
    .map_err(err)?;
    Ok(id)
}

const PROPOSAL_COLUMNS: &str = "id, conversation_id, tool_use_id, operation_id, title, dangerous,
    method, uri, body, fields, reason, status, result_message, result_data, created_at";

fn proposal_from_row(mut row: Row) -> Option<ChatProposal> {
    let id: String = row.take("id")?;
    let conversation_id: String = row.take("conversation_id")?;
    let body: Option<String> = row.take("body")?;
    let fields: String = row.take("fields")?;
    let status: String = row.take("status")?;
    let result_data: Option<String> = row.take("result_data")?;
    Some(ChatProposal {
        id: Uuid::parse_str(&id).ok()?,
        conversation_id: Uuid::parse_str(&conversation_id).ok()?,
        tool_use_id: row.take("tool_use_id")?,
        operation_id: row.take("operation_id")?,
        title: row.take("title")?,
        dangerous: row.take("dangerous")?,
        method: row.take("method")?,
        uri: row.take("uri")?,
        body: body.and_then(|b| serde_json::from_str(&b).ok()),
        fields: serde_json::from_str(&fields).unwrap_or_default(),
        reason: row.take("reason")?,
        status: ProposalStatus::from_db(&status),
        result_message: row.take("result_message")?,
        result_data: result_data.and_then(|d| serde_json::from_str(&d).ok()),
        created_at: row.take("created_at")?,
    })
}

pub fn list_proposals(conversation_id: Uuid) -> Result<Vec<ChatProposal>, String> {
    let mut conn = connect()?;
    let rows: Vec<Row> = conn
        .exec(
            format!(
                "SELECT {PROPOSAL_COLUMNS} FROM ChatProposals WHERE conversation_id = ? ORDER BY seq"
            ),
            (conversation_id.to_string(),),
        )
        .map_err(err)?;
    Ok(rows.into_iter().filter_map(proposal_from_row).collect())
}

pub fn get_proposal(id: Uuid) -> Result<Option<ChatProposal>, String> {
    let mut conn = connect()?;
    let row: Option<Row> = conn
        .exec_first(
            format!("SELECT {PROPOSAL_COLUMNS} FROM ChatProposals WHERE id = ?"),
            (id.to_string(),),
        )
        .map_err(err)?;
    Ok(row.and_then(proposal_from_row))
}

/// Move a card out of `PENDING`. Only one caller can win, so a double click
/// can't run a write twice.
pub fn claim_pending(id: Uuid, next: ProposalStatus) -> Result<bool, String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "UPDATE ChatProposals SET status = ?, decided_at = ? WHERE id = ? AND status = 'PENDING'",
        (next.as_str(), now_unix(), id.to_string()),
    )
    .map_err(err)?;
    Ok(conn.affected_rows() > 0)
}

pub fn finish_proposal(
    id: Uuid,
    status: ProposalStatus,
    message: &str,
    data: Option<&Value>,
) -> Result<(), String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "UPDATE ChatProposals SET status = ?, result_message = ?, result_data = ? WHERE id = ?",
        (
            status.as_str(),
            message,
            data.map(Value::to_string),
            id.to_string(),
        ),
    )
    .map_err(err)
}

pub fn pending_count(conversation_id: Uuid) -> Result<i64, String> {
    let mut conn = connect()?;
    let count: Option<i64> = conn
        .exec_first(
            "SELECT COUNT(*) FROM ChatProposals WHERE conversation_id = ? AND status = 'PENDING'",
            (conversation_id.to_string(),),
        )
        .map_err(err)?;
    Ok(count.unwrap_or(0))
}
