# Driver phone location tracking (plan)

Status: **planned, not implemented.**

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

## Open questions

- Platform choice (above).
- Token in header vs URL path.
- Strict IN_TRANSIT gate vs accept-always.
- History retention default (suggested 30 days).
- Should a phone and a hardware tracker on the same vehicle be reconciled
  (latest timestamp wins, suggested) or should one source take precedence?
