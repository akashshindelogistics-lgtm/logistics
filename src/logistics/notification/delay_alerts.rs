//! Background delay-alert scan: finds every dispatch still `IN_TRANSIT` past
//! its promised delivery window
//! ([`DispatchOrder::is_running_late`](crate::logistics::dispatch::dispatch::DispatchOrder::is_running_late))
//! and sends the customer a "your order is running behind schedule"
//! notification through the existing Twilio/Resend delivery pipeline —
//! exactly the same mechanism [`crate::logistics::notification::notification`]
//! already uses for the dispatch-created / dispatch-delivered events, just a
//! new trigger condition. See `docs/delay-alerts.md`.
//!
//! [`scan_and_alert`] is meant to be called on a timer (see `main.rs`), not
//! per request — there is no user-facing route for it. It's idempotent: a
//! dispatch that already carries a
//! [`NotificationEvent::DispatchRunningLate`] notification is never alerted
//! on again, so calling it as often as you like (or concurrently) never
//! double-sends.

use crate::logistics::customer::customer::Customer;
use crate::logistics::dispatch::dispatch::DispatchOrder;
use crate::logistics::notification::notification::{Notification, NotificationEvent};
use std::error::Error;

/// Scan every `IN_TRANSIT` dispatch across every organization, and for each
/// one that's running late and hasn't already been alerted on, record and
/// attempt to send a [`NotificationEvent::DispatchRunningLate`] notification
/// to its customer. Returns how many new alerts were sent.
///
/// An error on one dispatch (e.g. its customer has since been deleted) is
/// logged and skipped rather than aborting the whole scan — one bad row must
/// not stop every other late dispatch from being alerted on.
pub async fn scan_and_alert() -> Result<usize, Box<dyn Error>> {
    let late: Vec<DispatchOrder> = DispatchOrder::list_in_transit()?
        .into_iter()
        .filter(|d| d.is_running_late())
        .collect();

    let mut sent = 0;
    for dispatch in &late {
        match alert_one(dispatch).await {
            Ok(true) => sent += 1,
            Ok(false) => {}
            Err(err) => eprintln!("delay-alert scan: dispatch {}: {err}", dispatch.id),
        }
    }
    Ok(sent)
}

/// Alert on one dispatch, unless it's already been alerted on or its
/// customer can no longer be found. Returns whether a new alert was sent.
async fn alert_one(dispatch: &DispatchOrder) -> Result<bool, Box<dyn Error>> {
    let already_alerted = Notification::list_by_dispatch(dispatch.id)?
        .iter()
        .any(|n| n.event == NotificationEvent::DispatchRunningLate);
    if already_alerted {
        return Ok(false);
    }

    let Some(customer) = Customer::get_by_id(dispatch.customer_id)? else {
        return Ok(false);
    };
    let hours_late = dispatch.hours_late().unwrap_or(0);

    let notifs =
        Notification::record_dispatch_running_late(dispatch.org_id, dispatch.id, &customer, hours_late)
            .await?;
    Notification::deliver_queued_best_effort(&notifs).await;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logistics::db::connection::DbConnection;
    use crate::logistics::dispatch::dispatch::{DispatchLineItemInput, DispatchStatus, ProofOfDeliveryInput};
    use crate::logistics::driver::driver::Driver;
    use crate::logistics::godown::godown::Godown;
    use crate::logistics::notification::notification::NotificationStatus;
    use crate::logistics::orgs::orgs::Organization;
    use crate::logistics::stock::stock::Stock;
    use crate::logistics::test_support::TestDb;
    use crate::logistics::vehicle::vehicle::{Unit, Vehicle};
    use mysql::params;
    use mysql::prelude::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_unix() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// A saved, `IN_TRANSIT` dispatch for a fresh org/customer/vehicle/godown,
    /// with `dispatched_at` backdated `hours_ago` hours so its lateness is
    /// under the test's control.
    fn in_transit_dispatch_hours_ago(hours_ago: i64) -> (Organization, Customer, DispatchOrder) {
        let mut org = Organization::create_organization("Delay Co", "1 Depot Rd").expect("org");
        org.update_location(19.0, 72.0, Some("HQ")).expect("org loc");

        let driver = Driver::create(org.id, "Driver", "LIC1", "0").expect("driver");
        let mut vehicle = Vehicle::new("MH00 DL 0001".to_string(), 1_000_000, Unit::MetricTon);
        vehicle.add_new_vehicle_to_org(&org).expect("vehicle");
        vehicle.update_location(19.0, 72.0, Some("HQ")).expect("v loc");
        vehicle.assign_driver(Some(driver.id)).expect("assign");

        let godown = Godown::create(org.id, "Main", "Dock 1", Some(1_000_000)).expect("godown");
        Stock::new(1, 1000, "Widget").add_to_godown(godown.id).expect("stock");

        let mut customer = Customer::create_customer(org.id, "Buyer", "Market Rd").expect("customer");
        customer.update_location(19.1, 72.1, Some("Market")).expect("cust loc");
        customer.set_contact(None, Some("buyer@example.com".into())).expect("contact");

        let mut order = org
            .dispatch_stock_to_customer(
                &customer,
                &[DispatchLineItemInput { stock_description: "Widget".to_string(), requested_quantity: 5 }],
            )
            .expect("dispatch");
        order.transition_to(DispatchStatus::Confirmed, None, None).expect("confirmed");
        order.transition_to(DispatchStatus::Loaded, None, None).expect("loaded");
        order.transition_to(DispatchStatus::InTransit, None, None).expect("in transit");

        let backdated = now_unix() - hours_ago * 3600;
        let mut conn = DbConnection::from_env().get_connection().unwrap();
        conn.exec_drop(
            "UPDATE Dispatches SET dispatched_at = :t WHERE id = :id",
            params! { "t" => backdated, "id" => order.id.to_string() },
        )
        .unwrap();
        order.dispatched_at = backdated;

        (org, customer, order)
    }

    #[actix_web::test]
    async fn scan_and_alert_notifies_the_customer_of_an_overdue_in_transit_dispatch() {
        let _db = TestDb::create();
        let (_, _, order) = in_transit_dispatch_hours_ago(100); // 100h > the 72h target

        let sent = scan_and_alert().await.expect("scan");
        assert_eq!(sent, 1);

        let notifs = Notification::list_by_dispatch(order.id).expect("list");
        let alert = notifs
            .iter()
            .find(|n| n.event == NotificationEvent::DispatchRunningLate)
            .expect("alert recorded");
        assert_eq!(alert.recipient_kind, "customer");
        assert_eq!(alert.status, NotificationStatus::Queued);
        assert!(alert.body.to_lowercase().contains("behind"), "{}", alert.body);
    }

    #[actix_web::test]
    async fn scan_and_alert_never_sends_the_same_alert_twice() {
        let _db = TestDb::create();
        let (_, _, order) = in_transit_dispatch_hours_ago(100);

        assert_eq!(scan_and_alert().await.expect("first scan"), 1);
        assert_eq!(scan_and_alert().await.expect("second scan"), 0);

        let notifs = Notification::list_by_dispatch(order.id).expect("list");
        assert_eq!(
            notifs.iter().filter(|n| n.event == NotificationEvent::DispatchRunningLate).count(),
            1,
            "should never double-send: {notifs:?}"
        );
    }

    #[actix_web::test]
    async fn scan_and_alert_skips_a_dispatch_still_within_its_promised_window() {
        let _db = TestDb::create();
        let (_, _, order) = in_transit_dispatch_hours_ago(10); // well under 72h

        assert_eq!(scan_and_alert().await.expect("scan"), 0);
        assert!(Notification::list_by_dispatch(order.id).expect("list").is_empty());
    }

    #[actix_web::test]
    async fn scan_and_alert_skips_dispatches_that_are_no_longer_in_transit() {
        let _db = TestDb::create();
        let (_, _, mut order) = in_transit_dispatch_hours_ago(100);

        // Delivered — no longer IN_TRANSIT, so it drops out of the scan even
        // though it was created well over 72h ago.
        order
            .transition_to(
                DispatchStatus::Delivered,
                Some(ProofOfDeliveryInput {
                    receiver_name: "R".to_string(),
                    signature_or_photo_url: "https://x/y.png".to_string(),
                }),
                None,
            )
            .expect("delivered");

        assert_eq!(scan_and_alert().await.expect("scan"), 0);
    }
}
