use super::notification::{Notification, NotificationChannel, NotificationEvent, NotificationStatus};
use crate::logistics::customer::customer::Customer;
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::test_support::TestDb;
use uuid::Uuid;

fn org() -> Organization {
    Organization::create_organization("Notify Co", "1 St").expect("org")
}

#[test]
fn dispatch_created_records_customer_and_driver_and_prefers_email() {
    let _db = TestDb::create();
    let org = org();
    let mut customer = Customer::create_customer(org.id, "Asha Rao", "9 Market Rd").expect("customer");
    customer.set_contact(Some("+91 90000 11111".into()), Some("asha@example.com".into())).expect("contact");
    let dispatch_id = Uuid::new_v4();

    let recorded = Notification::record_dispatch_created(
        org.id,
        dispatch_id,
        &customer,
        Some("+91 98888 22222"),
    )
    .expect("record");

    assert_eq!(recorded.len(), 2);
    let cust = recorded.iter().find(|n| n.recipient_kind == "customer").unwrap();
    assert_eq!(cust.channel, NotificationChannel::Email);
    assert_eq!(cust.recipient, "asha@example.com");
    assert_eq!(cust.status, NotificationStatus::Queued);
    assert!(cust.body.contains("Asha Rao"));
    assert!(cust.body.contains(&dispatch_id.to_string()[..8]));

    let driver = recorded.iter().find(|n| n.recipient_kind == "driver").unwrap();
    assert_eq!(driver.channel, NotificationChannel::Sms);
    assert_eq!(driver.recipient, "+91 98888 22222");

    // Persisted and readable back, oldest first.
    let listed = Notification::list_by_dispatch(dispatch_id).expect("list");
    assert_eq!(listed.len(), 2);
    assert!(listed.iter().all(|n| n.event == NotificationEvent::DispatchCreated));
}

#[test]
fn falls_back_to_sms_then_skips_when_no_contact() {
    let _db = TestDb::create();
    let org = org();

    // Phone only -> SMS.
    let mut phone_only = Customer::create_customer(org.id, "Phone Only", "1 Rd").expect("c1");
    phone_only.set_contact(Some("+91 70000 00000".into()), None).expect("contact");
    let n = Notification::record_dispatch_created(org.id, Uuid::new_v4(), &phone_only, None).expect("rec");
    let cust = n.iter().find(|n| n.recipient_kind == "customer").unwrap();
    assert_eq!(cust.channel, NotificationChannel::Sms);
    assert_eq!(cust.recipient, "+91 70000 00000");

    // Driver phone missing -> that row is SKIPPED.
    let driver = n.iter().find(|n| n.recipient_kind == "driver").unwrap();
    assert_eq!(driver.status, NotificationStatus::Skipped);

    // No contact at all -> customer row SKIPPED.
    let no_contact = Customer::create_customer(org.id, "No Contact", "2 Rd").expect("c2");
    let n2 = Notification::record_dispatch_created(org.id, Uuid::new_v4(), &no_contact, None).expect("rec2");
    assert!(n2.iter().all(|n| n.status == NotificationStatus::Skipped));
}

#[test]
fn dispatch_delivered_notifies_only_the_customer() {
    let _db = TestDb::create();
    let org = org();
    let mut customer = Customer::create_customer(org.id, "Beed Traders", "3 Rd").expect("customer");
    customer.set_contact(None, Some("beed@example.com".into())).expect("contact");
    let dispatch_id = Uuid::new_v4();

    let recorded = Notification::record_dispatch_delivered(org.id, dispatch_id, &customer).expect("rec");
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].recipient_kind, "customer");
    assert_eq!(recorded[0].event, NotificationEvent::DispatchDelivered);
    assert!(recorded[0].body.contains("delivered"));
}

#[test]
fn list_by_org_is_scoped_and_newest_first() {
    let _db = TestDb::create();
    let a = Organization::create_organization("Org A", "1").expect("a");
    let b = Organization::create_organization("Org B", "2").expect("b");
    let mut ca = Customer::create_customer(a.id, "CA", "1").expect("ca");
    ca.set_contact(None, Some("ca@example.com".into())).expect("contact");
    let cb = Customer::create_customer(b.id, "CB", "2").expect("cb");

    Notification::record_dispatch_created(a.id, Uuid::new_v4(), &ca, None).expect("a1");
    Notification::record_dispatch_delivered(a.id, Uuid::new_v4(), &ca).expect("a2");
    Notification::record_dispatch_created(b.id, Uuid::new_v4(), &cb, None).expect("b1");

    let a_list = Notification::list_by_org(a.id, 100).expect("list a");
    assert_eq!(a_list.len(), 3); // 2 created (cust+driver) + 1 delivered
    assert!(a_list.iter().all(|n| n.org_id == a.id));
    // DESC by created_at: the delivered event (recorded last) is at or near the top.
    assert!(a_list.iter().take(1).any(|n| n.event == NotificationEvent::DispatchDelivered));

    assert_eq!(Notification::list_by_org(b.id, 100).expect("list b").len(), 2);
}

#[test]
fn deleting_the_org_cascades_to_its_notifications() {
    let _db = TestDb::create();
    let org = org();
    let c = Customer::create_customer(org.id, "Gone", "0").expect("c");
    let d = Uuid::new_v4();
    Notification::record_dispatch_created(org.id, d, &c, None).expect("rec");

    org.remove_organization().expect("remove org");
    assert!(Notification::list_by_org(org.id, 100).expect("list").is_empty());
}

#[test]
fn recording_a_notification_indexes_it_for_the_assistant() {
    use crate::logistics::ai::chunk;

    let _db = TestDb::create();
    let org = org();
    let mut customer = Customer::create_customer(org.id, "Priya Sharma", "5 Market Rd").expect("customer");
    customer
        .set_contact(None, Some("priya@example.com".into()))
        .expect("contact");
    let dispatch_id = Uuid::new_v4();

    Notification::record_dispatch_created(org.id, dispatch_id, &customer, None).expect("record");

    let results = chunk::search_by_org(org.id, "Priya Sharma dispatched", 8).expect("search");
    assert!(
        results.iter().any(|c| c.text.contains("Priya Sharma")),
        "the recorded notification should be indexed and findable: {results:?}"
    );
}
