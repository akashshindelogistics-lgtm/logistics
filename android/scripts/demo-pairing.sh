#!/usr/bin/env bash
# Phase 2 demo, run in a visible emulator window:
#   1. seed an org, a vehicle and a driver with a device token on the local backend
#   2. build and install the debug app on the emulator
#   3. pair the phone by typing the token (driven over adb, slowed down to watch)
#   4. report one location fix with the device token (what phase 3's service
#      will do on its own) and tap Refresh, so the status screen picks up the
#      vehicle's new position from GET /api/driver/me
# The run is recorded to android/build/demo/pairing-demo.mp4.
#
# Usage (from the repo root):  android/scripts/demo-pairing.sh
# Env overrides: AVD (default: first from `emulator -list-avds`), API (default http://127.0.0.1:8080),
# ANDROID_HOME (default ~/Android/Sdk), JAVA_HOME (default Studio's bundled JDK).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SDK=${ANDROID_HOME:-$HOME/Android/Sdk}
export ANDROID_HOME=$SDK
export JAVA_HOME=${JAVA_HOME:-$HOME/Downloads/android-studio/jbr}
ADB="$SDK/platform-tools/adb"
EMULATOR="$SDK/emulator/emulator"
# Default: the first emulator Android Studio created (names come from the
# .ini file, e.g. Medium_Phone_API_37.0, not the .avd folder).
AVD=${AVD:-$("$EMULATOR" -list-avds 2>/dev/null | head -n 1)}
API=${API:-http://127.0.0.1:8080}
OUT="$ROOT/android/build/demo"
PKG=com.logistics.driver
PAUSE=${PAUSE:-2}
mkdir -p "$OUT"

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
json() { python3 -c "import json,sys; print(json.load(sys.stdin)$1)"; }

# ── 1. Backend ───────────────────────────────────────────────────────────────
BACKEND_PID=""
cleanup() { [ -n "$BACKEND_PID" ] && kill "$BACKEND_PID" 2>/dev/null || true; }
trap cleanup EXIT

step "Backend at $API"
if ! curl -sf "$API/api/health" >/dev/null; then
    echo "Not running; starting it with cargo run (first build can take a few minutes)"
    (cd "$ROOT" && cargo run --bin logistics-system >"$OUT/backend.log" 2>&1) &
    BACKEND_PID=$!
    for _ in $(seq 1 300); do curl -sf "$API/api/health" >/dev/null && break; sleep 1; done
    curl -sf "$API/api/health" >/dev/null || { echo "Backend didn't start; see $OUT/backend.log"; exit 1; }
fi

# ── 2. Seed data ─────────────────────────────────────────────────────────────
step "Seeding an org, vehicle and driver"
SUFFIX=$(date +%s)
ORG_NAME="Mauli Demo $SUFFIX"
PASSWORD="demo-pass-$SUFFIX"
REG="MH12DM${SUFFIX: -4}"
ORG_ID=$(curl -sf "$API/api/orgs" -H 'Content-Type: application/json' \
    -d "{\"name\":\"$ORG_NAME\",\"address\":\"Chakan MIDC, Pune\",\"password\":\"$PASSWORD\"}" | json "['data']['id']")
AUTH="Bearer $(curl -sf "$API/api/auth/login" -H 'Content-Type: application/json' \
    -d "{\"org_id\":\"$ORG_ID\",\"password\":\"$PASSWORD\"}" | json "['data']['token']")"
curl -sf "$API/api/orgs/$ORG_ID/vehicles" -H "Authorization: $AUTH" -H 'Content-Type: application/json' \
    -d "{\"registration_number\":\"$REG\",\"capacity\":20,\"unit\":\"MetricTon\"}" >/dev/null
DRIVER_ID=$(curl -sf "$API/api/orgs/$ORG_ID/drivers" -H "Authorization: $AUTH" -H 'Content-Type: application/json' \
    -d '{"name":"Ramesh Patil","license_number":"MH12-2019-0042","phone":"+91 98220 00042"}' | json "['data']['id']")
curl -sf -X PUT "$API/api/vehicles/$REG/driver" -H "Authorization: $AUTH" -H 'Content-Type: application/json' \
    -d "{\"driver_id\":\"$DRIVER_ID\"}" >/dev/null
TOKEN=$(curl -sf -X POST "$API/api/drivers/$DRIVER_ID/device-token/rotate" -H "Authorization: $AUTH" | json "['data']['device_token']")
echo "Org: $ORG_NAME   Vehicle: $REG   Driver: Ramesh Patil"
echo "Device token: $TOKEN"

# ── 3. Build, emulator, install ──────────────────────────────────────────────
step "Building the debug APK"
# No Gradle or Kotlin daemon left running afterwards: on a 16 GB machine the
# emulator needs that memory, and starving it makes its watchdog kill it.
"$ROOT/android/gradlew" -p "$ROOT/android" assembleDebug --console=plain -q \
    --no-daemon -Pkotlin.compiler.execution.strategy=in-process

step "Emulator (${AVD:-none found})"
if ! "$ADB" devices | grep -q 'device$'; then
    [ -n "$AVD" ] || { echo "No emulator found; create one in Android Studio's Device Manager, or set AVD=<name>"; exit 1; }
    # 4 cores rather than the AVD's default: plenty for the app, and leaves the
    # host room for the backend.
    EMU_CMD=("$EMULATOR" -avd "$AVD" -cores 4 -no-snapshot-save -no-boot-anim)
    # In its own systemd scope where available: when memory runs short,
    # systemd-oomd kills a whole scope, and it should be the emulator's, not
    # the terminal (and everything else) that launched this script.
    if command -v systemd-run >/dev/null && systemd-run --user --scope true >/dev/null 2>&1; then
        EMU_CMD=(systemd-run --user --scope --quiet --unit="logistics-demo-emulator-$$" "${EMU_CMD[@]}")
    fi
    DISPLAY=${DISPLAY:-:0} XDG_SESSION_TYPE=x11 "${EMU_CMD[@]}" >"$OUT/emulator.log" 2>&1 &
    EMULATOR_PID=$!
    # Wait for boot, but give up if the emulator process exits (bad AVD name,
    # no KVM, ...) instead of hanging in `adb wait-for-device`.
    until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
        kill -0 "$EMULATOR_PID" 2>/dev/null || { echo "The emulator exited:"; tail -5 "$OUT/emulator.log"; exit 1; }
        sleep 2
    done
fi
until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do sleep 2; done
"$ADB" shell input keyevent 82 >/dev/null 2>&1 || true   # wake / dismiss lock screen

"$ADB" install -r "$ROOT/android/app/build/outputs/apk/debug/app-debug.apk" >/dev/null
"$ADB" shell pm clear "$PKG" >/dev/null   # start unpaired

# ── 4. Drive the app ─────────────────────────────────────────────────────────
# Tap the centre of the element whose test tag (exposed as resource-id) is $1.
tap_tag() {
    "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null
    local center
    center=$("$ADB" shell cat /sdcard/ui.xml | python3 -c "
import re, sys
xml = sys.stdin.read()
m = re.search(r'resource-id=\"$1\"[^>]*bounds=\"\[(\d+),(\d+)\]\[(\d+),(\d+)\]\"', xml)
if not m:
    tags = sorted(set(re.findall(r'resource-id=\"([a-z][a-z-]*)\"', xml)))
    sys.exit('no element tagged $1 on screen; tags present: ' + (', '.join(tags) or 'none'))
x1, y1, x2, y2 = map(int, m.groups())
print((x1 + x2) // 2, (y1 + y2) // 2)") || {
        "$ADB" exec-out screencap -p >"$OUT/failed-$1.png" 2>/dev/null || true
        echo "Screenshot of the screen at that moment: $OUT/failed-$1.png"
        return 1
    }
    "$ADB" shell input tap $center
}
# Wait (up to $2 s) until text $1 is on screen.
wait_text() {
    for _ in $(seq 1 "${2:-30}"); do
        "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null
        "$ADB" shell cat /sdcard/ui.xml | grep -q "$1" && return 0
        sleep 1
    done
    echo "Timed out waiting for \"$1\" on screen"; return 1
}

step "Recording to $OUT/pairing-demo.mp4"
"$ADB" shell screenrecord --time-limit 170 /sdcard/pairing-demo.mp4 &
RECORD_PID=$!
sleep 1

step "Opening the app"
"$ADB" shell am start -n "$PKG/.MainActivity" >/dev/null
wait_text "Pair this phone"
sleep "$PAUSE"

step "Typing the device token (server address is pre-filled with the emulator's host)"
tap_tag device-token
sleep 1
"$ADB" shell input text "$TOKEN"
sleep "$PAUSE"
# Enter triggers the field's "Done" IME action, which closes the keyboard.
# (Escape would act as Back and close the app.)
"$ADB" shell input keyevent 66
sleep 1

step "Pairing (checks the token with GET /api/driver/me)"
tap_tag pair-button
wait_text "Ramesh Patil"
wait_text "$REG"
sleep "$PAUSE"; sleep "$PAUSE"

step "Reporting a location fix with the device token, then tapping Refresh"
NOW=$(date +%s)
curl -sf "$API/api/driver/location" -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
    -d "{\"fixes\":[{\"latitude\":18.7606,\"longitude\":73.8636,\"recorded_at\":$NOW,\"accuracy_m\":8}]}" >/dev/null
tap_tag refresh-button
for _ in $(seq 1 15); do
    "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null
    "$ADB" shell cat /sdcard/ui.xml | grep -q "No position yet" || break
    sleep 1
done
sleep "$PAUSE"; sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/paired-status.png"

step "Stopping the recording"
"$ADB" shell pkill -INT screenrecord || true
wait "$RECORD_PID" 2>/dev/null || true
sleep 2
"$ADB" pull /sdcard/pairing-demo.mp4 "$OUT/pairing-demo.mp4" >/dev/null
echo "Video:      $OUT/pairing-demo.mp4"
echo "Screenshot: $OUT/paired-status.png"
echo "The emulator is left running."
