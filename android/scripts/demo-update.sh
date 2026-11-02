#!/usr/bin/env bash
# Demo of distributing the app through the server, run in a visible emulator
# window and checked at each step:
#   1. start a backend whose release directory (DRIVER_APP_DIR) is a fresh
#      folder under android/build/demo, and seed a driver
#   2. build the app twice: v1 (versionCode 1) and v2 (versionCode 2,
#      "0.2.0-demo"); install v1 and pair it
#   3. nothing is published yet, so the app offers no update
#   4. publish v2 with scripts/publish-apk.sh; after Refresh the app shows
#      "Update available", and Download update has the browser download it
#      (the script clicks through Chrome's first-run screens)
#   5. GET /api/driver-app/latest describes v2, and the download is v2's APK
#      byte for byte
#   6. install v2 over v1 (what the driver's tap on the download does): still
#      paired, now on 0.2.0-demo, and no update offered
#   7. the dashboard (frontend/tests/driver-app.demo.ts): the driver's page
#      shows v0.2.0-demo, a QR code, and a link that downloads the same APK
# The phone run is recorded to android/build/demo/update-demo.webm.
#
# Needs its own backend, so the release directory is known: if something
# already answers on API_PORT, pick another, e.g. API_PORT=8081.
#
# Usage (from the repo root):  android/scripts/demo-update.sh
# Env overrides: see lib.sh.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

RECORDING=""
cleanup() {
    [ -n "$RECORDING" ] && "$ADB" emu screenrecord stop >/dev/null 2>&1 || true
    cleanup_backend
}
trap cleanup EXIT

RELEASES="$OUT/driver-app-releases"
V2_NAME=0.2.0-demo

if curl -sf "$API/api/health" >/dev/null; then
    echo "Something already answers on port $API_PORT; this demo needs a backend of its own"
    echo "(to know where releases are published). Run it with e.g. API_PORT=8081."
    exit 1
fi
rm -rf "$RELEASES"
export DRIVER_APP_DIR="$RELEASES"
start_backend
seed_driver

# Built before the emulator starts, to spare it the memory.
build_apk -Pdriver.versionCode=2 "-Pdriver.versionName=$V2_NAME"
cp "$ROOT/android/app/build/outputs/apk/debug/app-debug.apk" "$OUT/app-v2.apk"
build_apk
cp "$ROOT/android/app/build/outputs/apk/debug/app-debug.apk" "$OUT/app-v1.apk"

boot_emulator
install_fresh   # v1, just built
for perm in ACCESS_FINE_LOCATION ACCESS_COARSE_LOCATION POST_NOTIFICATIONS; do
    "$ADB" shell pm grant "$PKG" "android.permission.$perm"
done

# True if the element tagged $1 is on screen now.
on_screen() {
    "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null
    "$ADB" shell cat /sdcard/ui.xml | grep -q "resource-id=\"$1\""
}

step "Recording to $OUT/update-demo.webm"
rm -f "$OUT/update-demo.webm"
"$ADB" emu screenrecord start --time-limit 600 "$OUT/update-demo.webm" >/dev/null
RECORDING=1

step "Pairing v1"
pair_app
sleep 3
on_screen update-card && { echo "An update is offered, but nothing is published"; exit 1; }
echo "App version 0.1.0, no update offered (nothing published)"
sleep "$PAUSE"

step "Publishing v2 on the server"
"$ROOT/android/scripts/publish-apk.sh" "$OUT/app-v2.apk" "$RELEASES"
LATEST=$(curl -sf "$API/api/driver-app/latest")
[ "$(json "['data']['version_name']" <<<"$LATEST")" = "$V2_NAME" ] || { echo "API: $LATEST"; exit 1; }
echo "API:      GET /api/driver-app/latest is $V2_NAME (versionCode $(json "['data']['version_code']" <<<"$LATEST"))"
curl -sf "$API/api/driver-app/download" -o "$OUT/downloaded.apk"
WANT=$(sha256sum "$OUT/app-v2.apk" | cut -d' ' -f1)
[ "$(sha256sum "$OUT/downloaded.apk" | cut -d' ' -f1)" = "$WANT" ] || { echo "The download isn't v2's APK"; exit 1; }
echo "Download: GET /api/driver-app/download is v2's APK (sha256 ${WANT:0:16}…)"

step "The app offers the update"
tap_tag refresh-button
wait_text "Update available: version $V2_NAME"
echo "Status screen: \"Update available: version $V2_NAME\""
sleep "$PAUSE"; sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/update-available.png"

step "Download update: the browser downloads v2"
"$ADB" shell rm -f "/sdcard/Download/logistics-driver-*.apk"
tap_tag download-update
sleep 4
TOP=$("$ADB" shell dumpsys activity activities | grep -m 1 -E 'topResumedActivity|mResumedActivity' \
    | grep -o -E '[A-Za-z0-9_.]+/[A-Za-z0-9_.$]+' | head -n 1)
[ -n "$TOP" ] && [ "${TOP%%/*}" != "$PKG" ] || { echo "Download update didn't open a browser (top: ${TOP:-?})"; exit 1; }
echo "Opened:   $TOP"
"$ADB" exec-out screencap -p >"$OUT/update-browser.png"
# A fresh Chrome first shows its welcome screens, and may ask before saving
# an APK. Click through whichever of these appear.
APK_ON_PHONE=""
for _ in $(seq 1 40); do
    APK_ON_PHONE=$("$ADB" shell ls /sdcard/Download/ 2>/dev/null | tr -d '\r' | grep -m 1 -E "^logistics-driver-$V2_NAME.*\.apk$" || true)
    [ -n "$APK_ON_PHONE" ] && break
    tap_text "Stay signed out" "Use without an account" "No thanks" "Accept & continue" "Got it" \
        "Download" "Download anyway" "Allow" "Continue" && sleep 2 || sleep 1
done
if [ -n "$APK_ON_PHONE" ]; then
    sleep 2   # let the browser finish writing it
    "$ADB" pull "/sdcard/Download/$APK_ON_PHONE" "$OUT/downloaded-on-phone.apk" >/dev/null
    [ "$(sha256sum "$OUT/downloaded-on-phone.apk" | cut -d' ' -f1)" = "$WANT" ] \
        || { echo "The APK the phone downloaded isn't v2's"; exit 1; }
    echo "Phone:    downloaded /sdcard/Download/$APK_ON_PHONE, v2's APK byte for byte"
else
    echo "Note: the browser didn't save the APK within 40 s (its screens may differ on this"
    echo "emulator); the server download was checked above. See $OUT/update-browser.png."
fi
sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/update-downloaded.png"

step "Installing v2 over v1 (what the driver's tap on the download does)"
"$ADB" install -r "$OUT/app-v2.apk" >/dev/null
"$ADB" shell am start -n "$PKG/.MainActivity" >/dev/null
wait_text "Ramesh Patil"
wait_text "$V2_NAME"
sleep 2
on_screen update-card && { echo "v2 still offers an update"; exit 1; }
echo "Still paired as Ramesh Patil, App version $V2_NAME, no update offered"
sleep "$PAUSE"; sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/updated-status.png"

step "Stopping the recording"
"$ADB" emu screenrecord stop >/dev/null
RECORDING=""
echo "Video:       $OUT/update-demo.webm"
echo "Screenshots: $OUT/update-available.png, update-browser.png, update-downloaded.png, updated-status.png"

step "Dashboard: the driver's page offers the same APK"
WEB_PORT=${WEB_PORT:-$([ "$API_PORT" = 8080 ] && echo 5173 || echo 5174)}
(cd "$ROOT/frontend" && DRIVER_APP_DEMO_ORG_NAME="$ORG_NAME" DRIVER_APP_DEMO_ORG_PASSWORD="$ORG_PASSWORD" \
    DRIVER_APP_DEMO_DRIVER_ID="$DRIVER_ID" DRIVER_APP_DEMO_VERSION="$V2_NAME" DRIVER_APP_DEMO_SHA256="$WANT" \
    E2E_API_PORT="$API_PORT" E2E_WEB_PORT="$WEB_PORT" \
    DISPLAY=${DISPLAY:-:0} XDG_SESSION_TYPE=x11 npm run --silent test:e2e:demo:driver-app)
echo "Dashboard video: $(ls -t "$ROOT"/frontend/test-results/driver-app*/video.webm 2>/dev/null | head -n 1)"
echo "The emulator is left running with v2 installed."
