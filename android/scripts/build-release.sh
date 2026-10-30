#!/usr/bin/env bash
# Build the signed release APK that drivers install.
#
#   1. run the unit tests and lint
#   2. build the release APK, signed with $DRIVER_KEYSTORE
#   3. check the signing certificate against android/release-cert.sha256,
#      so a release is never signed with a different key by mistake (drivers
#      couldn't install it over the old app)
#   4. copy it to android/build/release/logistics-driver-<version>.apk
#
# Bump versionCode (and versionName) in app/build.gradle.kts first: Android
# won't install an APK over one with the same or a higher versionCode.
#
# Usage:  DRIVER_KEYSTORE=~/.config/logistics-driver/release.jks android/scripts/build-release.sh
# Asks for the keystore password unless DRIVER_KEYSTORE_PASSWORD is set.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
export JAVA_HOME=${JAVA_HOME:-$HOME/Downloads/android-studio/jbr}
export PATH=$JAVA_HOME/bin:$PATH   # apksigner runs on java
BUILD_TOOLS=$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -n 1)
CERT_FILE="$ROOT/android/release-cert.sha256"
OUT="$ROOT/android/build/release"

: "${DRIVER_KEYSTORE:?Set DRIVER_KEYSTORE to the release keystore (see scripts/make-release-keystore.sh)}"
[ -f "$DRIVER_KEYSTORE" ] || { echo "No keystore at $DRIVER_KEYSTORE"; exit 1; }
export DRIVER_KEYSTORE=$(cd "$(dirname "$DRIVER_KEYSTORE")" && pwd)/$(basename "$DRIVER_KEYSTORE")
if [ -z "${DRIVER_KEYSTORE_PASSWORD:-}" ]; then
    read -rsp "Keystore password: " DRIVER_KEYSTORE_PASSWORD; echo
    export DRIVER_KEYSTORE_PASSWORD
fi

echo "== Tests, lint and the release build"
# --no-configuration-cache: the passwords must not be written to the cache.
"$ROOT/android/gradlew" -p "$ROOT/android" --console=plain -q --no-configuration-cache \
    testDebugUnitTest lintRelease assembleRelease

APK="$ROOT/android/app/build/outputs/apk/release/app-release.apk"
[ -f "$APK" ] || { echo "No signed APK at $APK"; exit 1; }

echo "== Signature"
"$BUILD_TOOLS/apksigner" verify "$APK"
CERT=$("$BUILD_TOOLS/apksigner" verify --print-certs "$APK" | sed -n 's/.*certificate SHA-256 digest: //p' | head -n 1)
if [ -f "$CERT_FILE" ]; then
    EXPECTED=$(tr -d '[:space:]' <"$CERT_FILE")
    if [ "$CERT" != "$EXPECTED" ]; then
        echo "Signed with certificate $CERT, but releases use $EXPECTED."
        echo "Drivers couldn't install this over the app they have. Use the original keystore."
        exit 1
    fi
    echo "Same signing certificate as earlier releases"
else
    echo "$CERT" >"$CERT_FILE"
    echo "First release: recorded the certificate in android/release-cert.sha256; commit it."
fi

VERSION_NAME=$("$BUILD_TOOLS/aapt2" dump badging "$APK" | sed -n "s/.*versionName='\([^']*\)'.*/\1/p")
VERSION_CODE=$("$BUILD_TOOLS/aapt2" dump badging "$APK" | sed -n "s/.*versionCode='\([^']*\)'.*/\1/p")
mkdir -p "$OUT"
DEST="$OUT/logistics-driver-$VERSION_NAME.apk"
cp "$APK" "$DEST"
echo
echo "Release:  $DEST"
echo "Version:  $VERSION_NAME (versionCode $VERSION_CODE)"
echo "SHA-256:  $(sha256sum "$DEST" | cut -d' ' -f1)"
echo "Send drivers the APK with docs/android-driver-install.md."
