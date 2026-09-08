use super::*;
use crate::logistics::customer::customer::Customer;
use crate::logistics::db::connection::DbConnection;
use crate::logistics::dispatch::dispatch::{
    DispatchLineItemInput, DispatchOrder, DispatchStatus, ProofOfDeliveryInput,
};
use crate::logistics::driver::driver::Driver;
use crate::logistics::godown::godown::Godown;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::stock::stock::Stock;
use crate::logistics::test_support::TestDb;
use crate::logistics::vehicle::vehicle::{Unit, Vehicle};
use mysql::prelude::*;
use mysql::params;

/// An org set up so `dispatch_stock_to_customer` will succeed: one located
/// customer, `vehicle_count` located vehicles each with an active driver and
/// plenty of capacity, and one godown holding a big pile of "Widget".
struct Fixture {
    org: Organization,
    customer: Customer,
}

fn seed(name: &str, vehicle_count: usize) -> Fixture {
    let mut org = Organization::create_organization(name, "1 Depot Rd").expect("org");
    org.update_location(19.0, 72.0, Some("HQ")).expect("org loc");

    let godown = Godown::create(org.id, "Main", "Dock 1", Some(1_000_000)).expect("godown");
    Stock::new(1, 100_000, "Widget").add_to_godown(godown.id).expect("stock");

    for i in 0..vehicle_count {
        let driver = Driver::create(org.id, &format!("Driver {i}"), &format!("LIC{i}"), "0")
            .expect("driver");
        let mut v = Vehicle::new(format!("MH00 RP {i:04}"), 1_000_000, Unit::MetricTon);
        v.add_new_vehicle_to_org(&org).expect("vehicle");
        v.update_location(19.0, 72.0, Some("HQ")).expect("v loc");
        v.assign_driver(Some(driver.id)).expect("assign");
    }

    let mut customer = Customer::create_customer(org.id, "Buyer", "Market Rd").expect("customer");
    customer.update_location(19.1, 72.1, Some("Market")).expect("cust loc");

    Fixture { org, customer }
}

fn line(qty: i64) -> DispatchLineItemInput {
    DispatchLineItemInput { stock_description: "Widget".to_string(), requested_quantity: qty }
}

/// Rewrite a dispatch's creation time and every status-history timestamp by
/// the same offset, to simulate an order that happened `seconds_ago` back.
fn backdate_dispatch(dispatch_id: uuid::Uuid, seconds_ago: i64) {
    let mut conn = DbConnection::from_env().get_connection().unwrap();
    let cutoff = now_unix() - seconds_ago;
    conn.exec_drop(
        "UPDATE Dispatches SET dispatched_at = :t WHERE id = :id",
        params! { "t" => cutoff, "id" => dispatch_id.to_string() },
    )
    .unwrap();
    conn.exec_drop(
        "UPDATE DispatchStatusHistory SET changed_at = :t
         WHERE dispatch_id = :id AND status = 'PENDING'",
        params! { "t" => cutoff, "id" => dispatch_id.to_string() },
    )
    .unwrap();
}

fn set_delivered_at(dispatch_id: uuid::Uuid, seconds_ago: i64) {
    let mut conn = DbConnection::from_env().get_connection().unwrap();
    conn.exec_drop(
        "UPDATE DispatchStatusHistory SET changed_at = :t
         WHERE dispatch_id = :id AND status = 'DELIVERED'",
        params! { "t" => now_unix() - seconds_ago, "id" => dispatch_id.to_string() },
    )
    .unwrap();
}

fn pod() -> Option<ProofOfDeliveryInput> {
    Some(ProofOfDeliveryInput {
        receiver_name: "R".to_string(),
        signature_or_photo_url: "https://x/y.png".to_string(),
    })
}

fn deliver(order: &mut DispatchOrder) {
    order.transition_to(DispatchStatus::Confirmed, None, None).unwrap();
    order.transition_to(DispatchStatus::Loaded, None, None).unwrap();
    order.transition_to(DispatchStatus::InTransit, None, None).unwrap();
    order.transition_to(DispatchStatus::Delivered, pod(), None).unwrap();
}

#[test]
fn empty_org_reports_all_zeros() {
    let _db = TestDb::create();
    let org = Organization::create_organization("Empty Co", "0 St").expect("org");

    let r = OpsReport::for_org(org.id).expect("report");
    assert_eq!(r.vehicle_utilization.total_vehicles, 0);
    assert_eq!(r.vehicle_utilization.utilization_percent, 0.0);
    assert_eq!(r.delivery_performance.delivered_count, 0);
    assert_eq!(r.delivery_performance.avg_hours_to_deliver, None);
    assert_eq!(r.delivery_performance.on_time_rate_percent, None);
    assert_eq!(r.units_dispatched_recently, 0);
    assert!(r.godown_inventory.is_empty());
    assert_eq!(r.dispatch_volume.len(), 14);
    assert!(r.dispatch_volume.iter().all(|p| p.count == 0));
    // Oldest first, and today is the last point.
    assert_eq!(r.dispatch_volume[13].date, iso_date_from_epoch_day(now_unix().div_euclid(86_400)));
}

#[test]
fn vehicle_utilization_counts_only_non_terminal_trips() {
    let _db = TestDb::create();
    let fx = seed("Util Co", 3);

    // One active dispatch (stays PENDING), one delivered (terminal).
    fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("active");
    let mut done = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("done");
    deliver(&mut done);

    let r = OpsReport::for_org(fx.org.id).expect("report");
    assert_eq!(r.vehicle_utilization.total_vehicles, 3);
    assert_eq!(r.vehicle_utilization.vehicles_on_active_trip, 1);
    assert_eq!(r.vehicle_utilization.utilization_percent, 33.3);
}

#[test]
fn delivery_performance_averages_and_on_time_rate() {
    let _db = TestDb::create();
    let fx = seed("Delivery Co", 3);

    // Fast delivery: created and delivered ~now → ~0h, on time.
    let mut fast = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("fast");
    deliver(&mut fast);

    // Slow delivery: created 100h ago, delivered 20h ago → 80h, not on time.
    let mut slow = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("slow");
    deliver(&mut slow);
    backdate_dispatch(slow.id, 100 * 3600);
    set_delivered_at(slow.id, 20 * 3600);

    // One returned dispatch.
    let mut returned = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("ret");
    returned.transition_to(DispatchStatus::Confirmed, None, None).unwrap();
    returned.transition_to(DispatchStatus::Loaded, None, None).unwrap();
    returned.transition_to(DispatchStatus::InTransit, None, None).unwrap();
    returned.transition_to(DispatchStatus::Returned, None, None).unwrap();

    let r = OpsReport::for_org(fx.org.id).expect("report");
    assert_eq!(r.delivery_performance.delivered_count, 2);
    assert_eq!(r.delivery_performance.returned_count, 1);
    // avg of ~0h and 80h ≈ 40h (allow slack for the "~0" leg).
    let avg = r.delivery_performance.avg_hours_to_deliver.expect("avg");
    assert!((39.0..=41.0).contains(&avg), "avg was {avg}");
    // 1 of 2 on time.
    assert_eq!(r.delivery_performance.on_time_rate_percent, Some(50.0));
}

#[test]
fn dispatch_volume_buckets_by_calendar_day() {
    let _db = TestDb::create();
    let fx = seed("Volume Co", 5);

    // Two today, one 3 days ago, one 20 days ago (outside the 14-day window).
    let a = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("a");
    let b = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("b");
    let _ = (a, b);
    let c = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("c");
    backdate_dispatch(c.id, 3 * 86_400 + 3600);
    let d = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(1)]).expect("d");
    backdate_dispatch(d.id, 20 * 86_400);

    let r = OpsReport::for_org(fx.org.id).expect("report");
    assert_eq!(r.dispatch_volume.len(), 14);
    assert_eq!(r.dispatch_volume[13].count, 2, "today");
    assert_eq!(r.dispatch_volume[10].count, 1, "3 days ago");
    // The 20-days-ago one is outside the window: total across the series is 3.
    assert_eq!(r.dispatch_volume.iter().map(|p| p.count).sum::<i64>(), 3);
    // Dates ascend.
    assert!(r.dispatch_volume.windows(2).all(|w| w[0].date < w[1].date));
}

#[test]
fn godown_inventory_sums_stock_and_computes_capacity_use() {
    let _db = TestDb::create();
    let org = Organization::create_organization("Godown Co", "1 St").expect("org");

    let capped = Godown::create(org.id, "Capped", "A", Some(1000)).expect("g1");
    Stock::new(2, 100, "Bolts").add_to_godown(capped.id).expect("s1"); // vol 200
    Stock::new(5, 40, "Beams").add_to_godown(capped.id).expect("s2"); // vol 200 → 400/1000

    let uncapped = Godown::create(org.id, "Uncapped", "B", None).expect("g2");
    Stock::new(1, 10, "Nuts").add_to_godown(uncapped.id).expect("s3");

    let r = OpsReport::for_org(org.id).expect("report");
    let cap = r.godown_inventory.iter().find(|g| g.godown_name == "Capped").unwrap();
    assert_eq!(cap.units_on_hand, 140);
    assert_eq!(cap.distinct_items, 2);
    assert_eq!(cap.capacity_used_percent, Some(40.0));

    let unc = r.godown_inventory.iter().find(|g| g.godown_name == "Uncapped").unwrap();
    assert_eq!(unc.units_on_hand, 10);
    assert_eq!(unc.capacity_used_percent, None);
}

#[test]
fn units_dispatched_recently_ignores_old_dispatches() {
    let _db = TestDb::create();
    let fx = seed("Recent Co", 3);

    fx.org.dispatch_stock_to_customer(&fx.customer, &[line(30)]).expect("recent");
    let old = fx.org.dispatch_stock_to_customer(&fx.customer, &[line(70)]).expect("old");
    backdate_dispatch(old.id, 40 * 86_400); // older than the 30-day window

    let r = OpsReport::for_org(fx.org.id).expect("report");
    assert_eq!(r.units_dispatched_recently, 30);
}
