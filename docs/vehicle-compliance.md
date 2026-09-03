# Vehicle compliance paperwork

Tracked by the `todo.org` item *"Track vehicle compliance paperwork
(insurance, RC, permit, PUC/fitness certificate) with expiry dates and
renewal reminders — mandatory for any commercial truck on Indian roads."*

## Model

A **vehicle document** records one piece of statutory paperwork for one
vehicle, with the expiry date that matters for keeping the truck road-legal.

```
Orgs ──1:N──▶ Vehicle ──1:N──▶ VehicleDocuments
                               (doc_type, document_number, issued_on?, expires_on, notes?)
```

- `VehicleDocument { id, org_id, vehicle_registration, doc_type,
  document_number, issued_on?, expires_on, notes?, days_until_expiry,
  status }` — `src/logistics/vehicle/document.rs`.
- `doc_type` is one of `Insurance`, `RegistrationCertificate`, `Permit`,
  `PollutionCertificate`, `FitnessCertificate`. Parsing is case-insensitive
  and accepts the shorthands `RC`, `PUC`, `FC`; an unrecognised value falls
  back to `Insurance` rather than failing the request (mirrors
  `Unit::from_str`).
- Dates are stored as ISO `YYYY-MM-DD` strings. `issued_on` is optional;
  `expires_on` is required. Both are validated as real calendar dates on
  write (`2026-02-30` is rejected with `400`).
- `days_until_expiry` and `status` are **computed on every read** from the
  current date, never persisted:
  - `Expired` — `expires_on` is in the past.
  - `ExpiringSoon` — expires within `EXPIRY_WARNING_DAYS` (30) days.
  - `Valid` — more than 30 days of validity left.
  The date maths uses a self-contained `days_from_civil` helper, so the
  crate takes no date-library dependency.
- The `VehicleDocuments` table has `ON DELETE CASCADE` foreign keys to both
  `Orgs(id)` and `Vehicle(registration_number)`, so removing a vehicle (or
  its org) removes its paperwork.

## API

| Method | Path | Description |
|---|---|---|
| GET | `/api/vehicles/{reg}/documents` | Documents for one vehicle, soonest expiry first |
| POST | `/api/vehicles/{reg}/documents` | Record a document (`400` on a bad date, `404` for a vehicle in another org) |
| PUT | `/api/vehicle-documents/{id}` | Update / renew a document (`403` for another org's document) |
| DELETE | `/api/vehicle-documents/{id}` | Delete a document |
| GET | `/api/orgs/{id}/vehicle-documents` | Whole-fleet compliance list, soonest expiry first |

Every route checks ownership before acting: the per-vehicle routes verify
the vehicle belongs to the authenticated org (`ensure_owned_vehicle`), and
the by-id routes load the document and check its stored `org_id`
(`load_owned_vehicle_document`, mirroring `load_owned_driver` /
`load_owned_godown`). "Renewal" is just a `PUT` that pushes `expires_on`
forward.

## Dashboard

The org detail page has a **Vehicle Compliance** section:

- a summary in the header — total count, plus `N expiring soon` / `M expired`
  badges when either is non-zero (the "renewal reminder");
- a table of every document across the fleet with a colour-coded status
  badge and a "Renew" / delete action per row;
- an inline "Add Compliance Document" form (vehicle, document type, number,
  expiry date).

The whole-fleet list is loaded alongside the org, customers, drivers and
stock-transfer history in `OrganizationDetail`'s `load()`.
