# Logistics Driver (mobile)

An [Expo](https://expo.dev) / React Native app that reports a driver's phone
location to the logistics backend, as an alternative to fitting a vehicle
with a hardware GPS tracker. It is phase 4 of
[docs/driver-phone-tracking.md](../docs/driver-phone-tracking.md) — read that
first for the design and the backend contract this app talks to
(`POST /api/driver/location`, `POST /api/drivers/{id}/device-token/rotate`).

## What it does

1. A dispatcher regenerates a device token for a driver on the dashboard (the
   token is shown once) and gives it, along with the API server's address, to
   the driver.
2. The driver opens the app, pastes both into the **pairing** screen. The
   token is stored in the OS keychain/keystore via `expo-secure-store`, never
   in plain storage.
3. On the **status** screen, the driver switches "Sharing location" on at the
   start of a shift. This requests foreground and background location
   permission, then registers a background task
   (`expo-location` + `expo-task-manager`) that keeps reporting even while the
   phone is locked or the app is in the background.
4. Every fix is validated the same way the server would (`src/lib/validation.ts`
   mirrors `validate_driver_fix` in `src/logistics/server/routes.rs`) and
   queued locally. The queue is flushed after every location update, and can
   also be flushed on demand with the status screen's "Sync now" button — so
   a phone that loses signal keeps recording and catches up once it's back
   online, instead of losing fixes.
5. The status screen shows the last vehicle reported to, how many fixes are
   queued, and the outcome of the last sync (ok / stale / a specific error).

## Project layout

```
src/
├── app/                  # Expo Router screens — see AGENTS.md
│   ├── _layout.tsx       # Registers the background task, sets up navigation
│   ├── index.tsx         # Redirects to /pair or /status
│   ├── pair.tsx          # Enter device token + server address
│   └── status.tsx        # Sharing toggle, queue/sync status, unpair
└── lib/                  # Everything else — no Expo/RN imports except where noted
    ├── types.ts          # Mirrors the backend's request/response shapes
    ├── config.ts          # Constants shared across the app (must match the backend)
    ├── validation.ts       # Client-side mirror of the server's fix validation
    ├── queue.ts             # Pure offline-queue operations (enqueue/batch/remove)
    ├── id.ts                 # Local id generator for queued fixes
    ├── api.ts                 # POST /api/driver/location client
    ├── ingest.ts               # Validates + queues one fix from the location callback
    ├── flush.ts                # Uploads the oldest batch; classifies failures
    ├── locationTask.ts          # expo-location + expo-task-manager wiring (native-facing)
    ├── storage.ts                # SecureStore: the paired device token
    ├── queueStorage.ts            # AsyncStorage: the offline queue
    └── statusStorage.ts            # AsyncStorage: what the status screen displays
```

The `lib/` modules that don't need a phone (`validation`, `queue`, `id`,
`api`, `ingest`, `flush`) are plain TypeScript and unit-tested directly; the
ones that do (`storage`, `queueStorage`, `statusStorage`, `locationTask`) are
thin wrappers so the business logic around them stays testable. `locationTask.ts`
is the only file that touches `expo-location`/`expo-task-manager` directly.

## Setup

```bash
npm install
npm start        # then press `a` for Android, or scan the QR with Expo Go
```

Background location is **not supported in Expo Go on iOS**, and any native
module added later needs a development build rather than Expo Go — see
`AGENTS.md` in this directory. To build one:

```bash
npx expo run:android   # local Android build, needs Android Studio/SDK
npx expo run:ios       # local iOS build, needs a Mac + Xcode
# or, without either installed locally:
npx eas-cli@latest build --profile development --platform android
```

## Testing

```bash
npm test        # jest — 53 tests: pure logic, storage wrappers, one screen
npm run typecheck
npm run lint
```

There is no Detox/Maestro end-to-end suite yet — the app has not been
exercised on a real device or simulator in this environment (no Android SDK,
no macOS). Before shipping, pair a real phone against a running backend and
walk through: pairing, granting background location, backgrounding the app
and confirming the vehicle still moves on the dashboard map, going offline
and back online, and unpairing.

## Known gaps

- **No QR pairing yet.** The token is entered by hand; the plan
  (`docs/driver-phone-tracking.md`) mentions a QR code as a nicer flow.
- **No "who am I" endpoint.** The status screen only learns the assigned
  vehicle's registration number after the first successful upload
  (`DriverLocationResult.vehicle_registration_number`) — there is no
  authenticated-by-device-token read endpoint to show it before that, or to
  show the driver's own name.
- **No `IN_TRANSIT` gate** (by design — see the plan's "Decisions taken").
  The app itself is what limits reporting to on-duty time, by only
  registering the background task while "Sharing location" is on.
- **Not yet built for a store.** No app icons beyond the Expo defaults, no
  EAS project configured, no signing.
