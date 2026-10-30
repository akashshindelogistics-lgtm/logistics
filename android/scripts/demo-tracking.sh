#!/usr/bin/env bash
# Phase 4 end-to-end demo, run in a visible emulator window and checked
# against the backend at each step:
#   1. seed an org, a vehicle and a driver with a device token on the local backend
#   2. build and install the debug app, pair it, and switch sharing on
#   3. drive the emulator along the Chakan - Pune road with `adb emu geo fix`;
#      each position must reach the vehicle on the backend
#   4. cut the emulator's network: new fixes queue on the phone, and the
#      backend keeps the last position it had
#   5. bring the network back: the queue uploads and the backend catches up
#   6. open the dashboard in a browser (frontend/tests/driver-tracking.demo.ts)
#      and check the final position on the Vehicles page and the vehicle page
# At each step the position is checked through the API (GET /api/vehicles)
# and in the database (the Vehicle row in MySQL, with the backend's MYSQL_*
# settings and defaults). The phone run is recorded to
# android/build/demo/tracking-demo.webm, the dashboard to
# frontend/test-results/. It takes about four minutes, because the app asks
# for a fix only every 30 s.
#
# Location and notification permissions are granted over adb, and the app is
# exempted from battery optimisation, so no system dialog interrupts the run
# (both prompts are covered in "Phase 3 as built" in docs/android-driver-app.md).
#
# Usage (from the repo root):  android/scripts/demo-tracking.sh
# Env overrides: see lib.sh.
set -euo pipefail
source "$(dirname "$0")/lib.sh"

RECORDING=""
cleanup() {
    [ -n "$RECORDING" ] && "$ADB" emu screenrecord stop >/dev/null 2>&1 || true
    # Never leave the emulator offline for whoever uses it next.
    "$ADB" shell svc wifi enable >/dev/null 2>&1 || true
    "$ADB" shell svc data enable >/dev/null 2>&1 || true
    cleanup_backend
}
trap cleanup EXIT

# Points along the Chakan - Pune road, about 1.5 km apart (latitude longitude).
ROUTE=(
    "18.7606 73.8636"   # Chakan MIDC
    "18.7480 73.8530"
    "18.7350 73.8420"   # offline from here...
    "18.7210 73.8330"   # ...to here
)

# Move the emulator's GPS to point $1 of ROUTE (geo fix takes longitude first).
move_to() {
    local lat lon
    read -r lat lon <<<"${ROUTE[$1]}"
    echo "Phone at $lat, $lon"
    "$ADB" emu geo fix "$lon" "$lat" >/dev/null
}

# The vehicle's position on the backend as "lat lon" (4 decimals), or "none".
backend_position() {
    curl -sf "$API/api/vehicles" -H "Authorization: $AUTH" | python3 -c "
import json, sys
vehicle = next(v for v in json.load(sys.stdin)['data'] if v['registration_number'] == '$REG')
loc = vehicle.get('location')
print('%.4f %.4f' % (loc['latitude'], loc['longitude']) if loc else 'none')"
}

# The vehicle's row in the database as "lat lon" (4 decimals), or "none".
db_position() {
    mysql -h "${MYSQL_HOST:-localhost}" -P "${MYSQL_PORT:-3306}" -u "${MYSQL_USER:-root}" \
        -p"${MYSQL_PASSWORD:-password}" "${MYSQL_DATABASE:-logistics}" -N -B 2>/dev/null -e \
        "SELECT IFNULL(CONCAT(CAST(latitude AS DECIMAL(10,4)), ' ', CAST(longitude AS DECIMAL(10,4))), 'none') FROM Vehicle WHERE registration_number = '$REG'"
}
command -v mysql >/dev/null || { echo "The demo checks the database with the mysql client; install it first."; exit 1; }

# Wait (up to $2 s) until the backend has the vehicle at point $1 of ROUTE,
# then check the database row says the same. The fix for a new position comes
# on the app's next 30 s tick.
wait_backend_at() {
    local want got db
    want=$(printf '%.4f %.4f' ${ROUTE[$1]})
    for _ in $(seq 1 "${2:-75}"); do
        got=$(backend_position)
        [ "$got" = "$want" ] && break
        sleep 1
    done
    [ "$got" = "$want" ] || { echo "Backend has the vehicle at $got, expected $want"; return 1; }
    echo "API:      vehicle at $got"
    db=$(db_position)
    [ "$db" = "$want" ] || { echo "Database: Vehicle row at ${db:-<no row>}, expected $want"; return 1; }
    echo "Database: Vehicle row at $db"
}

# The number on the status screen's "Queued fixes" row.
queued_fixes() { tag_text queued-fixes | grep -o '[0-9]\+$' || echo "?"; }

# Wait (up to $2 s) until the status screen shows $1 queued fixes.
wait_queued() {
    local n
    for _ in $(seq 1 "${2:-75}"); do
        n=$(queued_fixes)
        [ "$n" = "$1" ] && { echo "Queued fixes on the phone: $n"; return 0; }
        sleep 1
    done
    echo "Queued fixes on the phone: $n, expected $1"; return 1
}

start_backend
seed_driver
build_apk
boot_emulator
install_fresh
for perm in ACCESS_FINE_LOCATION ACCESS_COARSE_LOCATION POST_NOTIFICATIONS; do
    "$ADB" shell pm grant "$PKG" "android.permission.$perm"
done
"$ADB" shell dumpsys deviceidle whitelist "+$PKG" >/dev/null
"$ADB" shell svc wifi enable; "$ADB" shell svc data enable
move_to 0

step "Recording to $OUT/tracking-demo.webm"
rm -f "$OUT/tracking-demo.webm"
"$ADB" emu screenrecord start --time-limit 600 "$OUT/tracking-demo.webm" >/dev/null
RECORDING=1

step "Pairing"
pair_app
sleep "$PAUSE"

step "Switching sharing on"
tap_tag sharing-switch
sleep 2
move_to 0   # a fresh fix now that the app is listening
wait_backend_at 0
# The screen re-reads the vehicle from the backend after an upload.
for i in $(seq 1 15); do
    "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null
    "$ADB" shell cat /sdcard/ui.xml | grep -q "No position yet" || break
    [ "$i" = 15 ] && { echo "The status screen still says \"No position yet\""; exit 1; }
    sleep 1
done
echo "Status screen shows the vehicle's last position"
sleep "$PAUSE"

step "Driving on"
move_to 1
wait_backend_at 1
wait_queued 0 10
sleep "$PAUSE"

step "Network off: fixes queue on the phone"
"$ADB" shell svc wifi disable; "$ADB" shell svc data disable
sleep 3
move_to 2
wait_queued 1
move_to 3
wait_queued 2
BEFORE=$(printf '%.4f %.4f' ${ROUTE[1]})
[ "$(backend_position)" = "$BEFORE" ] && [ "$(db_position)" = "$BEFORE" ] \
    || { echo "The backend moved while the phone was offline"; exit 1; }
echo "API and database still have the last position sent before the network went off"
sleep "$PAUSE"

step "Network back: the queue uploads"
"$ADB" shell svc wifi enable; "$ADB" shell svc data enable
wait_backend_at 3 120
wait_queued 0 30
sleep "$PAUSE"; sleep "$PAUSE"
"$ADB" exec-out screencap -p >"$OUT/tracking-status.png"

step "Stopping the recording"
"$ADB" emu screenrecord stop >/dev/null
RECORDING=""
sleep 2
echo "Video:      $OUT/tracking-demo.webm"
echo "Screenshot: $OUT/tracking-status.png"

step "Dashboard: the vehicle where the phone left it"
if [ "$API_PORT" != 8080 ]; then
    # The Vite dev server proxies /api to 8080 only.
    echo "Skipped: the dashboard dev server talks to port 8080, and this run used $API_PORT"
else
    read -r LAT LON <<<"${ROUTE[3]}"
    (cd "$ROOT/frontend" && DRIVER_DEMO_ORG_NAME="$ORG_NAME" DRIVER_DEMO_ORG_PASSWORD="$ORG_PASSWORD" \
        DRIVER_DEMO_REG="$REG" DRIVER_DEMO_LAT="$LAT" DRIVER_DEMO_LON="$LON" \
        DISPLAY=${DISPLAY:-:0} XDG_SESSION_TYPE=x11 npm run --silent test:e2e:demo:driver-tracking)
    echo "Dashboard video: $(ls -t "$ROOT"/frontend/test-results/driver-tracking*/video.webm 2>/dev/null | head -n 1)"
fi
echo "The emulator is left running, still sharing; switch it off in the app or uninstall."
