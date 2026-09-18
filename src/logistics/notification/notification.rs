//! Dispatch notifications to the customer and driver.
//!
//! When a dispatch is created, and again when it's delivered, the system
//! builds a notification for each party it should tell — its body
//! AI-personalized via [`crate::logistics::ai::notification_copy`] when
//! `ANTHROPIC_API_KEY` is configured, falling back to a fixed template on
//! any error — records it, then immediately attempts to actually send it via
//! [`crate::logistics::notification::delivery`] (Twilio for SMS, Resend for
//! email). Every row lands with status `QUEUED`
//! (ready to send), `SKIPPED` (no phone/email on file for that party),
//! `SENT` (delivery succeeded), or `FAILED` (delivery was attempted and the
//! provider reported an error). A `QUEUED` row whose channel has no
//! provider credentials configured is left `QUEUED` rather than `FAILED` —
//! nothing was actually attempted, and it's ready to send once configured.
//! See `docs/notifications.md`.

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
    /// A dispatch is still `IN_TRANSIT` past
    /// [`crate::logistics::dispatch::dispatch::PROMISED_DELIVERY_HOURS`] since
    /// it was created. Recorded by
    /// [`crate::logistics::notification::delay_alerts::scan_and_alert`].
    DispatchRunningLate,
}

impl NotificationEvent {
    fn as_str(&self) -> &'static str {
        match self {
            NotificationEvent::DispatchCreated => "DISPATCH_CREATED",
            NotificationEvent::DispatchDelivered => "DISPATCH_DELIVERED",
            NotificationEvent::DispatchRunningLate => "DISPATCH_RUNNING_LATE",
        }
    }

    /// The email subject line for this event. Only used for
    /// [`NotificationChannel::Email`] — SMS has no subject.
    fn email_subject(&self) -> &'static str {
        match self {
            NotificationEvent::DispatchCreated => "Your order is on its way",
            NotificationEvent::DispatchDelivered => "Your order has been delivered",
            NotificationEvent::DispatchRunningLate => "Your order is running behind schedule",
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
    /// A message ready for a provider to send. Also the resting state for a
    /// channel with no provider configured — an attempt was never made.
    Queued,
    /// No phone or email on file for this party — nothing to send.
    Skipped,
    /// Delivery succeeded.
    Sent,
    /// Delivery was attempted and the provider reported an error.
    Failed,
}

impl NotificationStatus {
    fn as_str(&self) -> &'static str {
        match self {
            NotificationStatus::Queued => "QUEUED",
            NotificationStatus::Skipped => "SKIPPED",
            NotificationStatus::Sent => "SENT",
            NotificationStatus::Failed => "FAILED",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "SKIPPED" => NotificationStatus::Skipped,
            "SENT" => NotificationStatus::Sent,
            "FAILED" => NotificationStatus::Failed,
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
            event: match event.as_str() {
                "DISPATCH_DELIVERED" => NotificationEvent::DispatchDelivered,
                "DISPATCH_RUNNING_LATE" => NotificationEvent::DispatchRunningLate,
                _ => NotificationEvent::DispatchCreated,
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
    /// The body for each is AI-personalized when `ANTHROPIC_API_KEY` is
    /// configured (see `crate::logistics::ai::notification_copy`), falling
    /// back to a fixed template on any error — a misconfigured or
    /// rate-limited environment must never block recording a dispatch.
    /// Generation is skipped entirely for a row with no contact to send to,
    /// since nobody will ever see that copy.
    pub async fn record_dispatch_created(
        org_id: Uuid,
        dispatch_id: Uuid,
        customer: &Customer,
        driver_phone: Option<&str>,
    ) -> Result<Vec<Notification>, Box<dyn Error>> {
        let short_id = &dispatch_id.to_string()[..8];
        let customer_target = Self::customer_target(customer);
        let driver_target = driver_phone
            .filter(|s| !s.is_empty())
            .map(|p| (NotificationChannel::Sms, p.to_string()));

        let customer_body = if customer_target.is_some() {
            crate::logistics::ai::notification_copy::generate_dispatch_created_customer_body(&customer.name, short_id)
                .await
                .unwrap_or_else(|_| {
                    format!("Hi {}, your order (ref {short_id}) has been dispatched and is on its way.", customer.name)
                })
        } else {
            format!("Hi {}, your order (ref {short_id}) has been dispatched and is on its way.", customer.name)
        };

        let driver_body = if driver_target.is_some() {
            crate::logistics::ai::notification_copy::generate_dispatch_created_driver_body(&customer.name, short_id)
                .await
                .unwrap_or_else(|_| format!("New trip assigned: dispatch {short_id} to {}.", customer.name))
        } else {
            format!("New trip assigned: dispatch {short_id} to {}.", customer.name)
        };

        let notifs = vec![
            Self::build(org_id, dispatch_id, NotificationEvent::DispatchCreated, "customer", customer_target, customer_body),
            Self::build(org_id, dispatch_id, NotificationEvent::DispatchCreated, "driver", driver_target, driver_body),
        ];
        Self::persist(&notifs)?;
        Ok(notifs)
    }

    /// Record the "your shipment was delivered" notification for the
    /// customer. AI-personalized / fallback behavior matches
    /// [`Self::record_dispatch_created`].
    pub async fn record_dispatch_delivered(
        org_id: Uuid,
        dispatch_id: Uuid,
        customer: &Customer,
    ) -> Result<Vec<Notification>, Box<dyn Error>> {
        let short_id = &dispatch_id.to_string()[..8];
        let customer_target = Self::customer_target(customer);

        let customer_body = if customer_target.is_some() {
            crate::logistics::ai::notification_copy::generate_dispatch_delivered_customer_body(&customer.name, short_id)
                .await
                .unwrap_or_else(|_| format!("Hi {}, your order (ref {short_id}) has been delivered. Thank you!", customer.name))
        } else {
            format!("Hi {}, your order (ref {short_id}) has been delivered. Thank you!", customer.name)
        };

        let notifs = vec![Self::build(
            org_id,
            dispatch_id,
            NotificationEvent::DispatchDelivered,
            "customer",
            customer_target,
            customer_body,
        )];
        Self::persist(&notifs)?;
        Ok(notifs)
    }

    /// Record the "your shipment is running behind schedule" notification
    /// for the customer, when a dispatch is still `IN_TRANSIT` past
    /// [`crate::logistics::dispatch::dispatch::PROMISED_DELIVERY_HOURS`].
    /// Called by [`crate::logistics::notification::delay_alerts::scan_and_alert`],
    /// which is also responsible for not calling this twice for the same
    /// dispatch. AI-personalized / fallback behavior matches
    /// [`Self::record_dispatch_created`].
    pub async fn record_dispatch_running_late(
        org_id: Uuid,
        dispatch_id: Uuid,
        customer: &Customer,
        hours_late: i64,
    ) -> Result<Vec<Notification>, Box<dyn Error>> {
        let short_id = &dispatch_id.to_string()[..8];
        let customer_target = Self::customer_target(customer);

        let customer_body = if customer_target.is_some() {
            crate::logistics::ai::notification_copy::generate_dispatch_running_late_customer_body(&customer.name, short_id, hours_late)
                .await
                .unwrap_or_else(|_| {
                    format!(
                        "Hi {}, your order (ref {short_id}) is running about {hours_late}h behind its expected delivery time. We're sorry for the delay.",
                        customer.name
                    )
                })
        } else {
            format!(
                "Hi {}, your order (ref {short_id}) is running about {hours_late}h behind its expected delivery time. We're sorry for the delay.",
                customer.name
            )
        };

        let notifs = vec![Self::build(
            org_id,
            dispatch_id,
            NotificationEvent::DispatchRunningLate,
            "customer",
            customer_target,
            customer_body,
        )];
        Self::persist(&notifs)?;
        Ok(notifs)
    }

    fn persist(notifs: &[Notification]) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        Self::ensure_table(&mut conn)?;
        for n in notifs {
            Self::insert(&mut conn, n)?;
            // Best-effort: index this notification for the "ask your data"
            // assistant. Never allowed to fail recording the notification
            // itself — see chunk::upsert_best_effort.
            crate::logistics::ai::chunk::upsert_best_effort(
                n.org_id,
                crate::logistics::ai::chunk::ChunkKind::Notification,
                &n.id.to_string(),
                crate::logistics::ai::chunk::notification_chunk_text(n),
            );
        }
        Ok(())
    }

    fn update_status(id: Uuid, status: NotificationStatus) -> Result<(), Box<dyn Error>> {
        let mut conn = DbConnection::from_env().get_connection()?;
        conn.exec_drop(
            "UPDATE Notifications SET status = :status WHERE id = :id",
            params! { "status" => status.as_str(), "id" => id.to_string() },
        )?;
        Ok(())
    }

    /// Attempt to actually send every `QUEUED` notification in `notifs` via
    /// [`crate::logistics::notification::delivery::deliver`], updating each
    /// row's status to `SENT` or `FAILED` accordingly. Best-effort: a
    /// delivery failure only updates that row's status, it never propagates
    /// — the caller (recording a dispatch event) must not fail because a
    /// downstream SMS/email provider had a bad day. Rows whose channel has
    /// no provider configured are left `QUEUED`, not marked `FAILED`.
    pub async fn deliver_queued_best_effort(notifs: &[Notification]) {
        for n in notifs {
            if n.status != NotificationStatus::Queued {
                continue;
            }
            let subject = n.event.email_subject();
            match crate::logistics::notification::delivery::deliver(n.channel, &n.recipient, subject, &n.body).await
            {
                Ok(()) => {
                    if let Err(e) = Self::update_status(n.id, NotificationStatus::Sent) {
                        eprintln!("notification delivery: sent {} but failed to update its status: {e}", n.id);
                    }
                }
                Err(crate::logistics::notification::delivery::DeliveryError::NotConfigured) => {}
                Err(err) => {
                    eprintln!("notification delivery: failed to send {} to {}: {err}", n.id, n.recipient);
                    if let Err(e) = Self::update_status(n.id, NotificationStatus::Failed) {
                        eprintln!("notification delivery: also failed to mark {} as failed: {e}", n.id);
                    }
                }
            }
        }
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
