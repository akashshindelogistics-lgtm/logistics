# Logistics Driver (Android)

A native Kotlin app for the driver's phone. It pairs with a driver using a
device token and (from phase 3) reports the phone's location, so the driver's
assigned vehicle moves on the dispatch map without a hardware GPS tracker.
It replaces the Expo app in `../mobile/`.

Design, decisions and phases: [docs/android-driver-app.md](../docs/android-driver-app.md).
Backend contract: [docs/driver-phone-tracking.md](../docs/driver-phone-tracking.md).

## Status

Phase 2 (skeleton) is built:

- **Pairing screen:** enter the server address and the device token. The app
  checks them with `GET /api/driver/me` before saving anything.
- **Status screen:** the driver, organization, active status, assigned
  vehicle and its last position, from `GET /api/driver/me`. It has Refresh,
  Unpair (with confirmation) and the app version.
- **"Share my location" switch:** a stub that sends nothing yet. It's
  disabled for an inactive driver or one without a vehicle.

Location reporting (the foreground service, offline queue and uploads) is
phase 3.

## Setup

One-time, on this machine (details in the plan doc):

- Android Studio extracted to `~/Downloads/android-studio`; its bundled JDK is `jbr/`.
- SDK at `~/Android/Sdk` with platforms 36 and 37 and build-tools 36, plus the
  `Medium_Phone` emulator (API 37, with Google Play services).

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
```

Install on a running emulator or USB phone with
`$ANDROID_HOME/platform-tools/adb install -r android/app/build/outputs/apk/debug/app-debug.apk`.

## Demo

```sh
android/scripts/demo-pairing.sh
```

The script:

1. starts the backend if it isn't running;
2. seeds an org, a vehicle and a driver with a device token;
3. opens the emulator in a window;
4. installs the app and pairs it by typing the token over adb;
5. reports one location fix and taps Refresh.

The run is recorded to `android/build/demo/pairing-demo.mp4`, with a final
screenshot in `android/build/demo/paired-status.png`.

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
├── domain/              # pure Kotlin: server address and device token parsing
├── net/                 # DriverApi (OkHttp + kotlinx.serialization), DTOs, error messages
├── data/                # PairingRepository, PairingStore (DataStore), TokenCipher (Android Keystore)
└── ui/                  # DriverApp (pair vs status), screens, view models, theme
```

Versions are pinned in `gradle/libs.versions.toml`: AGP 9.4.1, Gradle 9.6.0,
Kotlin 2.4.20, Compose BOM 2026.09.00. `compileSdk` is 37, `targetSdk` 36 and
`minSdk` 26.
