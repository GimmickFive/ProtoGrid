#!/usr/bin/env bash
# Build the ProtoGrid Android APK.
#
# Usage:
#   ANDROID_HOME=/path/to/android-sdk ./scripts/build-android.sh [arm64-v8a|x86_64]
#
# arm64-v8a is the release ABI; x86_64 is for the Android emulator.
# Requires JDK 17, Android SDK 35 and platform 24, NDK 28.0.13004108, Go 1.27,
# Rust with the matching target and cargo-ndk.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ABI="${1:-arm64-v8a}"
case "$ABI" in
    arm64-v8a)
        GO_PLATFORM="android/arm64"
        ;;
    x86_64)
        GO_PLATFORM="android/amd64"
        ;;
    *)
        echo "usage: $0 [arm64-v8a|x86_64]" >&2
        exit 2
        ;;
esac

: "${ANDROID_HOME:?set ANDROID_HOME to the Android SDK path}"
NDK_VERSION="28.0.13004108"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"

FORK="$ROOT/forks/sing-box"
if [ ! -d "$FORK/.git" ]; then
    echo "==> cloning sing-box v1.14.4"
    git clone --depth 1 --branch v1.14.4 https://github.com/SagerNet/sing-box.git "$FORK"
    git -C "$FORK" config user.name "ProtoGrid"
    git -C "$FORK" config user.email "dev@protogrid.local"
    git -C "$FORK" am "$ROOT"/docs/patches/*.patch
fi

BUILT_ABI="$(cat "$FORK/.libbox-abi" 2>/dev/null || true)"
if [ "$BUILT_ABI" != "$ABI" ] || [ ! -f "$FORK/libbox.aar" ]; then
    echo "==> building libbox ($ABI)"
    (
        cd "$FORK"
        # Go 1.27 rejects cronet-go's `-l:libcronet.a` unless explicitly allowed.
        export CGO_LDFLAGS_ALLOW='-l:.*'
        make lib_install
        go run ./cmd/internal/build_libbox -target android -platform "$GO_PLATFORM"
    )
    echo "$ABI" > "$FORK/.libbox-abi"
fi

mkdir -p apps/protogrid-android/app/libs
cp -f "$FORK/libbox.aar" apps/protogrid-android/app/libs/libbox.aar

echo "==> building Rust JNI bridge ($ABI)"
case "$ABI" in
    arm64-v8a) RUST_TARGET="aarch64-linux-android" ;;
    x86_64) RUST_TARGET="x86_64-linux-android" ;;
esac
rustup target add "$RUST_TARGET"
cargo ndk -t "$ABI" \
    -o apps/protogrid-android/app/src/main/jniLibs \
    build -p protogrid-mobile --release

echo "==> assembling APK ($ABI)"
cd apps/protogrid-android
PROTOGRID_ABIS="$ABI" ./gradlew :app:assembleRelease

echo "APK: $(pwd)/app/build/outputs/apk/release/"
ls -lh app/build/outputs/apk/release/*.apk
