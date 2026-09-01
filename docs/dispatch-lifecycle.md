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
  `IN_TRANSIT` (a delivery attempt that didn't land). Reaching `DELIVERED`
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

## API

| Method | Path | Description |
|---|---|---|
| PUT | `/api/dispatches/{id}/status` | Move a dispatch to the next status; `400` on an illegal transition or a `DELIVERED` request missing `proof_of_delivery` |

`UpdateDispatchStatusPayload` carries an optional `proof_of_delivery:
{ receiver_name, signature_or_photo_url }` — required when `status` is
`DELIVERED`, ignored otherwise.

`POST /api/orgs/{id}/dispatch` (`dispatch_stock_to_customer`) is unchanged
in what it does — it still allocates stock and a vehicle in one step — but
the `DispatchOrder` it returns now starts at `PENDING` instead of the old
fixed `"DISPATCHED"` string. `GET /api/dispatches` and
`GET /api/dispatches/{id}/summary` both return the current status plus its
full history and proof of delivery, since `DispatchOrder::get_by_id` /
`list_by_org` / `list_all` populate both alongside every other field.

## Not done here

- **UI**: the frontend's `DispatchOrder` type and the Dispatches page still
  treat `status` as an opaque string (harmless — the field is still a
  string on the wire, just with different possible values now) and don't
  yet expose a way to advance a dispatch's status, view its history, or
  capture a proof of delivery.
- **Driver / vehicle-availability checks** before a dispatch can be
  created — separate `[#B]` todo items.
