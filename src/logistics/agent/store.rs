//! The audit trail and the autonomy policy: every run, every action an agent
//! asked for, and what became of it.

use crate::logistics::db::connection::DbConnection;
use mysql::prelude::*;
use mysql::*;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use utoipa::ToSchema;
use uuid::Uuid;

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// How much an org lets its agents do on their own. New orgs are `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutonomyMode {
    /// Agents don't run. Also the kill switch: a running agent stops at its
    /// next turn.
    Off,
    /// Agents run and record what they would do. Nothing changes.
    Shadow,
    /// Internal actions happen; everything else waits for a person.
    Supervised,
    /// Internal and operational actions happen; sensitive ones wait.
    Autonomous,
}

impl AutonomyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AutonomyMode::Off => "OFF",
            AutonomyMode::Shadow => "SHADOW",
            AutonomyMode::Supervised => "SUPERVISED",
            AutonomyMode::Autonomous => "AUTONOMOUS",
        }
    }

    /// Anything unrecognised is `Off`, so a bad stored value can't widen access.
    pub fn from_db(s: &str) -> Self {
        match s {
            "SHADOW" => AutonomyMode::Shadow,
            "SUPERVISED" => AutonomyMode::Supervised,
            "AUTONOMOUS" => AutonomyMode::Autonomous,
            _ => AutonomyMode::Off,
        }
    }
}

/// How much harm a tool can do. Operations too risky for any agent (deleting
/// data, managing users, credentials, recording payments) have no tool at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Risk {
    /// Looks at data. Never recorded as an action.
    Read,
    /// Only visible inside the org, such as raising an alert.
    Internal,
    /// Moves work forward and is easy to put right.
    Operational,
    /// Hard to undo, or customers or money are affected.
    Sensitive,
}

impl Risk {
    pub fn as_str(&self) -> &'static str {
        match self {
            Risk::Read => "READ",
            Risk::Internal => "INTERNAL",
            Risk::Operational => "OPERATIONAL",
            Risk::Sensitive => "SENSITIVE",
        }
    }

    fn from_db(s: &str) -> Self {
        match s {
            "INTERNAL" => Risk::Internal,
            "OPERATIONAL" => Risk::Operational,
            "READ" => Risk::Read,
            _ => Risk::Sensitive,
        }
    }
}

/// What becomes of a requested action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Record,
    AskPerson,
    Execute,
}

/// The whole autonomy policy. Sensitive actions always wait for a person.
pub fn decide(mode: AutonomyMode, risk: Risk) -> Disposition {
    match (mode, risk) {
        (AutonomyMode::Off | AutonomyMode::Shadow, _) => Disposition::Record,
        (_, Risk::Read | Risk::Internal) => Disposition::Execute,
        (AutonomyMode::Autonomous, Risk::Operational) => Disposition::Execute,
        (_, Risk::Operational | Risk::Sensitive) => Disposition::AskPerson,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionStatus {
    /// Shadow mode: what the agent would have done.
    Shadowed,
    PendingApproval,
    /// Approved and being carried out.
    Executing,
    Executed,
    Rejected,
    Failed,
}

impl ActionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionStatus::Shadowed => "SHADOWED",
            ActionStatus::PendingApproval => "PENDING_APPROVAL",
            ActionStatus::Executing => "EXECUTING",
            ActionStatus::Executed => "EXECUTED",
            ActionStatus::Rejected => "REJECTED",
            ActionStatus::Failed => "FAILED",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "SHADOWED" => ActionStatus::Shadowed,
            "PENDING_APPROVAL" => ActionStatus::PendingApproval,
            "EXECUTING" => ActionStatus::Executing,
            "EXECUTED" => ActionStatus::Executed,
            "REJECTED" => ActionStatus::Rejected,
            _ => ActionStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentAction {
    pub id: Uuid,
    pub run_id: Uuid,
    pub org_id: Uuid,
    pub agent: String,
    pub tool: String,
    /// The tool's arguments, as JSON text.
    pub input: String,
    /// The agent's explanation, for whoever reviews this.
    pub reason: String,
    pub risk: Risk,
    pub status: ActionStatus,
    pub result: Option<String>,
    pub created_at: i64,
    pub decided_at: Option<i64>,
    /// The approver's user id. `null` for the org-owner login.
    pub decided_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentRun {
    pub id: Uuid,
    pub org_id: Uuid,
    pub agent: String,
    /// `schedule` or `manual`.
    pub trigger_source: String,
    /// `RUNNING`, `COMPLETED`, `FAILED`, `REFUSED` or `HALTED`.
    pub status: String,
    pub summary: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

pub fn ensure_tables(conn: &mut PooledConn) -> Result<(), String> {
    for ddl in [
        "CREATE TABLE IF NOT EXISTS AgentSettings (
            org_id VARCHAR(36) PRIMARY KEY,
            mode VARCHAR(16) NOT NULL,
            updated_at BIGINT NOT NULL,
            CONSTRAINT fk_agent_settings_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
        "CREATE TABLE IF NOT EXISTS AgentRuns (
            id VARCHAR(36) PRIMARY KEY,
            seq BIGINT NOT NULL AUTO_INCREMENT UNIQUE,
            org_id VARCHAR(36) NOT NULL,
            agent VARCHAR(64) NOT NULL,
            trigger_source VARCHAR(16) NOT NULL,
            status VARCHAR(16) NOT NULL,
            summary TEXT NULL,
            input_tokens BIGINT NOT NULL DEFAULT 0,
            output_tokens BIGINT NOT NULL DEFAULT 0,
            started_at BIGINT NOT NULL,
            finished_at BIGINT NULL,
            INDEX idx_agent_runs_org (org_id, started_at),
            CONSTRAINT fk_agent_run_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
        )",
        "CREATE TABLE IF NOT EXISTS AgentActions (
            id VARCHAR(36) PRIMARY KEY,
            seq BIGINT NOT NULL AUTO_INCREMENT UNIQUE,
            run_id VARCHAR(36) NOT NULL,
            org_id VARCHAR(36) NOT NULL,
            agent VARCHAR(64) NOT NULL,
            tool VARCHAR(64) NOT NULL,
            input TEXT NOT NULL,
            reason TEXT NOT NULL,
            risk VARCHAR(16) NOT NULL,
            status VARCHAR(20) NOT NULL,
            result TEXT NULL,
            dedupe_key VARCHAR(191) NOT NULL,
            created_at BIGINT NOT NULL,
            decided_at BIGINT NULL,
            decided_by VARCHAR(36) NULL,
            INDEX idx_agent_actions_dedupe (org_id, dedupe_key),
            CONSTRAINT fk_agent_action_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
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

pub fn get_mode(org_id: Uuid) -> Result<AutonomyMode, String> {
    let mut conn = connect()?;
    let mode: Option<String> = conn
        .exec_first(
            "SELECT mode FROM AgentSettings WHERE org_id = :org_id",
            params! { "org_id" => org_id.to_string() },
        )
        .map_err(|e| e.to_string())?;
    Ok(mode.map_or(AutonomyMode::Off, |m| AutonomyMode::from_db(&m)))
}

pub fn set_mode(org_id: Uuid, mode: AutonomyMode) -> Result<(), String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "INSERT INTO AgentSettings (org_id, mode, updated_at) VALUES (:org_id, :mode, :now)
         ON DUPLICATE KEY UPDATE mode = :mode, updated_at = :now",
        params! { "org_id" => org_id.to_string(), "mode" => mode.as_str(), "now" => now_unix() },
    )
    .map_err(|e| e.to_string())
}

pub fn orgs_with_agents_on() -> Result<Vec<Uuid>, String> {
    let mut conn = connect()?;
    let ids: Vec<String> = conn
        .query("SELECT org_id FROM AgentSettings WHERE mode <> 'OFF'")
        .map_err(|e| e.to_string())?;
    Ok(ids.iter().filter_map(|s| Uuid::parse_str(s).ok()).collect())
}

/// Tokens this org's runs have used in the last 24 hours.
pub fn tokens_used_last_day(org_id: Uuid) -> Result<i64, String> {
    let mut conn = connect()?;
    let used: Option<i64> = conn
        .exec_first(
            "SELECT CAST(COALESCE(SUM(input_tokens + output_tokens), 0) AS SIGNED) FROM AgentRuns
             WHERE org_id = :org_id AND started_at >= :since",
            params! { "org_id" => org_id.to_string(), "since" => now_unix() - 86_400 },
        )
        .map_err(|e| e.to_string())?;
    Ok(used.unwrap_or(0))
}

/// Whether a run started after `since` is still going. A process that died
/// mid-run leaves `RUNNING` behind, which `since` stops from blocking forever.
pub fn has_active_run(org_id: Uuid, since: i64) -> Result<bool, String> {
    let mut conn = connect()?;
    let found: Option<i64> = conn
        .exec_first(
            "SELECT 1 FROM AgentRuns WHERE org_id = :org_id AND status = 'RUNNING' AND started_at >= :since LIMIT 1",
            params! { "org_id" => org_id.to_string(), "since" => since },
        )
        .map_err(|e| e.to_string())?;
    Ok(found.is_some())
}

pub fn start_run(org_id: Uuid, agent: &str, trigger_source: &str) -> Result<Uuid, String> {
    let mut conn = connect()?;
    let id = Uuid::new_v4();
    conn.exec_drop(
        "INSERT INTO AgentRuns (id, org_id, agent, trigger_source, status, started_at)
         VALUES (:id, :org_id, :agent, :trigger_source, 'RUNNING', :now)",
        params! {
            "id" => id.to_string(), "org_id" => org_id.to_string(), "agent" => agent,
            "trigger_source" => trigger_source, "now" => now_unix(),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn finish_run(
    id: Uuid,
    status: &str,
    summary: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<(), String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "UPDATE AgentRuns SET status = :status, summary = :summary, input_tokens = :input_tokens,
         output_tokens = :output_tokens, finished_at = :now WHERE id = :id",
        params! {
            "status" => status, "summary" => summary, "input_tokens" => input_tokens,
            "output_tokens" => output_tokens, "now" => now_unix(), "id" => id.to_string(),
        },
    )
    .map_err(|e| e.to_string())
}

type RunRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    i64,
    i64,
    i64,
    Option<i64>,
);

const RUN_COLS: &str = "id, org_id, agent, trigger_source, status, summary, input_tokens, output_tokens, started_at, finished_at";

fn row_to_run(r: RunRow) -> AgentRun {
    AgentRun {
        id: Uuid::parse_str(&r.0).unwrap_or_default(),
        org_id: Uuid::parse_str(&r.1).unwrap_or_default(),
        agent: r.2,
        trigger_source: r.3,
        status: r.4,
        summary: r.5,
        input_tokens: r.6,
        output_tokens: r.7,
        started_at: r.8,
        finished_at: r.9,
    }
}

pub fn get_run(id: Uuid) -> Result<Option<AgentRun>, String> {
    let mut conn = connect()?;
    let row: Option<RunRow> = conn
        .exec_first(
            format!("SELECT {RUN_COLS} FROM AgentRuns WHERE id = :id"),
            params! { "id" => id.to_string() },
        )
        .map_err(|e| e.to_string())?;
    Ok(row.map(row_to_run))
}

pub fn list_runs(org_id: Uuid, limit: u32) -> Result<Vec<AgentRun>, String> {
    let mut conn = connect()?;
    let rows: Vec<RunRow> = conn
        .exec(
            format!(
                "SELECT {RUN_COLS} FROM AgentRuns WHERE org_id = :org_id ORDER BY seq DESC LIMIT {limit}"
            ),
            params! { "org_id" => org_id.to_string() },
        )
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(row_to_run).collect())
}

/// The status of an action already taken or awaiting a decision for this key,
/// so an agent doesn't ask again on every run. A rejection counts: being told
/// no once is enough. Shadow rows only count in shadow mode, so turning on
/// real autonomy starts clean.
pub fn existing_action(
    org_id: Uuid,
    dedupe_key: &str,
    mode: AutonomyMode,
) -> Result<Option<ActionStatus>, String> {
    let mut conn = connect()?;
    let shadow_clause = if mode == AutonomyMode::Shadow {
        ""
    } else {
        "AND status <> 'SHADOWED'"
    };
    let status: Option<String> = conn
        .exec_first(
            format!(
                "SELECT status FROM AgentActions
                 WHERE org_id = :org_id AND dedupe_key = :dedupe_key AND status <> 'FAILED' {shadow_clause}
                 ORDER BY seq DESC LIMIT 1"
            ),
            params! { "org_id" => org_id.to_string(), "dedupe_key" => dedupe_key },
        )
        .map_err(|e| e.to_string())?;
    Ok(status.map(|s| ActionStatus::from_db(&s)))
}

pub struct NewAction<'a> {
    pub run_id: Uuid,
    pub org_id: Uuid,
    pub agent: &'a str,
    pub tool: &'a str,
    pub input: &'a str,
    pub reason: &'a str,
    pub risk: Risk,
    pub status: ActionStatus,
    pub result: Option<&'a str>,
    pub dedupe_key: &'a str,
}

pub fn record_action(new: NewAction) -> Result<Uuid, String> {
    let mut conn = connect()?;
    let id = Uuid::new_v4();
    conn.exec_drop(
        "INSERT INTO AgentActions (id, run_id, org_id, agent, tool, input, reason, risk, status, result, dedupe_key, created_at)
         VALUES (:id, :run_id, :org_id, :agent, :tool, :input, :reason, :risk, :status, :result, :dedupe_key, :now)",
        params! {
            "id" => id.to_string(), "run_id" => new.run_id.to_string(),
            "org_id" => new.org_id.to_string(), "agent" => new.agent, "tool" => new.tool,
            "input" => new.input, "reason" => new.reason, "risk" => new.risk.as_str(),
            "status" => new.status.as_str(), "result" => new.result,
            "dedupe_key" => new.dedupe_key, "now" => now_unix(),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

const ACTION_COLS: &str = "id, run_id, org_id, agent, tool, input, reason, risk, status, result, created_at, decided_at, decided_by";

/// By index, because the crate's tuple rows stop at 12 columns.
fn row_to_action(r: Row) -> AgentAction {
    let text = |i: usize| r.get::<String, _>(i).unwrap_or_default();
    AgentAction {
        id: Uuid::parse_str(&text(0)).unwrap_or_default(),
        run_id: Uuid::parse_str(&text(1)).unwrap_or_default(),
        org_id: Uuid::parse_str(&text(2)).unwrap_or_default(),
        agent: text(3),
        tool: text(4),
        input: text(5),
        reason: text(6),
        risk: Risk::from_db(&text(7)),
        status: ActionStatus::from_db(&text(8)),
        result: r.get::<Option<String>, _>(9).flatten(),
        created_at: r.get::<i64, _>(10).unwrap_or(0),
        decided_at: r.get::<Option<i64>, _>(11).flatten(),
        decided_by: r.get::<Option<String>, _>(12).flatten(),
    }
}

pub fn get_action(id: Uuid) -> Result<Option<AgentAction>, String> {
    let mut conn = connect()?;
    let row: Option<Row> = conn
        .exec_first(
            format!("SELECT {ACTION_COLS} FROM AgentActions WHERE id = :id"),
            params! { "id" => id.to_string() },
        )
        .map_err(|e| e.to_string())?;
    Ok(row.map(row_to_action))
}

/// Newest first, optionally for one status.
pub fn list_actions(
    org_id: Uuid,
    status: Option<ActionStatus>,
    limit: u32,
) -> Result<Vec<AgentAction>, String> {
    let mut conn = connect()?;
    let rows: Vec<Row> = conn
        .exec(
            format!(
                "SELECT {ACTION_COLS} FROM AgentActions
                 WHERE org_id = :org_id AND (:status IS NULL OR status = :status)
                 ORDER BY seq DESC LIMIT {limit}"
            ),
            params! {
                "org_id" => org_id.to_string(),
                "status" => status.map(|s| s.as_str()),
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(row_to_action).collect())
}

/// Take a pending action for a decision. Only one caller can win, so a double
/// click or two people approving at once can't carry it out twice.
pub fn claim_pending(
    id: Uuid,
    next: ActionStatus,
    decided_by: Option<Uuid>,
) -> Result<bool, String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "UPDATE AgentActions SET status = :next, decided_at = :now, decided_by = :decided_by
         WHERE id = :id AND status = 'PENDING_APPROVAL'",
        params! {
            "next" => next.as_str(), "now" => now_unix(),
            "decided_by" => decided_by.map(|u| u.to_string()), "id" => id.to_string(),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.affected_rows() == 1)
}

pub fn finish_action(id: Uuid, status: ActionStatus, result: &str) -> Result<(), String> {
    let mut conn = connect()?;
    conn.exec_drop(
        "UPDATE AgentActions SET status = :status, result = :result WHERE id = :id",
        params! { "status" => status.as_str(), "result" => result, "id" => id.to_string() },
    )
    .map_err(|e| e.to_string())
}
