#!/usr/bin/env bash
# Publish an APK as the driver app's current release: copy it into the
# backend's release directory and point latest.json at it. The backend then
# serves it at /api/driver-app/download, and the app offers it as an update to
# phones with a lower versionCode (see docs/android-driver-app.md,
# "Distribution").
#
# Usage:  android/scripts/publish-apk.sh <apk> <release dir>
#
# The release dir is the backend's DRIVER_APP_DIR (default
# ./uploads/driver-app next to where the backend runs). For a deployed server,
# publish into an empty local folder and copy it onto the uploads volume:
#   android/scripts/publish-apk.sh android/build/release/logistics-driver-0.2.0.apk /tmp/driver-app
#   docker compose cp /tmp/driver-app/. api:/app/uploads/driver-app/
set -euo pipefail

APK=${1:?Usage: publish-apk.sh <apk> <release dir>}
DIR=${2:?Usage: publish-apk.sh <apk> <release dir>}
[ -f "$APK" ] || { echo "No APK at $APK"; exit 1; }
ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
BUILD_TOOLS=$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -n 1)

BADGING=$("$BUILD_TOOLS/aapt2" dump badging "$APK")
VERSION_CODE=$(sed -n "s/.*versionCode='\([0-9]*\)'.*/\1/p" <<<"$BADGING" | head -n 1)
VERSION_NAME=$(sed -n "s/.*versionName='\([^']*\)'.*/\1/p" <<<"$BADGING" | head -n 1)
[ -n "$VERSION_CODE" ] && [ -n "$VERSION_NAME" ] || { echo "Couldn't read the version from $APK"; exit 1; }
FILE="logistics-driver-$VERSION_NAME.apk"

mkdir -p "$DIR"
if [ -f "$DIR/latest.json" ]; then
    CURRENT=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['version_code'])" "$DIR/latest.json")
    if [ "$VERSION_CODE" -le "$CURRENT" ]; then
        echo "versionCode $VERSION_CODE is not above the published $CURRENT; phones wouldn't install it as an update."
        echo "Raise versionCode in android/app/build.gradle.kts."
        exit 1
    fi
fi

# The APK first, then the manifest (written aside and renamed into place), so
# latest.json never names a file that isn't fully there.
cp "$APK" "$DIR/$FILE.part" && mv "$DIR/$FILE.part" "$DIR/$FILE"
python3 - "$DIR" "$FILE" "$VERSION_CODE" "$VERSION_NAME" "$(sha256sum "$DIR/$FILE" | cut -d' ' -f1)" <<'EOF'
import json, os, sys, time
d, file, code, name, sha = sys.argv[1:]
manifest = {"version_code": int(code), "version_name": name, "file": file,
            "sha256": sha, "released_at": int(time.time())}
with open(os.path.join(d, "latest.json.part"), "w") as f:
    json.dump(manifest, f, indent=2)
os.replace(os.path.join(d, "latest.json.part"), os.path.join(d, "latest.json"))
EOF
echo "Published $VERSION_NAME (versionCode $VERSION_CODE) in $DIR"
