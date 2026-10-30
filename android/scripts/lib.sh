# Shared by the emulator demos in this folder; source it, don't run it.
#
# Env overrides: AVD (default: first from `emulator -list-avds`), API_PORT
# (default 8080; the backend is started with PORT=$API_PORT if nothing answers
# there), PAUSE (seconds to linger on each step, default 2), ANDROID_HOME
# (default ~/Android/Sdk), JAVA_HOME (default Studio's bundled JDK).

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SDK=${ANDROID_HOME:-$HOME/Android/Sdk}
export ANDROID_HOME=$SDK
export JAVA_HOME=${JAVA_HOME:-$HOME/Downloads/android-studio/jbr}
ADB="$SDK/platform-tools/adb"
EMULATOR="$SDK/emulator/emulator"
# Default: the first emulator Android Studio created (names come from the
# .ini file, e.g. Medium_Phone_API_37.0, not the .avd folder).
AVD=${AVD:-$("$EMULATOR" -list-avds 2>/dev/null | head -n 1)}
API_PORT=${API_PORT:-8080}
API=http://127.0.0.1:$API_PORT
# The same backend as the emulator sees it.
APP_SERVER=http://10.0.2.2:$API_PORT
OUT="$ROOT/android/build/demo"
PKG=com.logistics.driver
PAUSE=${PAUSE:-2}
mkdir -p "$OUT"

step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
json() { python3 -c "import json,sys; print(json.load(sys.stdin)$1)"; }

# ── Backend ──────────────────────────────────────────────────────────────────
BACKEND_PID=""
# The whole process group: killing only `cargo run` would leave the server
# it started running, holding the port.
cleanup_backend() { [ -n "$BACKEND_PID" ] && kill -- -"$BACKEND_PID" 2>/dev/null || true; }

# Use the backend on $API_PORT, or start this checkout's if nothing answers.
start_backend() {
    step "Backend at $API"
    if curl -sf "$API/api/health" >/dev/null; then
        # Often another worktree's server: say whose, so a surprise is visible.
        local pid
        pid=$(ss -ltnp 2>/dev/null | grep ":$API_PORT\b" | grep -o 'pid=[0-9]*' | head -n 1 | cut -d= -f2)
        echo "Already running${pid:+ (from $(readlink "/proc/$pid/cwd" 2>/dev/null || echo "pid $pid"))}; using it"
        return
    fi
    echo "Not running; starting it with cargo run (first build can take a few minutes)"
    # In its own process group, so cleanup_backend can stop cargo and the server.
    (cd "$ROOT" && PORT=$API_PORT exec setsid cargo run --bin logistics-system >"$OUT/backend.log" 2>&1) &
    BACKEND_PID=$!
    for _ in $(seq 1 300); do curl -sf "$API/api/health" >/dev/null && break; sleep 1; done
    curl -sf "$API/api/health" >/dev/null || { echo "Backend didn't start; see $OUT/backend.log"; exit 1; }
}

# Create an org, a vehicle and a driver assigned to it, and rotate the
# driver's device token. Sets ORG_NAME, ORG_PASSWORD, AUTH, REG, DRIVER_ID and TOKEN.
seed_driver() {
    step "Seeding an org, vehicle and driver"
    local suffix org_id
    suffix=$(date +%s)
    ORG_NAME="Mauli Demo $suffix"
    ORG_PASSWORD="demo-pass-$suffix"
    REG="MH12DM${suffix: -4}"
    org_id=$(curl -sf "$API/api/orgs" -H 'Content-Type: application/json' \
        -d "{\"name\":\"$ORG_NAME\",\"address\":\"Chakan MIDC, Pune\",\"password\":\"$ORG_PASSWORD\"}" | json "['data']['id']")
    AUTH="Bearer $(curl -sf "$API/api/auth/login" -H 'Content-Type: application/json' \
        -d "{\"org_id\":\"$org_id\",\"password\":\"$ORG_PASSWORD\"}" | json "['data']['token']")"
    curl -sf "$API/api/orgs/$org_id/vehicles" -H "Authorization: $AUTH" -H 'Content-Type: application/json' \
        -d "{\"registration_number\":\"$REG\",\"capacity\":20,\"unit\":\"MetricTon\"}" >/dev/null
    DRIVER_ID=$(curl -sf "$API/api/orgs/$org_id/drivers" -H "Authorization: $AUTH" -H 'Content-Type: application/json' \
        -d '{"name":"Ramesh Patil","license_number":"MH12-2019-0042","phone":"+91 98220 00042"}' | json "['data']['id']")
    curl -sf -X PUT "$API/api/vehicles/$REG/driver" -H "Authorization: $AUTH" -H 'Content-Type: application/json' \
        -d "{\"driver_id\":\"$DRIVER_ID\"}" >/dev/null
    TOKEN=$(curl -sf -X POST "$API/api/drivers/$DRIVER_ID/device-token/rotate" -H "Authorization: $AUTH" | json "['data']['device_token']")
    echo "Org: $ORG_NAME   Vehicle: $REG   Driver: Ramesh Patil"
    echo "Device token: $TOKEN"
}

# ── App and emulator ─────────────────────────────────────────────────────────
build_apk() {
    step "Building the debug APK"
    # No Gradle or Kotlin daemon left running afterwards: on a 16 GB machine
    # the emulator needs that memory, and starving it makes its watchdog kill it.
    "$ROOT/android/gradlew" -p "$ROOT/android" assembleDebug --console=plain -q \
        --no-daemon -Pkotlin.compiler.execution.strategy=in-process
}

# Boot $AVD in a visible window unless a device is already attached.
boot_emulator() {
    step "Emulator (${AVD:-none found})"
    if ! "$ADB" devices | grep -q 'device$'; then
        [ -n "$AVD" ] || { echo "No emulator found; create one in Android Studio's Device Manager, or set AVD=<name>"; exit 1; }
        # 4 cores rather than the AVD's default: plenty for the app, and
        # leaves the host room for the backend.
        local cmd=("$EMULATOR" -avd "$AVD" -cores 4 -no-snapshot-save -no-boot-anim)
        # In its own systemd scope where available: when memory runs short,
        # systemd-oomd kills a whole scope, and it should be the emulator's,
        # not the terminal (and everything else) that launched this script.
        if command -v systemd-run >/dev/null && systemd-run --user --scope true >/dev/null 2>&1; then
            cmd=(systemd-run --user --scope --quiet --unit="logistics-demo-emulator-$$" "${cmd[@]}")
        fi
        DISPLAY=${DISPLAY:-:0} XDG_SESSION_TYPE=x11 "${cmd[@]}" >"$OUT/emulator.log" 2>&1 &
        local pid=$!
        # Wait for boot, but give up if the emulator process exits (bad AVD
        # name, no KVM, ...) instead of hanging in `adb wait-for-device`.
        until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
            kill -0 "$pid" 2>/dev/null || { echo "The emulator exited:"; tail -5 "$OUT/emulator.log"; exit 1; }
            sleep 2
        done
    fi
    until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do sleep 2; done
    "$ADB" shell input keyevent 82 >/dev/null 2>&1 || true   # wake / dismiss lock screen
}

# Install the APK just built and wipe the app's data, so it starts unpaired.
install_fresh() {
    "$ADB" install -r "$ROOT/android/app/build/outputs/apk/debug/app-debug.apk" >/dev/null
    "$ADB" shell pm clear "$PKG" >/dev/null
}

# ── Driving the UI over adb ──────────────────────────────────────────────────
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

# The text of the element tagged $1 and its children, joined by spaces.
tag_text() {
    "$ADB" shell uiautomator dump /sdcard/ui.xml >/dev/null
    "$ADB" shell cat /sdcard/ui.xml | python3 -c "
import sys, xml.etree.ElementTree as ET
root = ET.fromstring(sys.stdin.read())
for node in root.iter('node'):
    if node.get('resource-id') == '$1':
        print(' '.join(n.get('text') for n in node.iter('node') if n.get('text')))
        break"
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

# Pair with the seeded driver's token, typed as a driver would.
pair_app() {
    "$ADB" shell am start -n "$PKG/.MainActivity" >/dev/null
    wait_text "Pair this phone"
    sleep "$PAUSE"
    if [ "$API_PORT" != 8080 ]; then
        # The pre-filled address assumes port 8080.
        tap_tag server-url
        "$ADB" shell input keycombination 113 29   # Ctrl+A, then type over it
        "$ADB" shell input text "$APP_SERVER"
    fi
    tap_tag device-token
    sleep 1
    "$ADB" shell input text "$TOKEN"
    sleep "$PAUSE"
    # Enter triggers the field's "Done" IME action, which closes the keyboard.
    # (Escape would act as Back and close the app.)
    "$ADB" shell input keyevent 66
    sleep 1
    tap_tag pair-button
    wait_text "Ramesh Patil"
    wait_text "$REG"
}
