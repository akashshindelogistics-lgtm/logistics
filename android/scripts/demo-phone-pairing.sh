#!/usr/bin/env bash
# Demo of pairing a driver's phone from the dashboard, with the emulator and
# the dashboard (Chromium) side by side, checked at each step:
#   1. seed an org, vehicle and driver with no phone paired
#   2. dashboard (frontend/tests/phone-pairing.demo.ts): the driver's page
#      issues a device token as a QR code, and the demo decodes the QR code
#      off the screen
#   3. the decoded link opens on the emulator, as when the camera reads the
#      code: the pairing form fills in, and pairs only when Pair is tapped
#   4. the database has the phone paired and seen; the dashboard shows
#      "last reported just now"
#   5. the dashboard unpairs: the token is gone from the database, and the
#      app's next Refresh is refused
# The phone run is recorded to android/build/demo/phone-pairing-demo.webm.
#
# The QR code holds the dashboard's own address (Vite, which proxies /api),
# so the emulator reaches it through `adb reverse`.
#
# Usage (from the repo root):  android/scripts/demo-phone-pairing.sh
# Env overrides: see lib.sh; WEB_PORT (default 5173, or 5174 when API_PORT
# isn't 8080).
set -euo pipefail
source "$(dirname "$0")/lib.sh"

WEB_PORT=${WEB_PORT:-$([ "$API_PORT" = 8080 ] && echo 5173 || echo 5174)}
HANDOFF="$OUT/pairing-handoff"
RECORDING=""
DASHBOARD_PID=""
cleanup() {
    [ -n "$RECORDING" ] && "$ADB" emu screenrecord stop >/dev/null 2>&1 || true
    [ -n "$DASHBOARD_PID" ] && kill "$DASHBOARD_PID" 2>/dev/null || true
    "$ADB" reverse --remove "tcp:$WEB_PORT" >/dev/null 2>&1 || true
    cleanup_backend
}
trap cleanup EXIT

command -v mysql >/dev/null || { echo "The demo checks the database with the mysql client; install it first."; exit 1; }
# The driver's row: "<token set> <paired_at set> <last_seen set>", as 0/1.
db_pairing() {
    mysql -h "${MYSQL_HOST:-localhost}" -P "${MYSQL_PORT:-3306}" -u "${MYSQL_USER:-root}" \
        -p"${MYSQL_PASSWORD:-password}" "${MYSQL_DATABASE:-logistics}" -N -B 2>/dev/null -e \
        "SELECT CONCAT(device_token_hash IS NOT NULL, ' ', device_paired_at IS NOT NULL, ' ', device_last_seen_at IS NOT NULL)
         FROM Drivers WHERE id = '$DRIVER_ID'"
}

start_backend
seed_driver
# seed_driver pairs a token for the other demos; start this one unpaired.
curl -sf -X DELETE "$API/api/drivers/$DRIVER_ID/device-token" -H "Authorization: $AUTH" >/dev/null
[ "$(db_pairing)" = "0 0 0" ] || { echo "Database: driver should start unpaired, got $(db_pairing)"; exit 1; }
echo "Database: no phone paired"

build_apk
boot_emulator
install_fresh
"$ADB" reverse "tcp:$WEB_PORT" "tcp:$WEB_PORT" >/dev/null

step "Recording to $OUT/phone-pairing-demo.webm"
rm -f "$OUT/phone-pairing-demo.webm"
"$ADB" emu screenrecord start --time-limit 600 "$OUT/phone-pairing-demo.webm" >/dev/null
RECORDING=1
"$ADB" shell am start -n "$PKG/.MainActivity" >/dev/null
wait_text "Pair this phone"

step "Dashboard: Pair a phone (opens Chromium)"
rm -rf "$HANDOFF" && mkdir -p "$HANDOFF"
(cd "$ROOT/frontend" && PAIRING_DEMO_HANDOFF="$HANDOFF" PAIRING_DEMO_ORG_NAME="$ORG_NAME" \
    PAIRING_DEMO_ORG_PASSWORD="$ORG_PASSWORD" PAIRING_DEMO_DRIVER_ID="$DRIVER_ID" \
    E2E_API_PORT="$API_PORT" E2E_WEB_PORT="$WEB_PORT" \
    DISPLAY=${DISPLAY:-:0} XDG_SESSION_TYPE=x11 npm run --silent test:e2e:demo:phone-pairing) \
    >"$OUT/phone-pairing-dashboard.log" 2>&1 &
DASHBOARD_PID=$!
for _ in $(seq 1 240); do
    [ -f "$HANDOFF/link.txt" ] && break
    kill -0 "$DASHBOARD_PID" 2>/dev/null || { echo "The dashboard demo stopped:"; tail -20 "$OUT/phone-pairing-dashboard.log"; exit 1; }
    sleep 1
done
LINK=$(cat "$HANDOFF/link.txt")
echo "QR code on the dashboard reads: $LINK"

step "The phone opens the pairing link"
"$ADB" shell "am start -a android.intent.action.VIEW -d '$LINK'" >/dev/null
wait_text "Filled in from the pairing code"
[ "$(tag_text server-url | grep -o "http://localhost:$WEB_PORT")" = "http://localhost:$WEB_PORT" ] \
    || { echo "Server address not filled in: $(tag_text server-url)"; exit 1; }
[ "$(db_pairing)" = "1 1 0" ] || { echo "Database: expected paired but not yet seen, got $(db_pairing)"; exit 1; }
echo "Form filled in from the code; nothing sent yet (database: paired, not yet seen)"
sleep "$PAUSE"; sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/pairing-from-code.png"

step "The driver taps Pair"
tap_tag pair-button
wait_text "Ramesh Patil"
wait_text "$REG"
[ "$(db_pairing)" = "1 1 1" ] || { echo "Database: expected paired and seen, got $(db_pairing)"; exit 1; }
echo "Paired as Ramesh Patil ($REG); database: paired and seen"
sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/pairing-paired.png"
touch "$HANDOFF/phone-paired"

step "Dashboard: last reported, then Unpair"
for _ in $(seq 1 120); do
    [ -f "$HANDOFF/unpaired" ] && break
    kill -0 "$DASHBOARD_PID" 2>/dev/null || { echo "The dashboard demo stopped:"; tail -20 "$OUT/phone-pairing-dashboard.log"; exit 1; }
    sleep 1
done
[ -f "$HANDOFF/unpaired" ] || { echo "The dashboard didn't unpair"; exit 1; }
[ "$(db_pairing)" = "0 0 0" ] || { echo "Database: expected unpaired, got $(db_pairing)"; exit 1; }
echo "Database: token cleared"

step "The phone's next request is refused"
tap_tag refresh-button
wait_text "wasn't recognised"
echo "App: \"This device token wasn't recognised\""
sleep "$PAUSE"; sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/pairing-refused.png"

wait "$DASHBOARD_PID" || { echo "The dashboard demo failed:"; tail -30 "$OUT/phone-pairing-dashboard.log"; exit 1; }
DASHBOARD_PID=""
grep -E '✓|passed' "$OUT/phone-pairing-dashboard.log" | sed 's/^/Dashboard: /'

step "Stopping the recording"
"$ADB" emu screenrecord stop >/dev/null
RECORDING=""
echo "Video:           $OUT/phone-pairing-demo.webm"
echo "Dashboard video: $(ls -t "$ROOT"/frontend/test-results/phone-pairing*/video.webm 2>/dev/null | head -n 1)"
echo "Screenshots:     $OUT/pairing-from-code.png, pairing-paired.png, pairing-refused.png"
