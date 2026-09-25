# Hired vehicles from vendors

Today every dispatch has to go out on one of the org's **own** trucks:
`select_free_vehicle` (`src/logistics/orgs/orgs.rs`) only considers `Vehicle`
rows of the org with an active `Driver` of the org, and
`Dispatches.vehicle_registration_number` is `NOT NULL`. An org with no fleet
of its own (or whose fleet is fully booked) cannot dispatch at all.

In practice such an org phones a **vehicle vendor** (transporter / broker)
and books a "market truck" for the trip. This plan adds that path.

## Status

- **Phase 1 (vendors): done.** `src/logistics/vendor/vendor.rs` has the
  `VehicleVendor` model and `VehicleVendors` table. The four routes are in
  `src/logistics/server/routes.rs` (tag *Vehicle vendors*), and the
  `/vendors` page is in `frontend/src/pages/Vendors.tsx`. Deleting a vendor
  is unconditional for now: the "refused while it has an open hire" check
  arrives with `VehicleHires` in phase 2.
- Phases 2–4: not started.

## Decisions

- **One vendor per hire.** The dispatcher picks a single vendor; no
  multi-vendor quotes or bidding.
- **The dispatcher enters the details.** The vendor never logs in. After the
  phone call the dispatcher types in the truck number, driver and agreed rate.
- **No automatic hiring.** When no own truck is free, the dispatch still fails
  as today, but the error points at the hire option. Choosing to hire is always
  an explicit dispatcher action.

## Why a hire record, not a hired row in `Vehicle`

Adding `ownership = HIRED` to `Vehicle` looks cheaper but fits badly:

- `Vehicle.registration_number` is a **global** primary key. Market trucks
  change every trip and the same truck can work for several orgs over time,
  so hired numbers would collide.
- A hired truck comes with the **vendor's driver**, not one of the org's
  `Driver` rows, so the "active assigned driver" rule doesn't apply.
- Fleet utilisation and compliance renewal reminders are about the org's own
  trucks; hired trucks would skew both.

So a hired truck lives on a per-dispatch **`VehicleHire`** record and never
enters the `Vehicle` table.

## Data model

**`VehicleVendors`** (org-scoped, cascade-deleted with the org)

| column | notes |
| --- | --- |
| `id, org_id` | |
| `name` | required |
| `contact_person, phone` | phone required (the dispatcher calls it) |
| `gstin` | optional |
| `notes` | optional: rate terms, lanes served |
| `is_active` | inactive vendors are hidden from the hire picker |

**`VehicleHires`** (one per hired dispatch or trip)

| column | notes |
| --- | --- |
| `id, org_id, vendor_id` | vendor must belong to the org |
| `dispatch_id` / `trip_id` | exactly one is set |
| `registration_number` | the hired truck's number (free text, not an FK) |
| `capacity, unit` | as declared by the vendor; used for the capacity check |
| `driver_name, driver_phone, driver_license` | the vendor's driver |
| `freight_amount` | agreed hire cost, whole currency, > 0 |
| `advance_paid` | defaults 0, ≤ `freight_amount` |
| `status` | `REQUESTED → CONFIRMED → RELEASED`, or `CANCELLED` |
| `requested_at, confirmed_at, released_at` | server-stamped |

**`Dispatches`** changes

- `vehicle_source` `VARCHAR(10) NOT NULL DEFAULT 'OWN'`: `OWN` or `HIRED`.
- `hire_id` nullable.
- `vehicle_registration_number` becomes **nullable**. It is `NULL` only while
  the dispatch is `AWAITING_VEHICLE`. Once the hire is confirmed it holds the
  hired truck's number, so everything that displays a dispatch's truck keeps
  working.
- Older local databases are back-filled by an `ensure_vendor_columns` helper,
  the same way `ensure_trip_columns` handles trips.

**`DispatchStatus`** gains **`AWAITING_VEHICLE`** before `PENDING`:

```
AWAITING_VEHICLE -> PENDING -> CONFIRMED -> LOADED -> IN_TRANSIT -> DELIVERED / RETURNED
       \                \           \          \-> CANCELLED
        \-> CANCELLED
```

Only a hired dispatch ever starts in `AWAITING_VEHICLE`; `AWAITING_VEHICLE ->
PENDING` is **not** reachable through `PUT /api/dispatches/{id}/status`. It
happens only by assigning the hired vehicle (below).

## Flow

1. **Raise the hire.** `POST /api/orgs/{id}/dispatch` takes an optional
   `vehicle_source: "HIRED"` + `vendor_id`. Stock is validated and drawn down
   exactly as today (`plan_stock_draw` / `draw_down_plans`), so the goods are
   reserved while the dispatcher is on the phone. `select_free_vehicle` is
   skipped. The dispatch is written as `AWAITING_VEHICLE` with a `REQUESTED`
   hire.
2. **Assign the truck.** `PUT /api/vehicle-hires/{id}/assign` with
   `{registration_number, capacity, unit, driver_name, driver_phone,
   driver_license, freight_amount, advance_paid?}`:
   - rejects a capacity below the dispatch's summed line-item volume (same
     rule as own trucks);
   - sets the hire `CONFIRMED`, copies the registration number onto the
     dispatch, and moves the dispatch `AWAITING_VEHICLE -> PENDING` with a
     status-history entry;
   - fires the dispatch-created notification to the customer and to the hired
     driver's phone.
3. **Run the trip.** From `PENDING` on it is an ordinary dispatch: proof of
   delivery, returns and invoicing all work unchanged.
4. **Release.** When the dispatch reaches `DELIVERED`, `RETURNED` or
   `CANCELLED`, the hire moves to `RELEASED` (or `CANCELLED`).
5. **Cancel before assignment.** Cancelling an `AWAITING_VEHICLE` dispatch
   must **credit the drawn stock back** into its godowns. Cancellation does not
   restore stock today for any dispatch; this plan fixes it for hired dispatches
   and reuses the returns credit-back logic. Whether own-fleet cancellations
   should do the same is a separate question.

**Multi-stop trips** follow the same flow: `POST /api/orgs/{id}/trips` accepts
the same `vehicle_source` / `vendor_id`, one hire is raised for the trip
(`trip_id` set), and assigning it moves every stop to `PENDING` and sets
`Trips.vehicle_registration_number`, which becomes nullable too.

**Own-fleet selection** must ignore hired dispatches: the "vehicle is on an
active trip" subquery in `select_free_vehicle` gains
`AND disp.vehicle_source = 'OWN'`, so a hired truck that happens to share a
number never blocks an own truck.

## Routes

Admin | Dispatcher for writes; every role reads.

| Method & path | |
| --- | --- |
| `GET /api/orgs/{id}/vendors` | the org's vendors |
| `POST /api/orgs/{id}/vendors` | create |
| `PUT /api/vendors/{id}`, `DELETE /api/vendors/{id}` | edit / delete (delete refused while it has an open hire) |
| `GET /api/orgs/{id}/vehicle-hires` | all hires, newest first, filterable by `status` |
| `PUT /api/vehicle-hires/{id}/assign` | enter the truck + driver + rate (step 2) |
| `POST /api/vehicle-hires/{id}/payments` | record a payment to the vendor (phase 3) |

`POST /api/orgs/{id}/dispatch` and `POST /api/orgs/{id}/trips` gain the
optional `vehicle_source` / `vendor_id` fields; omitting them means `OWN`,
so existing callers keep working.

## Knock-on changes

| Area | Change |
| --- | --- |
| Readers of `vehicle_registration_number` | reports, AI chunk/digest/status, invoice, trip, Dashboard/Dispatches/Trips pages: handle `NULL` ("Awaiting vehicle") and show a `Hired · <vendor>` tag |
| Billing | a **vendor payable** per hire: `freight_amount`, advance, payments, balance. Indian practice is a large advance at loading and the balance against proof of delivery. Margin per dispatch = invoice amount − hire cost |
| Reports | fleet utilisation stays own-fleet only; add hired-trip count, spend per vendor, own vs hired share, and margin on hired trips |
| Compliance | optional upload of the hired truck's RC / insurance onto the hire via the existing uploads. Warn if missing, never block |
| GPS | no tracker on market trucks; manual location updates only for now |
| Delay alerts | apply unchanged once the dispatch is `PENDING`; also flag a dispatch sitting in `AWAITING_VEHICLE` too long |

## Frontend

- **Vendors page** (`/vendors`, sidebar under Fleet): list, add, edit,
  activate/deactivate.
- **Dispatch form / trip form**: a *Vehicle source* toggle, **Own fleet** (as
  today) or **Hire from vendor** with a vendor picker. The "no vehicle free"
  error offers a one-click switch to *Hire from vendor*.
- **Dispatches page**: an `Awaiting vehicle` status badge and an inline
  **Assign hired vehicle** form (truck no., capacity, driver, rate, advance).
  Hired rows show the vendor name.
- **Vendor payables** (phase 3): balance per hire with a *Record payment*
  action, and an outstanding total per vendor on the Vendors page.

## Phases

Each phase ships backend + frontend + Rust unit/route tests + frontend unit
tests + a Playwright flow + a standalone headed demo flow.

1. **Vendors**: model, table, CRUD routes, Vendors page.
2. **Hire-based dispatch**: `AWAITING_VEHICLE`, `vehicle_source` / `hire_id`,
   nullable vehicle column + migration, assign route, stock credit on cancel,
   own-fleet selection fix, form toggle + assign form, NULL-safe readers.
   Trips included.
3. **Vendor payables and reports**: payments, balance, margin, spend per
   vendor.
4. **Later / optional**: vendor rate cards to pre-fill `freight_amount`,
   hired-truck GPS via a per-hire tracker key, a delay alert for dispatches
   stuck awaiting a vehicle.
