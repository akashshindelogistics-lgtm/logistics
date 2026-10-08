# Native Android driver app (plan)

A Kotlin Android app on the driver's phone that reports the phone's location
to the logistics backend, so a vehicle without a hardware GPS tracker still
shows up live on the dashboard. It replaces the Expo / React Native app in
`mobile/` (phase 4 of [driver-phone-tracking.md](driver-phone-tracking.md)).

Tracked by the `todo.org` item *"Native Android driver app (Kotlin) to replace
the React Native/Expo app in mobile/"*. This document is its phase 1.

## Why native instead of Expo

The Expo app works on paper but has never run on a phone. The parts that
decide whether a driver's position actually arrives all sit underneath Expo:

- **The foreground service.** Background reporting on Android *is* a
  location-type foreground service with a persistent notification. Expo
  wraps it (`expo-location` + `expo-task-manager`) and hides the lifecycle:
  what happens when the OS kills the process, how the service restarts, and
  what the notification says and does.
- **OEM background killing.** Xiaomi, Vivo, Oppo, Realme and Samsung, the
  phones drivers in India mostly carry, kill background apps much more
  aggressively than stock Android. Coping with that needs direct access to
  battery-optimisation settings and intents, which is plain Android API.
- **Permissions.** The location permission flow on Android 10+ (foreground
  first, background as a separate step, notifications on 13+) is easier to get
  right, and to explain to a driver, when the app owns it.
- **Tooling.** Android Studio, Gradle and an emulator can run here. The Expo
  app's development build needed either Android Studio anyway or EAS cloud
  builds.

Drivers use Android, so iOS is out of scope. Nothing on the backend changes.

## What the app does

Same scope as the Expo app, so `mobile/` can be retired with nothing lost:

1. **Pair.** A dispatcher issues a device token for the driver
   (`POST /api/drivers/{id}/device-token/rotate`, shown once). The driver
   enters it with the server address on the pairing screen. Once the dashboard
   pairing panel (driver tracking phase 2) exists, the driver scans its QR
   code instead of typing.
2. **Share location during a shift.** A single "Sharing location" switch on
   the status screen starts and stops a foreground service. While it runs, a
   notification says location is being shared, with a Stop action.
3. **Report.** The service requests a fix every 30 s or 50 m. Each fix is
   validated with the server's rules and appended to an offline queue, and the
   queue is uploaded in batches of up to 100.
4. **Survive bad signal.** With no network, fixes keep queueing (capped at
   2000, oldest dropped) and are uploaded once the connection returns.
5. **Show status.** Vehicle last reported to, queued fix count, last sync time
   and outcome, a Sync now button, and Unpair.

## Backend contract (unchanged)

All of this is already built and tested. See "Phase 1 as built" in
[driver-phone-tracking.md](driver-phone-tracking.md).

`POST {server}/api/driver/location`, header `Authorization: Bearer <device token>`:

```json
{ "fixes": [ { "latitude": 18.52, "longitude": 73.85, "recorded_at": 1791400000,
               "accuracy_m": 8.0, "speed_mps": 5.1 } ] }
```

- `recorded_at` is **unix seconds** of when the phone captured the fix, not
  when it was uploaded.
- 1-100 fixes per request. **One invalid fix rejects the whole batch**, so the
  app validates before queueing:
  - latitude in -90..90 and longitude in -180..180;
  - `recorded_at` > 0 and at most 300 s ahead of the clock;
  - `accuracy_m` and `speed_mps`, if present, finite and non-negative.
- `200` returns `{ accepted, location_updated, vehicle_registration_number,
  location }`. `location_updated: false` means the server already had a newer
  fix, which is not an error.
- `401` bad token, `403` inactive driver, `409` no vehicle assigned, `400`
  invalid batch.

## Behaviour carried over from the Expo app

| Concern | Expo app (`mobile/src/lib/`) | Native app |
| --- | --- | --- |
| Fix interval | 30 s / 50 m, balanced accuracy | Same, via `LocationRequest` (`PRIORITY_BALANCED_POWER_ACCURACY`, `setMinUpdateDistanceMeters(50f)`) |
| Validation | `validation.ts` | `FixValidator.kt`, same rules and messages, same test cases |
| Queue cap | 2000, drop oldest | Same, in Room |
| Batch size | 100 oldest first | Same |
| After upload `200` | Remove the batch, record outcome | Same, in one Room transaction |
| `401` / `403` / `409` | Keep the queue, show a specific message | Same messages; stop retrying until the next fix or Sync now |
| `400` | Keep the queue (should not happen) | Same |
| Network error / `5xx` | Keep, retry on next fix | Keep; WorkManager retries with a network constraint and backoff |
| Token storage | `expo-secure-store` | Encrypted with an Android Keystore key, stored in DataStore |
| Server address | Must start with `http://` or `https://` | Same check; release builds require `https://` |

The constants (100, 300, 2000, 30 s, 50 m) go in one `Config.kt`, with a
comment naming the Rust constant each one mirrors, as `config.ts` does now.

## Technical decisions

| Area | Decision | Why |
| --- | --- | --- |
| Language / UI | Kotlin, Jetpack Compose, Material 3, single activity | Current Android default, two small screens |
| Location | `FusedLocationProviderClient` (Google Play services) | Best battery/accuracy trade-off; every phone drivers carry has Play services. No `LocationManager` fallback in v1 |
| Background | Foreground service, `foregroundServiceType="location"`, started from the status screen | The only reliable way to keep reporting while the screen is off |
| Queue | Room table `queued_fix` | Survives process death; batching, cap and delete-by-id are single queries |
| Upload | OkHttp + kotlinx.serialization; one `DriverApi` class | A single endpoint does not need Retrofit |
| Retries | WorkManager unique work (`flush-queue`), `NetworkType.CONNECTED`, exponential backoff | Uploads resume when the network returns even if the service was killed |
| Settings / status | Jetpack DataStore | Small key-value state the UI observes as a `Flow` |
| Token at rest | AES-GCM key in Android Keystore, ciphertext in DataStore | `EncryptedSharedPreferences` (security-crypto) is deprecated |
| DI | Manual (one `AppContainer`) | Too small for Hilt to pay for itself |
| SDK levels | `targetSdk` 36, `minSdk` 26 (Android 8.0); `compileSdk` 37 (latest installed, as of phase 2) | Google Play requires target 36 for new apps and updates from 31 Aug 2026; 26 covers practically every phone in use |
| Build | Gradle Kotlin DSL, version catalog `gradle/libs.versions.toml`, single `:app` module | Versions in one place; exact versions are pinned when the skeleton is created in phase 2 |
| Package id | `com.logistics.driver` (same as the Expo app) | The Expo app was never published, so nothing is lost by reusing it |
| Location in repo | `android/` at the repo root, next to `frontend/` and `mobile/` | `mobile/` is deleted in phase 6 |

### No `ACCESS_BACKGROUND_LOCATION` in v1

The service is always started by the driver tapping "Sharing location" while
the app is on screen, and a foreground service started that way keeps
getting location with the screen off using only the foreground ("while in
use") permission. So the app requests:

- `ACCESS_FINE_LOCATION` (and `ACCESS_COARSE_LOCATION`, which Android 12+
  requires alongside it);
- `POST_NOTIFICATIONS` on Android 13+, so the "sharing location" notification
  shows. Without it the service still runs, but the driver can't see that;
- `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_LOCATION` (install-time).

It does **not** request "Allow all the time". That avoids the separate
settings-page step that drivers often get wrong, and Google Play's extra
review and declaration for background location.

The cost: Android won't let the app start a location service from the
background, so it can't silently restart sharing after a reboot or after the
OS kills it. Instead:

- on `BOOT_COMPLETED`, if sharing was on before the reboot, post a "Tap to
  resume sharing location" notification that opens the app and restarts the
  service;
- show a warning on the status screen if sharing is switched on but the
  service isn't running.

If real use shows drivers forget to resume, revisit this and add background
location with the Play declaration.

### Battery optimisation and OEM killers

- On first start of sharing, if the app isn't exempt from battery
  optimisation, explain why it matters and open
  `Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS`. Don't use the
  direct `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` prompt, which Play
  restricts to a short list of app types.
- Detect the manufacturer (`Build.MANUFACTURER`) and show that brand's steps
  (Xiaomi "Autostart" and "No restrictions", Vivo/Oppo/Realme "Allow
  background activity", Samsung "Never sleeping apps"). Write them up in the
  driver install guide, following [dontkillmyapp.com](https://dontkillmyapp.com).
- The foreground service runs with `START_STICKY`, and the queue is in Room,
  so a killed service loses at most the fix in flight.

### Security

- The device token is a secret: never logged (OkHttp logging redacts the
  `Authorization` header and is off in release), and only stored encrypted.
- Release builds allow HTTPS only. Debug builds allow cleartext to `10.0.2.2`
  (the emulator's address for the host) and `localhost` through a
  debug-only `network_security_config.xml`, for testing against
  `cargo run` on this machine.
- `android:allowBackup="false"`, so the token and queue never land in a cloud
  backup that could be restored onto another phone.

## Project layout (phase 2)

```
android/
├── settings.gradle.kts, build.gradle.kts, gradle.properties
├── gradle/libs.versions.toml, gradle/wrapper/
└── app/src/
    ├── main/java/com/logistics/driver/
    │   ├── MainActivity.kt          # single activity, Compose navigation
    │   ├── AppContainer.kt          # manual DI: DB, DataStore, API, repositories
    │   ├── Config.kt                # constants mirrored from the backend
    │   ├── domain/                  # pure Kotlin: Fix, FixValidator, Batching, FlushOutcome
    │   ├── data/                    # Room (QueuedFix, FixDao, AppDb), PairingStore, StatusStore, TokenCipher
    │   ├── net/                     # DriverApi (OkHttp), DTOs, error classification
    │   ├── tracking/                # LocationService, FlushWorker, BootReceiver
    │   └── ui/                      # PairScreen, StatusScreen, view models, theme
    ├── main/res/xml/network_security_config.xml   # (debug variant only allows cleartext)
    ├── test/                        # JVM unit tests
    └── androidTest/                 # Compose UI and instrumented tests
```

`domain/` has no Android imports, so the validation, batching and outcome
rules are unit-tested as plain Kotlin, the same split `mobile/src/lib/` uses.

## Testing plan (phase 4)

- **JVM unit tests:**
  - `FixValidator`: port every case from `mobile/src/lib/__tests__/validation.test.ts`;
  - queue cap and batching;
  - flush outcome classification for 200, 200-stale, 400/401/403/409, 5xx and
    no network;
  - `DriverApi` against OkHttp `MockWebServer`: request shape, header,
    `recorded_at` in seconds, error bodies.
- **Room:** DAO tests (in-memory database) for enqueue with cap, oldest batch,
  and delete-by-id.
- **Compose UI test:** pairing screen validation (empty token, bad URL) and
  navigation to the status screen.
- **End to end on the emulator:**
  1. Run `cargo run` on the host and create an org, vehicle and driver; assign
     the driver and rotate a device token (a script, like the Playwright
     helpers).
  2. Install the debug APK and pair with server `http://10.0.2.2:8080`.
  3. Start sharing, then feed positions with `adb emu geo fix <lon> <lat>`.
  4. Check `GET /api/vehicles` shows the new location.
  5. Turn the emulator's network off and on, and confirm queued fixes upload.
  6. Record the run with `adb shell screenrecord` as the phase demo.

  This doubles as the "watch it work" demo this repo expects for features.
- **CI:** a GitHub Actions job runs `./gradlew testDebugUnitTest lint
  assembleDebug` when `android/**` changes. Emulator tests stay local at
  first, because emulators on hosted runners are slow and flaky.

## Development setup on this machine

Android Studio is downloaded (`~/Downloads/android-studio-rabbit1-linux.tar.gz`)
and extracted to `~/Downloads/android-studio` (build 262.9437, bundled JDK 25
in `jbr/`). One-time setup still to do:

1. Run `~/Downloads/android-studio/bin/studio.sh`. On this i3/X desktop, run it
   with `XDG_SESSION_TYPE=x11 DISPLAY=:0` if it doesn't open.
2. In the setup wizard, accept the default SDK location (`~/Android/Sdk`).
   The wizard hasn't been run yet, so no SDK is installed.
   Then in SDK Manager install:
   - Android SDK Platform 36;
   - Build-Tools;
   - Platform-Tools;
   - Emulator;
   - a "Google APIs" x86_64 system image for API 36.

   Use Google APIs rather than AOSP, because fused location needs Play
   services.
3. Create an emulator in Device Manager (for example a Pixel with that image).
   KVM is available to this user, so it runs hardware-accelerated.
4. For command-line builds, add to the shell profile:

   ```sh
   export ANDROID_HOME=$HOME/Android/Sdk
   export JAVA_HOME=$HOME/Downloads/android-studio/jbr   # JDK bundled with Studio
   export PATH=$PATH:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator
   ```

5. For a real phone, enable Developer options and USB debugging, then check
   it appears in `adb devices`.

## Phases

1. **Plan** (this document).
2. **Skeleton:** Gradle project under `android/`, theme, navigation, pairing
   screen (token + server URL, stored encrypted), status screen with the
   sharing switch wired to a stub. `./gradlew assembleDebug` builds and the
   app runs on the emulator.
3. **Background reporting:**
   - `LocationService` (foreground, location type) with its notification and
     Stop action;
   - validation, Room queue and batched upload;
   - `FlushWorker` retries, and keeping the queue on 401/403/409;
   - the resume-after-reboot notification and the battery-optimisation
     guidance.
4. **Tests and demo:** the testing plan above, with the emulator end-to-end
   run recorded.
5. **Release:**
   - signed release APK, with the keystore and passwords outside git and
     passed to Gradle through environment variables;
   - app name and icon;
   - a driver install guide (sideloading the APK, permissions, per-brand
     battery settings);
   - the CI build job.
6. **Retire `mobile/`** once the app has been verified on a real phone:
   - delete the Expo app;
   - update README, [driver-phone-tracking.md](driver-phone-tracking.md) and
     the driver tracking phase 4 note in `todo.org`.

## Decisions

- **Distribution: signed APK installed directly (sideloaded).** There's no
  Play Store listing and no Play developer account. What follows from that:
  - **Signing key:** the release keystore must never be lost. Android
    installs an update over the old app only if it's signed with the same
    key; with a new key every driver would have to uninstall first, losing
    the pairing and any queued fixes. Keep the keystore and its passwords
    outside git, with a backup.
  - **Updates are manual:** a driver installs a newer APK over the old one.
    `versionCode` must go up on every release, and the status screen shows
    the app version so the office can tell who is out of date.
  - **The install guide** covers allowing "Install unknown apps" for the
    browser or file manager used, and what to do if Play Protect warns about
    an app from outside the Play Store.
  - Since Play policy no longer applies, the "no `ACCESS_BACKGROUND_LOCATION`"
    decision above rests on keeping driver setup simple, not on Play review.
- **`GET /api/driver/me` is added to the backend.** It's authenticated by the
  device token, like `POST /api/driver/location`, and returns the driver's
  name and their assigned vehicle (or none). The app uses it:
  - on the pairing screen, to check the token and server address before
    saving them, then show "Paired as <name>";
  - on the status screen, to show the driver and vehicle before the first
    upload, and to show "No vehicle assigned" up front instead of only after
    a `409`.

## Open questions

- **Hosted backend:** a real phone needs a public HTTPS server address. This
  waits on the "deploy frontend and backend on hosted platform" item in
  `todo.org`. Until then, real-phone testing runs over USB with
  `adb reverse tcp:8080 tcp:8080` (the phone then uses
  `http://localhost:8080`, allowed in debug builds).
- **Shift hours:** should sharing switch itself off after a set time (for
  example 14 h) in case the driver forgets? Not in v1.
