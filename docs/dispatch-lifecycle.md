# Dispatch lifecycle

Tracked by the `todo.org` item *"Model dispatch as a lifecycle ... instead of
writing a single 'DISPATCHED' status once at creation and never updating
it."*

## Model

A dispatch now moves through a small state machine instead of being written
once with a fixed status:

```
PENDING -> CONFIRMED -> LOADED -> IN_TRANSIT -> DELIVERED
   |           |          |            \-----> RETURNED
    \-----------\----------\-> CANCELLED
```

- `PENDING` — the order is recorded, stock is reserved and a vehicle is
  selected (this all still happens synchronously in
  `Organization::dispatch_stock_to_customer`), but nothing has physically
  moved. A vehicle is only eligible for selection when it has an **active
  assigned driver**, its `capacity` covers the shipment's volume
  (`volume_in_size × quantity`), and it is **not already on a non-terminal
  trip**. If nothing qualifies, dispatch is rejected before a `PENDING`
  order is created. A trip reaching `DELIVERED` / `RETURNED` / `CANCELLED`
  releases its vehicle back into the pool.
- `CONFIRMED` — ops has confirmed the order will be fulfilled as booked.
- `LOADED` — stock has been physically loaded onto the assigned vehicle.
- `IN_TRANSIT` — the vehicle has left for the delivery address.
- `DELIVERED` / `RETURNED` — terminal. `RETURNED` is reachable only from
  `IN_TRANSIT` (a delivery attempt that didn't land) and **credits the
  shipment's stock back into a godown** — see below. Reaching `DELIVERED`
  additionally requires proof of delivery — see below.
- `CANCELLED` — terminal, reachable from `PENDING` / `CONFIRMED` / `LOADED`
  but not once the vehicle is already out.

`DispatchStatus::can_transition_to` (`src/logistics/dispatch/dispatch.rs`)
is the single source of truth for which moves are legal; every other jump —
skipping a step, moving out of a terminal state — is rejected.

## Status history

Every status a dispatch has passed through is recorded in a new
`DispatchStatusHistory` table (`dispatch_id`, `status`, `changed_at`),
oldest first. `DispatchOrder.save()` writes the first entry (`PENDING` at
creation time); `DispatchOrder::transition_to` writes one on every
subsequent move. `DispatchOrder` responses now carry the full history as
`status_history: DispatchStatusEvent[]`.

## Proof of delivery

`DispatchOrder::transition_to` refuses to move a dispatch to `DELIVERED`
unless it's also given a `ProofOfDeliveryInput` (`receiver_name` +
`signature_or_photo_url`) — a rejected transition, not a silent no-op.
`RETURNED` has no such requirement, since nothing was handed over.

On success it's written once to a new `DispatchProofOfDelivery` table
(`dispatch_id` primary key — a dispatch can only reach `DELIVERED` once,
so this is insert-only, never updated), stamped with the same timestamp as
the `DELIVERED` status-history entry rather than a caller-supplied one.
`DispatchOrder` responses carry it as `proof_of_delivery:
ProofOfDelivery | null`, populated the same way as `status_history`.

There is no file-upload/storage system backing `signature_or_photo_url` —
it's a free-form URL or `data:` URI column; the caller is responsible for
hosting the actual image.

## Returns

Tracked by the `todo.org` item *"Support returns: a customer return should
credit stock back into a godown and record a RETURNED dispatch."*

Moving a dispatch `IN_TRANSIT → RETURNED` now adds every line item's
quantity back into the org's stock, alongside recording the status change:

- `DispatchOrder::transition_to`'s third argument, `return_to_godown_id:
  Option<Uuid>`, is the godown to credit. It must belong to this dispatch's
  org — a foreign godown is rejected and the status is left unchanged.
- When it's `None`, `resolve_return_godown` picks a godown that already
  holds one of the returned items (by name), and failing that the org's
  first godown by name. An org with no godown at all fails the return.
- `credit_line_items_back` bumps an existing `Stock` row's `quantity` for
  each line item, or inserts one (carrying the line item's snapshotted
  `volume_in_size`) if the chosen godown doesn't stock it yet — the same
  find-or-top-up shape `StockTransfer` uses for its destination.
- `PUT /api/dispatches/{id}/status` carries the optional
  `return_to_godown_id` on `UpdateDispatchStatusPayload`; the Dispatches
  page's "Mark Returned" action opens an inline godown picker (defaulting
  to "auto").

## API

| Method | Path | Description |
|---|---|---|
| PUT | `/api/dispatches/{id}/status` | Move a dispatch to the next status; `400` on an illegal transition or a `DELIVERED` request missing `proof_of_delivery` |

`UpdateDispatchStatusPayload` carries an optional `proof_of_delivery:
{ receiver_name, signature_or_photo_url }` — required when `status` is
`DELIVERED`, ignored otherwise — and an optional `return_to_godown_id`, used
only when `status` is `RETURNED` (see **Returns** above).

`POST /api/orgs/{id}/dispatch` (`dispatch_stock_to_customer`) allocates
stock and a vehicle in one step and returns a `DispatchOrder` that starts at
`PENDING`. `GET /api/dispatches` and
`GET /api/dispatches/{id}/summary` both return the current status plus its
full history and proof of delivery, since `DispatchOrder::get_by_id` /
`list_by_org` / `list_all` populate both alongside every other field.

## Multiple line items per dispatch

Tracked by the `todo.org` item *"Allow one dispatch to carry multiple stock
line items."* A real shipment usually mixes several SKUs, so a dispatch now
carries a list of line items instead of a single `stock_description` +
`quantity`.

- `DispatchOrder.line_items: Vec<DispatchLineItem>` where
  `DispatchLineItem { stock_description, quantity, volume_in_size }`.
  `volume_in_size` is snapshotted from the stock item at dispatch time.
  `DispatchOrder::total_quantity()` sums across the lines.
- Stored in a `DispatchLineItems` table (`ON DELETE CASCADE` from
  `Dispatches`), read back with the same N+1-per-parent pattern as
  `status_history` / `proof_of_delivery`. The old
  `Dispatches.stock_description` / `Dispatches.quantity` columns are gone;
  `drop_legacy_single_line_dispatch_tables` drops and recreates the dispatch
  tables when the old columns are detected (dev/test data is disposable — the
  same approach the godown and customer changes took).
- `POST /api/orgs/{id}/dispatch` takes
  `{ customer_id, line_items: [{ stock_description, requested_quantity }] }`.
  Validation is **all-or-nothing**: the request is rejected (and nothing is
  drawn down) if the list is empty, a quantity is `<= 0`, a description is
  repeated, or *any* line can't be satisfied from the org's godowns. The
  vehicle capacity check is against the summed volume of every line.
- The dashboard's Dispatch form has repeatable stock-line rows ("Add another
  line" / remove); the Dispatches and Dashboard tables list every line and
  show the combined quantity.

## Not done here

- **UI**: the frontend's `DispatchOrder` type and the Dispatches page still
  treat `status` as an opaque string (harmless — the field is still a
  string on the wire, just with different possible values now) and don't
  yet expose a way to advance a dispatch's status, view its history, or
  capture a proof of delivery.
- **Driver / vehicle-availability checks** before a dispatch can be
  created — separate `[#B]` todo items.
