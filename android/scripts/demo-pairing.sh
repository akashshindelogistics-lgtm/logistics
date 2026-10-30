#!/usr/bin/env bash
# Phase 2 demo, run in a visible emulator window:
#   1. seed an org, a vehicle and a driver with a device token on the local backend
#   2. build and install the debug app on the emulator
#   3. pair the phone by typing the token (driven over adb, slowed down to watch)
#   4. report one location fix with the device token and tap Refresh, so the
#      status screen picks up the vehicle's new position from GET /api/driver/me
# The run is recorded to android/build/demo/pairing-demo.mp4.
#
# Usage (from the repo root):  android/scripts/demo-pairing.sh
# Env overrides: see lib.sh.
set -euo pipefail
source "$(dirname "$0")/lib.sh"
trap cleanup_backend EXIT

start_backend
seed_driver
build_apk
boot_emulator
install_fresh

step "Recording to $OUT/pairing-demo.mp4"
"$ADB" shell screenrecord --time-limit 170 /sdcard/pairing-demo.mp4 &
RECORD_PID=$!
sleep 1

step "Pairing (checks the token with GET /api/driver/me)"
pair_app
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
