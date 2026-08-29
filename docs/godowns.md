# Godowns (warehouses)

Tracked by the `todo.org` item *"Organisation can have multiple godowns where
they store their inventory."*

## Model

An organization owns zero or more **godowns** (warehouses). Stock is held **in a
godown**, not directly by the organization.

```
Orgs ──1:N──▶ Godowns ──1:N──▶ Stock
             (name, address,    (description, quantity,
              optional location) volume_in_size)
```

- `Godown { id, org_id, name, address, location? }` — `location` is the same
  optional lat/long/timestamp/address shape used by orgs, vehicles and
  customers, updated through `PUT /api/godowns/{id}/location`.
- `Stock` rows now carry `godown_id` instead of `org_id`. A stock item is
  identified within a godown by its `description` (unchanged semantics, just
  scoped to a godown rather than an org).
- `Organization` responses expose `godowns: Godown[]`, and each godown carries
  its own `stock: Stock[]`. The old flat `Organization.stock` field is gone.

## API

| Method | Path | Description |
|---|---|---|
| GET | `/api/orgs/{id}/godowns` | List the org's godowns (auth-scoped) |
| POST | `/api/orgs/{id}/godowns` | Create a godown under the org |
| GET | `/api/godowns/{gid}` | Get one godown with its stock |
| PUT | `/api/godowns/{gid}` | Rename / re-address a godown |
| DELETE | `/api/godowns/{gid}` | Delete a godown (cascades to its stock) |
| PUT | `/api/godowns/{gid}/location` | Set the godown's coordinates |
| POST | `/api/godowns/{gid}/stock` | Add a stock item to the godown |
| PUT | `/api/godowns/{gid}/stock` | Update a stock item's quantity/volume |
| DELETE | `/api/godowns/{gid}/stock/{desc}` | Remove a stock item |

Every `/api/godowns/{gid}...` route loads the godown, checks
`godown.org_id == authenticated org`, and returns `403` otherwise. The old
`/api/orgs/{id}/stock*` routes are **removed**.

## Dispatch

`POST /api/orgs/{id}/dispatch` still takes `{ customer_id, stock_description,
requested_quantity }`. It now:

1. Sums `quantity` for that `stock_description` across **all** the org's
   godowns and fails if the total is short.
2. Picks the nearest vehicle to the customer (unchanged Haversine logic).
3. Decrements the requested quantity from the org's godowns, largest holding
   first, until satisfied.
4. Writes one `DispatchOrder` (still keyed by `org_id` — dispatch history is
   not godown-scoped).

## Schema migration

There is no migration framework; `test_support::migrate()` is the source of
truth and every table is `CREATE TABLE IF NOT EXISTS`. Because `Stock` changed
its foreign key (`org_id` → `godown_id`), `migrate()` also runs a one-time
transitional step: if it finds a legacy `Stock.org_id` column it drops and
recreates the table. There is no hosted backend and dev/test stock is
disposable, so this is safe; a developer with local stock they care about
should re-seed it after upgrading.
