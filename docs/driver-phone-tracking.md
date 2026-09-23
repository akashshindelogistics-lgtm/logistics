# Driver phone location tracking (plan)

Status: **Phases 1 (backend) and 4 (mobile app skeleton) implemented; phases 2-3
planned.** See [Phase 1 as built](#phase-1-as-built) and
[Phase 4 as built](#phase-4-as-built) for where the implementation settled the
open questions below. Phase 4 was built ahead of phases 2-3 (dashboard pairing
UI, location history) at the user's request; the device token is currently
paired by pasting it, since there is no dashboard panel yet to show it as a QR
code.

[gps-tracking.md](gps-tracking.md) added hardware GPS trackers that push a
vehicle's position to `POST /api/track/{tracker_key}`. This plan adds a second
source: a small Android/iOS app on the driver's phone that reports location
periodically, for fleets that don't fit hardware trackers.

## Why the existing endpoint is not enough

A phone could already call `/api/track/{tracker_key}` today, but that is a poor
fit:

- **The key belongs to a vehicle, not a driver.** Drivers change vehicles. A
  phone holding a vehicle key keeps reporting for a truck the driver has left.
- **Rotating a leaked or lost phone's key breaks the hardware tracker** on the
  same vehicle, since they share one key.
- **No timestamps.** `Vehicle::update_location` stamps server time. A phone
  that queues fixes offline and sends them later would make the map jump
  backwards or show stale positions as current.
- **No history.** Only the latest fix is stored.
- **`Driver` has no login.** Drivers are records (name, licence, phone,
  `is_active`) managed by org users; nothing authenticates as a driver yet.

## Design

### 1. Driver credentials

Add a per-driver device credential, separate from any vehicle's tracker key.

- `Drivers.device_token VARCHAR(36) NULL` (UUID, same pattern and the same
  `ensure_*_column` patch-on-read approach as `tracker_key`).
- `POST /api/drivers/{id}/device-token/rotate` (org bearer, same-org driver)
  issues or replaces the token. The dashboard shows it as a pairing string or
  QR code the driver scans once.
- A lost phone is fixed by rotating that driver's token only. Vehicles and
  hardware trackers are unaffected.

Decision to confirm: token in the URL path (matches `/api/track/{key}`) vs an
`Authorization: Bearer` header. Recommendation: header, because URLs end up in
proxy and access logs.

### 2. Reporting endpoint

`POST /api/driver/location`, authenticated by the device token.

```json
{ "fixes": [
  { "latitude": 18.52, "longitude": 73.85,
    "recorded_at": "2026-09-21T10:15:00Z",
    "accuracy_m": 12.0, "speed_mps": 8.3 }
] }
```

- The server resolves driver -> vehicle via `Vehicle.assigned_driver_id`. `409`
  if the driver has no assigned vehicle; `403` if the driver is inactive.
- Batch of at most ~100 fixes. Each fix is validated as on Earth (like
  `/api/track`), and `recorded_at` may not be in the future (small clock-skew
  allowance).
- The vehicle's current location is only advanced if the newest fix is newer
  than `last_updated_at`, so late-arriving batches never move the map
  backwards.
- Optional gate: only accept while the vehicle has a dispatch in `IN_TRANSIT`.
  This limits tracking to duty time (privacy) and saves battery. To decide:
  strict gate vs accept-always with the app deciding when to send.

### 3. Location history (optional, second phase)

`VehicleLocationHistory (id, registration_number, latitude, longitude,
recorded_at, accuracy_m, speed_mps, source ENUM('DEVICE','DRIVER_APP','MANUAL'))`
with an index on `(registration_number, recorded_at)`.

- Also written by `/api/track` and `PUT .../location`, so history is complete
  regardless of source.
- Retention: prune rows older than N days (config), so the table cannot grow
  without bound.
- `GET /api/vehicles/{reg}/location-history?from=&to=` feeds a trail and
  playback on the trip map.

### 4. The mobile app

Decision to confirm: platform. Options:

| Option | Background reliability | Notes |
| --- | --- | --- |
| React Native (recommended) | Good, via a background-geolocation library | Same TypeScript skills as the dashboard |
| Flutter | Good | New language/toolchain for the team |
| PWA in the existing frontend | Poor: browsers stop geolocation when backgrounded or locked, worst on iOS | Fine as a fallback while the page is open |
| Off-the-shelf (Traccar Client, OwnTracks) | Good | Needs an adapter for their payload formats and gives no control of UX |

Minimum app scope: pair with the token, show assigned vehicle and shift state,
start/stop reporting, background reporting (Android foreground service with a
persistent notification; iOS "Always" permission), an offline queue that
flushes in batches, and a visible "location is being shared" indicator.

## Privacy and safety

- Report only while on duty, and always show the driver that sharing is active.
- Get explicit consent at pairing and document what is collected and how long
  history is kept.
- The device token is a secret: HTTPS only, never logged, rotatable.

## Phases

1. **Backend:** driver `device_token` + rotate route, `POST /api/driver/location`
   with batching and out-of-order handling, unit tests, OpenAPI annotations,
   README route table and `docs/gps-tracking.md` cross-link.
2. **Dashboard:** driver pairing panel (token/QR, regenerate) on the org
   detail Drivers section; vehicle detail shows the source of the last fix.
   Frontend unit tests plus a Playwright spec and a standalone headed demo
   flow.
3. **History:** `VehicleLocationHistory`, retention, history endpoint, trail
   on the trip map.
4. **Mobile app:** minimal React Native driver app in a separate directory,
   with its own test and release notes.

## Phase 4 as built

`mobile/` — an [Expo](https://expo.dev) app (SDK 57, TypeScript, Expo Router).
See [mobile/README.md](../mobile/README.md) for the full layout, setup and
known gaps; the summary here is what it settles from the plan.

- **Pairing:** paste the device token (from
  `POST /api/drivers/{id}/device-token/rotate`) and the server address; stored
  via `expo-secure-store`, not plain storage. QR pairing is deferred to phase 2
  (there is nothing on the dashboard to render a QR code from yet).
- **Reporting:** `expo-location` + `expo-task-manager` run a background task
  that keeps posting while the app is backgrounded or the phone is locked, at
  a 30s / 50m interval. Fixes are validated client-side with the same rules as
  `validate_driver_fix` (`mobile/src/lib/validation.ts`), so a bad fix — an
  out-of-range coordinate, a clock far in the future — is dropped before it
  ever reaches the batch-fails-atomically behaviour on the server.
- **Offline queue:** fixes persist to `AsyncStorage` and are uploaded in
  batches of up to 100 (mirroring `MAX_DRIVER_FIXES_PER_REQUEST`). A batch the
  server rejects for an account reason (401/403/409) is left queued rather
  than dropped, since retrying costs nothing once a dispatcher fixes the
  underlying problem.
- **Status screen:** sharing on/off toggle, queued-fix count, last sync
  outcome, and the vehicle last reported to (learned from the upload response,
  since there is no separate "who am I" read endpoint — see the README's
  known gaps).
- **Tests:** 53 Jest tests covering the pure queue/validation/API logic, the
  storage wrappers, and one screen (pairing) with
  `@testing-library/react-native`; `tsc --noEmit` and `eslint .` both clean.
  Not yet exercised on a real device or simulator — this environment has no
  Android SDK and no macOS.

## Open questions

- Platform choice (above).
- Token in header vs URL path.
- Strict IN_TRANSIT gate vs accept-always.
- History retention default (suggested 30 days).
- Should a phone and a hardware tracker on the same vehicle be reconciled
  (latest timestamp wins, suggested) or should one source take precedence?

## Phase 1 as built

The backend is done. It follows the design above except where noted; the
"open questions" it settled are called out.

**Routes**

| Method & path | Auth | Purpose |
| --- | --- | --- |
| `POST /api/drivers/{id}/device-token/rotate` | org bearer, Admin or Dispatcher, driver in the caller's org | Issue or replace the driver's device token. The plain token is in this response only. `403` for another org's driver or the wrong role, `404` if unknown. |
| `POST /api/driver/location` | `Authorization: Bearer <device token>` | Report a batch of fixes. See below. |

**`POST /api/driver/location`**

- Body: `{ "fixes": [{ "latitude", "longitude", "recorded_at", "accuracy_m"?, "speed_mps"? }] }`.
- `recorded_at` is **unix seconds** (an `i64`), matching `Location.timestamp`
  and avoiding a date-parsing dependency. The plan sketched ISO 8601.
- The driver is resolved from the token, then the vehicle from
  `Vehicle.assigned_driver_id` (`Vehicle::by_assigned_driver`). If a driver is
  assigned to several vehicles the alphabetically first registration is used.
- Only the **newest** fix in the batch is applied; the rest could not survive
  being overwritten. `Vehicle::record_fix` runs one conditional `UPDATE ...
  WHERE last_updated_at IS NULL OR last_updated_at < :recorded_at`, so the
  location only ever moves forward and a late or repeated upload is a no-op
  (`200` with `location_updated: false`). An applied fix clears
  `location_address`, since the old address described the old position.
- Validation, all `400`: batch empty or over 100 fixes; coordinates off Earth;
  `recorded_at` not positive or more than 300 s ahead of the server clock;
  negative or non-finite `accuracy_m` / `speed_mps`.
- The **whole batch is rejected if any fix is invalid** and nothing is applied,
  so the app never has to guess which fixes landed.
- Errors: `401` missing, malformed or unknown token (an org JWT is not a
  device token); `403` driver inactive; `409` driver has no assigned vehicle.
- `accuracy_m` and `speed_mps` are validated but **not stored yet**; they are
  for the history table in phase 3.

**Decisions taken**

- **Token in the `Authorization` header**, not the URL path, so it stays out of
  access logs.
- **No `IN_TRANSIT` gate.** The endpoint accepts fixes whenever the driver is
  active and has an assigned vehicle. Limiting reporting to duty time is the
  app's job (it only starts reporting on shift); a server-side gate can be
  added later without changing the contract.
- **The token is stored hashed.** `Drivers.device_token_hash` holds the SHA-256
  of the token (`CHAR(64)`, unique). The plain token is never stored, so a
  database dump cannot be replayed, and it is not part of the `Driver` struct,
  so no driver list or detail response can ever contain it. The dashboard must
  therefore show it once, straight from the rotate response (phase 2). Old
  databases get the column from `Driver::ensure_device_token_column`.
- Phone and hardware tracker on the same vehicle: **latest timestamp wins**.
  Both write `last_updated_at`; the phone path refuses anything not newer. The
  hardware path (`/api/track`) still stamps server time.

**Tests**

- Model (`driver.rs`, `vehicle.rs`): token resolves to its driver; rotating kills
  the old token; an unknown token is `None`; the token is stored as a 64-char
  hash, not in plain text; tokens are per driver; `by_assigned_driver`;
  `record_fix` stores capture time, ignores older and equal-time fixes, and
  clears a stale address.
- Routes (`routes.rs`): a phone moves its assigned vehicle with only the token;
  a batch applies only the newest fix; a late upload cannot move the vehicle
  backwards; missing, unknown, malformed and org-JWT tokens are `401`; rotation
  kills the old token; inactive driver `403`; no vehicle `409`; each
  validation rule `400`; one bad fix rejects the whole batch; a driver never
  moves another org's vehicle; rotate from another org `403`, unknown driver
  `404`, wrong role `403`; driver responses never contain the token.
