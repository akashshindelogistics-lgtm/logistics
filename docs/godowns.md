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

- `Godown { id, org_id, name, address, max_capacity?, location? }` —
  `location` is the same optional lat/long/timestamp/address shape used by
  orgs, vehicles and customers, updated through
  `PUT /api/godowns/{id}/location`. `max_capacity` is an optional cap on the
  godown's total stored volume (Σ `volume_in_size × quantity`); adding or
  updating stock that would push the total over it is rejected with `409`.
  Set it on create or via `PUT /api/godowns/{id}`.
- `Stock` rows now carry `godown_id` instead of `org_id`. A stock item is
  identified within a godown by its `description` (unchanged semantics, just
  scoped to a godown rather than an org). An optional `reorder_threshold`
  sets a restock point: reads expose a derived `below_threshold` flag that
  is `true` once `quantity` drops under it.
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
| POST | `/api/godowns/{gid}/transfer` | Move a stock item to another godown of the same org |
| GET | `/api/orgs/{id}/stock-transfers` | Transfer history for the org, newest first |

Every `/api/godowns/{gid}...` route loads the godown, checks
`godown.org_id == authenticated org`, and returns `403` otherwise. The old
`/api/orgs/{id}/stock*` routes are **removed**.

## Godown-to-godown transfers

Tracked by the `todo.org` item *"Support stock transfer between two godowns of
the same org (distinct from dispatch-to-customer), with an audit trail of the
move."*

`POST /api/godowns/{gid}/transfer` (where `{gid}` is the **source** godown)
takes `{ to_godown_id, description, quantity }` and, in
`StockTransfer::execute`:

1. rejects a same-godown, cross-org, or non-positive-quantity request (`400`),
2. checks the source godown actually holds `quantity` units of `description`
   (`400` if not),
3. checks the destination has room under its `max_capacity` (`409` if not),
4. inside one MySQL transaction: draws the units down from the source (deleting
   the row if it hits zero), adds them to the destination (a new stock row, or
   a bump to the existing one — the destination keeps its own `volume_in_size`
   if the item is already there, otherwise it arrives at the source's), and
   inserts one **insert-only** `StockTransfers` audit row
   (`id, org_id, from_godown_id, to_godown_id, description, quantity,
   volume_in_size, transferred_at`).

A transfer conserves the org's total stock; a dispatch does not. The
`StockTransfers` table is `ON DELETE CASCADE` from `Orgs` and is never updated
or deleted otherwise.

## Dispatch

`POST /api/orgs/{id}/dispatch` takes `{ customer_id, line_items: [{
stock_description, requested_quantity }] }` (see docs/dispatch-lifecycle.md).
For each line it:

1. Sums `quantity` for that `stock_description` across **all** the org's
   godowns and fails the whole request if any line's total is short.
2. Picks the nearest vehicle to the customer (unchanged Haversine logic),
   checking capacity against the summed volume of every line.
3. Decrements each line's quantity from the org's godowns, largest holding
   first, until satisfied.
4. Writes one `DispatchOrder` with a `DispatchLineItems` row per line (still
   keyed by `org_id` — dispatch history is not godown-scoped).

## Schema migration

There is no migration framework; `test_support::migrate()` is the source of
truth and every table is `CREATE TABLE IF NOT EXISTS`. Because `Stock` changed
its foreign key (`org_id` → `godown_id`), `migrate()` also runs a one-time
transitional step: if it finds a legacy `Stock.org_id` column it drops and
recreates the table. There is no hosted backend and dev/test stock is
disposable, so this is safe; a developer with local stock they care about
should re-seed it after upgrading.
