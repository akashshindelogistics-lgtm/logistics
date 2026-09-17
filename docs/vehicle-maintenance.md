# Preventive maintenance scheduling

Tracked by the `todo.org` item *"Preventive maintenance scheduling - service
due by mileage/date, with alerts, for vehicles."*

## Model

Mirrors the compliance-expiry pattern in `vehicle::document`
(`docs/vehicle-compliance.md`, the compliance-paperwork tracking that
already ships) almost exactly: a record with a due criterion, a status
computed fresh on every read rather than stored, and a warning window before
it's actually overdue.

- `VehicleMaintenance { id, org_id, vehicle_registration, description,
  due_on, due_at_mileage_km, current_mileage_km, last_service_on, notes,
  days_until_due, km_until_due, status }`
  (`src/logistics/vehicle/maintenance.rs`). `description` is free text (e.g.
  "Oil change", "Brake pad replacement") rather than a closed enum like
  compliance doc types — maintenance items aren't a small fixed legal set.
- At least one of `due_on` (ISO `YYYY-MM-DD`) / `due_at_mileage_km` is
  required; a record with neither has nothing to be "due" about.
- `MaintenanceStatus::{UpToDate, DueSoon, Overdue}` — `Overdue` if a due date
  has passed OR the latest recorded mileage has reached the due mileage;
  `DueSoon` if within `DUE_WARNING_DAYS` (14) or `DUE_WARNING_KM` (500) of
  either trigger, without yet being overdue.
- **The one real difference from compliance documents:** there is no ambient
  "today" equivalent for mileage the way there is for dates. This app has no
  odometer telemetry (no GPS-derived distance tracking, no per-trip mileage
  accumulation), so `current_mileage_km` is simply the latest reading ops
  entered on the record itself — updated via `record_mileage()`, the same
  way any other field would be — not a live, automatically-updated
  vehicle-wide value. A mileage-based item with no reading recorded yet has
  `km_until_due: null` and can never be `Overdue`/`DueSoon` by mileage alone
  until someone records one.

## API

| Method | Path | Description |
|---|---|---|
| GET | `/api/vehicles/{reg}/maintenance` | Maintenance items for one vehicle, soonest date-based due date first |
| POST | `/api/vehicles/{reg}/maintenance` | Schedule a new item — `{ description, due_on?, due_at_mileage_km?, last_service_on?, notes? }`. `400` if neither due field is given or a date is invalid |
| PUT | `/api/vehicle-maintenance/{id}` | Update the schedule (description/due date/due mileage/last-serviced/notes) |
| PUT | `/api/vehicle-maintenance/{id}/mileage` | Record the vehicle's latest odometer reading — `{ current_mileage_km }` |
| DELETE | `/api/vehicle-maintenance/{id}` | Delete the item |
| GET | `/api/orgs/{id}/vehicle-maintenance` | Every item across the org's fleet — the list a "service due" view is built from |

No role restriction — any authenticated org member can manage maintenance
schedules, matching `vehicle::document`'s existing precedent (compliance
paperwork has no role gate either).

## Frontend

The org detail page's **Vehicle Maintenance** card (mirrors the existing
**Vehicle Compliance** card immediately above it): a table of items with
their vehicle, description, due date/mileage and a colour-coded status
badge, header badges counting how many are due soon / overdue, and an
inline "Schedule maintenance" form (vehicle, item, due date, due-at-km — at
least one of the last two required). Row actions: "Record mileage" (only
shown for a mileage-tracked item; prompts for the new reading), "Renew"
(prompts for a new due date and/or due mileage, whichever the item has, and
stamps `last_service_on` as today), and delete.

## Alerts

"With alerts" here means the same thing it already means for compliance
paperwork: a **dashboard flag**, not a push notification. The org detail
page's header shows "N due soon" / "N overdue" badges the moment a fetch
returns a due/overdue item — there's no separate background job or
SMS/email channel for this, unlike the dispatch delay-alert feature
(`docs/delay-alerts.md`), which really does push a notification because
that's a customer-facing event on a live shipment. A vehicle's next service
being due is an internal fleet-ops concern, checked when someone opens the
org page — the same reasoning `vehicle::document`'s renewal reminders
already settled on.

## Known limitations

- No automatic mileage tracking. See "Model" above; adding one (from GPS
  distance accumulation, say) is a materially bigger feature than this item
  asked for and would need its own design.
- One fixed warning window (14 days / 500km) for every item, not
  configurable per maintenance type — matches `EXPIRY_WARNING_DAYS`'s
  existing fleet-wide-constant precedent in `vehicle::document`.

## Tests

- Rust unit (`maintenance.rs`): rejects a record with neither due
  criterion; rejects an invalid date; status tracks the date-based window;
  status tracks the mileage-based window once a reading is recorded (and
  stays `UpToDate` with no reading yet); either trigger alone can make a
  record overdue; update recomputes status and rejects dropping both due
  criteria; delete; cascade-delete with the vehicle; org/vehicle listing
  order (dated items before mileage-only ones).
- Rust route (`routes.rs`): full CRUD lifecycle through the API including
  recording an odometer reading; `400` for no due criterion; `404`/`403` for
  cross-org access on create/update/list.
- Frontend unit (`vehicles.test.ts`): the six new API client functions.
  (`OrganizationDetail.test.tsx`): scheduling an item through the form;
  due-soon/overdue counts and per-row status badges; recording an odometer
  reading through the prompt.
- Playwright (`organization.spec.ts`): schedules a date-based item (due
  soon) and a mileage-based one via the UI, records an odometer reading past
  the mileage due point, and confirms the item flips to overdue.
