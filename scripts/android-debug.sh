#!/usr/bin/env bash
# Fast UI iteration: build the Lite debug APK and install it next to the
# release app (package app.protogrid.client.lite.debug).
#
#   ./scripts/android-debug.sh [serial]
#
# ANDROID_HOME defaults to /root/android-sdk in the dev container.
set -euo pipefail
cd "$(dirname "$0")/.."

export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/root/android-sdk}}"
DEVICE="${1:-}"
ADB=(adb)
if [[ -n "$DEVICE" ]]; then
    ADB+=(-s "$DEVICE")
fi

(
    cd apps/protogrid-android
    ./gradlew :app:assembleLiteDebug --console=plain
)

APK="apps/protogrid-android/app/build/outputs/apk/lite/debug/app-lite-debug.apk"
test -f "$APK"
"${ADB[@]}" install -r "$APK"
"${ADB[@]}" shell am start -n \
    app.protogrid.client.lite.debug/app.protogrid.client.MainActivity >/dev/null
echo "installed: app.protogrid.client.lite.debug"
