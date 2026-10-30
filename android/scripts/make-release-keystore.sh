#!/usr/bin/env bash
# Create the release signing key, once. Every release must be signed with
# this same key: Android only installs an update over the old app if the
# signatures match, so losing it means every driver uninstalls and pairs
# again. Keep it out of git, and back it up (with its password) somewhere
# other than this machine.
#
# Usage:  android/scripts/make-release-keystore.sh [path]
#         (default ~/.config/logistics-driver/release.jks)
# Asks for a password (at least 6 characters); key alias "driver", valid 30 years.
set -euo pipefail

KEYSTORE=${1:-$HOME/.config/logistics-driver/release.jks}
JAVA_HOME=${JAVA_HOME:-$HOME/Downloads/android-studio/jbr}
KEYTOOL="$JAVA_HOME/bin/keytool"
[ -x "$KEYTOOL" ] || KEYTOOL=keytool

if [ -e "$KEYSTORE" ]; then
    echo "$KEYSTORE already exists; not overwriting the release key."
    exit 1
fi
KEYSTORE=$(realpath -m "$KEYSTORE")
case "$KEYSTORE" in
    "$(cd "$(dirname "$0")/../.." && pwd)"/*)
        echo "Put the keystore outside the repo, not at $KEYSTORE"; exit 1 ;;
esac

read -rsp "Keystore password: " PASSWORD; echo
read -rsp "Again: " AGAIN; echo
[ "$PASSWORD" = "$AGAIN" ] || { echo "The passwords differ"; exit 1; }
[ ${#PASSWORD} -ge 6 ] || { echo "Use at least 6 characters"; exit 1; }

mkdir -p "$(dirname "$KEYSTORE")"
chmod 700 "$(dirname "$KEYSTORE")"
# A PKCS12 keystore has one password for the store and the key. Passed
# through the environment, so it doesn't show in `ps`.
export PASSWORD
"$KEYTOOL" -genkeypair -keystore "$KEYSTORE" -storetype PKCS12 -alias driver \
    -keyalg RSA -keysize 4096 -validity 10950 \
    -dname "CN=Logistics Driver" \
    -storepass:env PASSWORD -keypass:env PASSWORD
chmod 600 "$KEYSTORE"

echo
echo "Created $KEYSTORE (alias driver)."
echo "Back it up now, with its password, somewhere other than this machine."
echo "Then build a release with:  DRIVER_KEYSTORE=$KEYSTORE android/scripts/build-release.sh"
