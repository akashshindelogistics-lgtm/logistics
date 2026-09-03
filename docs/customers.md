# Customers

Tracked by the `todo.org` item *"Customer should be linked to particular ORG
and should be not shared."*

## Model

A **customer** (a delivery recipient) belongs to exactly one organization.
Customers used to be a single flat, org-less table shared across every org;
they are now owned the same way drivers and godowns are.

```
Orgs ──1:N──▶ Customers
             (name, address, optional location)
```

- `Customer { id, org_id, name, address, location? }` — `org_id` is
  `NOT NULL` with `FOREIGN KEY (org_id) REFERENCES Orgs(id) ON DELETE CASCADE`,
  so deleting an org removes its customers. `location` is the same optional
  lat/long/timestamp/address shape used by orgs, vehicles and godowns.
- Only the owning org can list, locate, delete, or dispatch to a customer.

## API

| Method | Path | Description |
|---|---|---|
| GET | `/api/customers` | List customers for the authenticated org |
| POST | `/api/orgs/{id}/customers` | Create a customer under the org (`403` for a different org) |
| PUT | `/api/customers/{id}/location` | Set the customer's coordinates |
| DELETE | `/api/customers/{id}` | Delete the customer |

`PUT .../location` and `DELETE .../{id}` load the customer, check
`customer.org_id == authenticated org`, and return `403` (or `404` when the
id is unknown) otherwise — the `load_owned_customer` helper, mirroring
`load_owned_driver`. The old flat `POST /api/customers` route is **removed**;
create now goes through `/api/orgs/{id}/customers` to match the driver
convention.

## Dispatch

`POST /api/orgs/{id}/dispatch` takes `{ customer_id, line_items: [{
stock_description, requested_quantity }] }` (see docs/dispatch-lifecycle.md),
and rejects a `customer_id` that belongs to another org with `400 "Customer
belongs to a different organization"` — an org can only dispatch to its own
customers.

## Frontend

The Customers page lists only the logged-in org's customers, creates them
under that org (`getOrgId()` from the stored auth), and has a per-row delete
button. The dispatch form's customer dropdown is naturally scoped because it
is fed by the same org-scoped list.

## Schema migration

There is no migration framework; `test_support::migrate()` is the source of
truth and every table is `CREATE TABLE IF NOT EXISTS`. Because `Customers`
gained a `NOT NULL org_id` foreign key with no way to back-fill which org
owns a pre-existing row, `migrate()` (and `Customer::ensure_table`) run a
one-time transitional step: if the `Customers` table exists without an
`org_id` column it is dropped and recreated. There is no hosted backend and
dev/test customers are disposable, so this is safe — the same approach the
godown change took for `Stock`. A developer with local customers they care
about should re-create them after upgrading.
