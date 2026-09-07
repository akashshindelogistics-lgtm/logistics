# Automatic GPS location tracking

Until now a vehicle's position could only be set with `PUT
/api/vehicles/{reg}/location`, which needs the organisation's bearer token. A
real fleet fits each truck with a GPS tracker that reports on its own, with no
human and no org login. This adds that push path.

## The tracker key

Every vehicle carries a `tracker_key` — a random UUID generated when the
vehicle is registered (`Vehicle::new`). It is the whole credential a device
needs: knowing a vehicle's key lets you report that vehicle's position and
nothing else. It is returned on every `Vehicle` response so the org can read
it off the dashboard.

- **Schema:** `Vehicle.tracker_key VARCHAR(36)`. Fresh databases get the
  column in their `CREATE TABLE`; a pre-existing local database is patched by
  `ensure_tracker_key_column` (probe `information_schema`, `ALTER TABLE ADD
  COLUMN`, backfill every row with `UUID()`), called on the vehicle read and
  write paths the same way `Customer::ensure_table` is. There is no hosted
  backend, so no formal migration.
- `Vehicle::by_tracker_key(uuid)` resolves a key to its vehicle (or `None`).
- `Vehicle::rotate_tracker_key()` issues a new key and returns it.
- `Vehicle::update_vehicle` reads the stored key back onto `self` after the
  update, because callers build a throwaway `Vehicle` (with a fresh generated
  key) before editing capacity/unit.

## Routes

| Method & path | Auth | Purpose |
| --- | --- | --- |
| `POST /api/track/{tracker_key}` | none | Device push. Body `{ latitude, longitude }`. Looks the vehicle up by key, validates the coordinates are on Earth (−90..90 / −180..180 → `400`), stamps the time server-side, updates the location. `404` if no vehicle has the key. Returns the new `Location`. |
| `POST /api/vehicles/{reg}/tracker-key/rotate` | org bearer, owned vehicle | Replace the key (lost or leaked device). `403` for another org's vehicle, `404` if unknown. Returns the updated `Vehicle`. |

`POST /api/track/{tracker_key}` is deliberately the only unauthenticated
write in the API. The key is high-entropy and single-purpose; rotating it is
the remedy if one leaks.

## Frontend

The vehicle detail page (`/vehicles/:reg`) gains a **GPS Tracker** panel: a
read-only "Tracker push URL" field showing `/api/track/<key>`, a short
explainer, and a **Regenerate key** button (confirm dialog → `rotateTrackerKey`
→ the field updates in place). The panel is hidden if the vehicle has no key
(only possible in old test fixtures — `tracker_key` is optional on the
frontend `Vehicle` type for that reason, always present from the server).

## Tests

- Rust model (`vehicle.rs`): key generated and round-trips through every read
  path; `by_tracker_key` miss returns `None`; rotate replaces the old key;
  `update_vehicle` keeps the real key on the returned struct.
- Rust routes (`routes.rs`): track push works with no auth header and shows on
  the fleet list; unknown key `404`; out-of-range coordinates `400`; rotate
  issues a new key and kills the old one; rotate from another org `403`.
- Frontend unit: `rotateTrackerKey` client; detail page shows/hides the panel
  and regenerates the key.
- Playwright (`vehicles.spec.ts`): device pushes location with only the key
  and it appears on the fleet list; regenerate invalidates the old key. The
  full-flow demo (`e2e-full-flow.spec.ts`) has a "GPS tracker reports the
  vehicle's live position" step.
