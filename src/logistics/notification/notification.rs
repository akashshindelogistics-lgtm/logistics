//! Dispatch notifications to the customer and driver.
//!
//! When a dispatch is created, and again when it's delivered, the system
//! records a notification for each party it should tell. Recording is all
//! this module does today: every row lands with status `QUEUED` (a message
//! ready to send) or `SKIPPED` (we had no phone/email for that party). Wiring
//! an actual SMS / email provider is a deployment concern — it reads the
//! `QUEUED` rows and marks them `SENT` / `FAILED`. See `docs/notifications.md`.

use crate::logistics::customer::customer::Customer;
use crate::logistics::db::connection::DbConnection;
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

/// What happened to the dispatch that we're telling someone about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationEvent {
    DispatchCreated,
    DispatchDelivered,
}

impl NotificationEvent {
    fn as_str(&self) -> &'static str {
        match self {
            NotificationEvent::DispatchCreated => "DISPATCH_CREATED",
            NotificationEvent::DispatchDelivered => "DISPATCH_DELIVERED",
        }
    }
}

/// How a notification would be delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationChannel {
    Sms,
    Email,
}

impl NotificationChannel {
    fn as_str(&self) -> &'static str {
        match self {
            NotificationChannel::Sms => "SMS",
            NotificationChannel::Email => "EMAIL",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "EMAIL" => NotificationChannel::Email,
            _ => NotificationChannel::Sms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationStatus {
    /// A message ready for a provider to send.
    Queued,
    /// No phone or email on file for this party — nothing to send.
    Skipped,
}

impl NotificationStatus {
    fn as_str(&self) -> &'static str {
        match self {
            NotificationStatus::Queued => "QUEUED",
            NotificationStatus::Skipped => "SKIPPED",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "SKIPPED" => NotificationStatus::Skipped,
            _ => NotificationStatus::Queued,
        }
    }
}

/// One recorded notification.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Notification {
    pub id: Uuid,
    pub org_id: Uuid,
    pub dispatch_id: Uuid,
    pub event: NotificationEvent,
    pub channel: NotificationChannel,
    /// `"customer"` or `"driver"`.
    pub recipient_kind: String,
    /// The phone number or email address, or a placeholder when `SKIPPED`.
    pub recipient: String,
    pub body: String,
    pub status: NotificationStatus,
    pub created_at: i64,
}

type Row = (String, String, String, String, String, String, String, String, String, i64);

const SELECT_COLS: &str =
    "id, org_id, dispatch_id, event, channel, recipient_kind, recipient, body, status, created_at";

impl Notification {
    pub fn ensure_table(conn: &mut mysql::PooledConn) -> Result<(), Box<dyn Error>> {
        conn.query_drop(
            "CREATE TABLE IF NOT EXISTS Notifications (
                id VARCHAR(36) PRIMARY KEY,
                seq BIGINT NOT NULL AUTO_INCREMENT UNIQUE,
                org_id VARCHAR(36) NOT NULL,
                dispatch_id VARCHAR(36) NOT NULL,
                event VARCHAR(32) NOT NULL,
                channel VARCHAR(16) NOT NULL,
                recipient_kind VARCHAR(16) NOT NULL,
                recipient VARCHAR(255) NOT NULL,
                body TEXT NOT NULL,
                status VARCHAR(16) NOT NULL,
                created_at BIGINT NOT NULL,
                CONSTRAINT fk_notification_org FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE
            )",
        )?;
        // `seq` (a stable newest-first ordering key) was added after the table
        // first shipped; back-fill it onto a local database that predates it.
        let has_seq: Option<i64> = conn.exec_first(
            "SELECT 1 FROM information_schema.columns
             WHERE table_schema = DATABASE() AND table_name = 'Notifications'
               AND column_name = 'seq'",
            (),
        )?;
        if has_seq.is_none() {
            conn.query_drop(
                "ALTER TABLE Notifications ADD COLUMN seq BIGINT NOT NULL AUTO_INCREMENT UNIQUE",
            )?;
        }
        Ok(())
    }

    fn row_to_notification(
        (id, org_id, dispatch_id, event, channel, recipient_kind, recipient, body, status, created_at): Row,
    ) -> Self {
        Notification {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
            org_id: Uuid::parse_str(&org_id).unwrap_or_else(|_| Uuid::new_v4()),
            dispatch_id: Uuid::parse_str(&dispatch_id).unwrap_or_else(|_| Uuid::new_v4()),
            event: if event == "DISPATCH_DELIVERED" {
                NotificationEvent::DispatchDelivered
            } else {
                NotificationEvent::DispatchCreated
            },
            channel: NotificationChannel::from_str(&channel),
            recipient_kind,
            recipient,
            body,
            status: NotificationStatus::from_str(&status),
            created_at,
        }
    }

    fn insert(conn: &mut mysql::PooledConn, n: &Notification) -> Result<(), Box<dyn Error>> {
        conn.exec_drop(
            "INSERT INTO Notifications (id, org_id, dispatch_id, event, channel, recipient_kind, recipient, body, status, created_at)
             VALUES (:id, :org_id, :dispatch_id, :event, :channel, :recipient_kind, :recipient, :body, :status, :created_at)",
            params! {
                "id" => n.id.to_string(),
                "org_id" => n.org_id.to_string(),
                "dispatch_id" => n.dispatch_id.to_string(),
                "event" => n.event.as_str(),
                "channel" => n.channel.as_str(),
                "recipient_kind" => &n.recipient_kind,
                "recipient" => &n.recipient,
                "body" => &n.body,
                "status" => n.status.as_str(),
                "created_at" => n.created_at,
            },
        )?;
        Ok(())
    }

    /// The best channel + address to reach a customer on: email if we have it,
    /// otherwise SMS, otherwise nothing.
    fn customer_target(customer: &Customer) -> Option<(NotificationChannel, String)> {
        if let Some(email) = customer.email.as_deref().filter(|s| !s.is_empty()) {
            Some((NotificationChannel::Email, email.to_string()))
        } else if let Some(phone) = customer.phone.as_deref().filter(|s| !s.is_empty()) {
            Some((NotificationChannel::Sms, phone.to_string()))
        } else {
            None
        }
    }

    fn build(
        org_id: Uuid,
        dispatch_id: Uuid,
        event: NotificationEvent,
        recipient_kind: &str,
        target: Option<(NotificationChannel, String)>,
        body: String,
    ) -> Notification {
        let (channel, recipient, status) = match target {
            Some((ch, to)) => (ch, to, NotificationStatus::Queued),
            None => (
                NotificationChannel::Sms,
                "(no contact on file)".to_string(),
                NotificationStatus::Skipped,
            ),
        };
        Notification {
            id: Uuid::new_v4(),
            org_id,
            dispatch_id,
            event,
            channel,
            recipient_kind: recipient_kind.to_string(),
            recipient,
            body,
            status,
            created_at: now_unix(),
        }
    }

    /// Record the "your shipment is on its way" notifications for a new
    /// dispatch: one for the customer, one for the driver (via `driver_phone`).
    pub fn record_dispatch_created(
        org_id: Uuid,
        dispatch_id: Uuid,
        customer: &Customer,
        driver_phone: Option<&str>,
    ) -> Result<Vec<Notification>, Box<dyn Error>> {
        let short_id = &dispatch_id.to_string()[..8];
        let notifs = vec![
            Self::build(
                org_id,
                dispatch_id,
                NotificationEvent::DispatchCreated,
                "customer",
                Self::customer_target(customer),
                format!(
                    "Hi {}, your order (ref {short_id}) has been dispatched and is on its way.",
                    customer.name
                ),
            ),
            Self::build(
                org_id,
                dispatch_id,
                NotificationEvent::DispatchCreated,
                "driver",
                driver_phone
                    .filter(|s| !s.is_empty())
                    .map(|p| (NotificationChannel::Sms, p.to_string())),
                format!("New trip assigned: dispatch {short_id} to {}.", customer.name),
            ),
        ];
        Self::persist(&notifs)?;
        Ok(notifs)
    }

    /// Record the "your shipment was delivered" notification for the customer.
    pub fn record_dispatch_delivered(
        org_id: Uuid,
        dispatch_id: Uuid,
        customer: &Customer,
    ) -> Result<Vec<Notification>, Box<dyn Error>> {
        let short_id = &dispatch_id.to_string()[..8];
        let notifs = vec![Self::build(
            org_id,
            dispatch_id,
            NotificationEvent::DispatchDelivered,
            "customer",
            Self::customer_target(customer),
            format!(
                "Hi {}, your order (ref {short_id}) has been delivered. Thank you!",
                customer.name
            ),
        )];
        Self::persist(&notifs)?;
        Ok(notifs)
    }

    fn persist(notifs: &[Notification]) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        for n in notifs {
            Self::insert(&mut conn, n)?;
        }
        Ok(())
    }

    pub fn list_by_dispatch(dispatch_id: Uuid) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let rows: Vec<Row> = conn.exec(
            format!(
                "SELECT {SELECT_COLS} FROM Notifications WHERE dispatch_id = :dispatch_id
                 ORDER BY seq ASC"
            ),
            params! { "dispatch_id" => dispatch_id.to_string() },
        )?;
        Ok(rows.into_iter().map(Self::row_to_notification).collect())
    }

    pub fn list_by_org(org_id: Uuid, limit: u32) -> Result<Vec<Self>, Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        let rows: Vec<Row> = conn.exec(
            format!(
                "SELECT {SELECT_COLS} FROM Notifications WHERE org_id = :org_id
                 ORDER BY seq DESC LIMIT :limit"
            ),
            params! { "org_id" => org_id.to_string(), "limit" => limit },
        )?;
        Ok(rows.into_iter().map(Self::row_to_notification).collect())
    }
}
