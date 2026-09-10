use super::user::{OrgRole, OrgUser, UserError};
use crate::logistics::orgs::orgs::Organization;
use crate::logistics::test_support::TestDb;

fn make_org(name: &str) -> Organization {
    Organization::create_organization(name, "1 St").expect("create org")
}

#[test]
fn role_strings_round_trip_and_unknown_is_least_privilege() {
    for role in [OrgRole::Admin, OrgRole::Dispatcher, OrgRole::WarehouseStaff] {
        assert_eq!(OrgRole::from_str(role.as_str()), role);
    }
    assert_eq!(OrgRole::from_str("dispatch"), OrgRole::Dispatcher);
    assert_eq!(OrgRole::from_str("warehouse staff"), OrgRole::WarehouseStaff);
    assert_eq!(OrgRole::from_str("nonsense"), OrgRole::WarehouseStaff);
    assert_eq!(OrgRole::from_str(""), OrgRole::WarehouseStaff);
}

#[test]
fn create_then_authenticate_and_reload() {
    let _db = TestDb::create();
    let org = make_org("Team Org");

    let user = OrgUser::create(org.id, "Priya", "PRIYA@Example.com ", "hunter2pw", OrgRole::Dispatcher)
        .expect("create user");
    assert_eq!(user.email, "priya@example.com", "email is normalised");
    assert_eq!(user.role, OrgRole::Dispatcher);
    assert!(user.is_active);

    let ok = OrgUser::verify_login("priya@example.com", "hunter2pw").expect("verify").expect("some");
    assert_eq!(ok.id, user.id);
    assert!(OrgUser::verify_login("priya@example.com", "wrong").expect("verify").is_none());
    assert!(OrgUser::verify_login("nobody@example.com", "hunter2pw").expect("verify").is_none());

    let reloaded = OrgUser::get_by_id(user.id).expect("get").expect("some");
    assert_eq!(reloaded.name, "Priya");
    assert_eq!(reloaded.org_id, org.id);
}

#[test]
fn email_must_be_unique_across_orgs() {
    let _db = TestDb::create();
    let a = make_org("Org A");
    let b = make_org("Org B");
    OrgUser::create(a.id, "A", "dup@example.com", "password1", OrgRole::Admin).expect("first");
    let err = OrgUser::create(b.id, "B", "DUP@example.com", "password1", OrgRole::Admin).unwrap_err();
    assert!(matches!(err, UserError::EmailTaken));
}

#[test]
fn rejects_bad_email_and_short_password() {
    let _db = TestDb::create();
    let org = make_org("Validation Org");
    assert!(matches!(
        OrgUser::create(org.id, "X", "not-an-email", "password1", OrgRole::Admin).unwrap_err(),
        UserError::InvalidInput(_)
    ));
    assert!(matches!(
        OrgUser::create(org.id, "X", "x@example.com", "short", OrgRole::Admin).unwrap_err(),
        UserError::InvalidInput(_)
    ));
}

#[test]
fn inactive_user_cannot_log_in() {
    let _db = TestDb::create();
    let org = make_org("Deactivate Org");
    let mut user = OrgUser::create(org.id, "Sam", "sam@example.com", "password1", OrgRole::WarehouseStaff)
        .expect("create");

    user.update("Sam", OrgRole::WarehouseStaff, false).expect("deactivate");
    assert!(OrgUser::verify_login("sam@example.com", "password1").expect("verify").is_none());

    user.update("Sam", OrgRole::Dispatcher, true).expect("reactivate + promote");
    let back = OrgUser::verify_login("sam@example.com", "password1").expect("verify").expect("some");
    assert_eq!(back.role, OrgRole::Dispatcher);
}

#[test]
fn list_is_scoped_to_the_org_and_delete_removes_one() {
    let _db = TestDb::create();
    let a = make_org("Scoped A");
    let b = make_org("Scoped B");
    OrgUser::create(a.id, "A1", "a1@example.com", "password1", OrgRole::Admin).expect("a1");
    let a2 = OrgUser::create(a.id, "A2", "a2@example.com", "password1", OrgRole::Dispatcher).expect("a2");
    OrgUser::create(b.id, "B1", "b1@example.com", "password1", OrgRole::Admin).expect("b1");

    assert_eq!(OrgUser::list_by_org(a.id).expect("list a").len(), 2);
    assert_eq!(OrgUser::list_by_org(b.id).expect("list b").len(), 1);

    a2.delete().expect("delete a2");
    assert_eq!(OrgUser::list_by_org(a.id).expect("list a again").len(), 1);
}

#[test]
fn deleting_the_org_cascades_to_its_users() {
    let _db = TestDb::create();
    let org = make_org("Cascade Org");
    OrgUser::create(org.id, "Gone", "gone@example.com", "password1", OrgRole::Admin).expect("create");

    org.remove_organization().expect("remove org");
    assert!(OrgUser::list_by_org(org.id).expect("list").is_empty());
}
