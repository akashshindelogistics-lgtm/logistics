# Logistics Driver (Android)

A native Kotlin app for the driver's phone. It pairs with a driver using a
device token and reports the phone's location, so the driver's
assigned vehicle moves on the dispatch map without a hardware GPS tracker.
It replaces the Expo app in `../mobile/`.

Design, decisions and phases: [docs/android-driver-app.md](../docs/android-driver-app.md).
Backend contract: [docs/driver-phone-tracking.md](../docs/driver-phone-tracking.md).

## Status

Phases 2 (skeleton), 3 (background reporting) and 4 (tests and demo) are built:

- **Pairing screen:** enter the server address and the device token. The app
  checks them with `GET /api/driver/me` before saving anything.
- **Status screen:** the driver, organization, active status, assigned
  vehicle and its last position, from `GET /api/driver/me`. It has Refresh,
  Unpair (with confirmation) and the app version.
- **"Share my location":** asks for location (and notification) permission,
  then starts a foreground service that takes a fix every 30 s or 50 m,
  queues it in Room and uploads in batches of 100. Fixes queue while offline
  and upload when the network is back (WorkManager). Queued count, last fix,
  last sync and Sync now are on the status screen.
- **Restarts:** after a reboot or an app update, a "Tap to resume"
  notification; the status screen warns when sharing is on but stopped.
- **Battery:** a one-time explanation and a card with the phone brand's
  battery settings steps.

Phase 5 (signed release APK, icon, driver install guide, CI) comes next.

## Setup

One-time, on this machine (details in the plan doc):

- Android Studio extracted to `~/Downloads/android-studio`; its bundled JDK is `jbr/`.
- SDK at `~/Android/Sdk` with platforms 36 and 37 and build-tools 36, plus the
  `Medium_Phone_API_37.0` emulator (API 37, with Google Play services).

For command-line builds:

```sh
export ANDROID_HOME=$HOME/Android/Sdk
export JAVA_HOME=$HOME/Downloads/android-studio/jbr
```

Or open `android/` in Android Studio (File > Open), which uses its own JDK and SDK.

## Build and test

From the repo root:

```sh
android/gradlew -p android assembleDebug       # app/build/outputs/apk/debug/app-debug.apk
android/gradlew -p android testDebugUnitTest   # JVM unit tests
android/gradlew -p android lintDebug           # Android lint
android/gradlew -p android connectedDebugAndroidTest   # on a running emulator or phone
```

The instrumented tests (`app/src/androidTest`) cover the Room queue on a real
SQLite database and the pairing screen in the running app, against a
MockWebServer on the phone. They unpair the app, so don't run them on a phone
in use.

Install on a running emulator or USB phone with
`$ANDROID_HOME/platform-tools/adb install -r android/app/build/outputs/apk/debug/app-debug.apk`.

## Demos

Both open the emulator in a window, start the backend if nothing answers on
port 8080 (`API_PORT=8081` to use another), seed an org, a vehicle and a
driver with a device token, install the app and pair it by typing the token
over adb. Shared steps are in `scripts/lib.sh`.

```sh
android/scripts/demo-tracking.sh   # phase 4 end to end, about 3 minutes
android/scripts/demo-pairing.sh    # pairing only, under a minute
```

`demo-tracking.sh` switches sharing on and drives the emulator along the
Chakan - Pune road with `adb emu geo fix`, checking `GET /api/vehicles` after
each position. It then turns the emulator's network off, checks that fixes
queue on the phone while the backend stays put, and turns the network back
on to check that the queue uploads. Any check that fails stops the script. Recorded
to `android/build/demo/tracking-demo.webm`, with a final screenshot in
`tracking-status.png`.

`demo-pairing.sh` reports one location fix with curl and taps Refresh.
Recorded to `android/build/demo/pairing-demo.mp4`.

## Connecting to a backend

- **Emulator:** the debug build pre-fills `http://10.0.2.2:8080`, which is
  `cargo run` on this machine as seen from the emulator.
- **USB phone:** run `adb reverse tcp:8080 tcp:8080`, then use
  `http://localhost:8080`.
- **Real deployment:** an `https://` address. Release builds refuse plain
  `http://`; debug builds allow it only for the three local addresses above
  (`src/debug/res/xml/network_security_config.xml`).

## Layout

```
app/src/main/java/com/logistics/driver/
├── Config.kt            # constants mirrored from the backend
├── DriverApplication.kt, AppContainer.kt, MainActivity.kt
├── domain/              # pure Kotlin: input parsing, fix validation, flush outcomes, battery advice
├── net/                 # DriverApi (OkHttp + kotlinx.serialization), DTOs, error messages
├── data/                # pairing (DataStore + Keystore), Room fix queue, StatusStore, LocationReporter
├── tracking/            # LocationService, FlushWorker, BootReceiver, notifications
└── ui/                  # DriverApp (pair vs status), screens, view models, theme
```

Versions are pinned in `gradle/libs.versions.toml`: AGP 9.4.1, Gradle 9.6.0,
Kotlin 2.4.20, Compose BOM 2026.09.00, Room 2.8.5 (KSP 2.3.12), WorkManager 2.12.0,
Play services location 21.4.0. `compileSdk` is 37, `targetSdk` 36 and
`minSdk` 26.
