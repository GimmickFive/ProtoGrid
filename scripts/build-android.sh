#!/usr/bin/env bash
# Build the ProtoGrid Android APKs: the standard core and the Lite core.
#
# Usage:
#   ANDROID_HOME=/path/to/android-sdk ./scripts/build-android.sh [arm64-v8a|x86_64]
#
# arm64-v8a is the release ABI; x86_64 is for the Android emulator.
# Requires JDK 17, Android SDK 37 (Build Tools 36.0.0) and platform 24,
# NDK 28.0.13004108, Go 1.27, Rust with the matching target and cargo-ndk.
#
# Two libbox AARs are produced from the same patched sing-box tree:
#   app/libs/libbox.aar       -> :app:assembleStandardRelease
#   app/libs/libbox-lite.aar  -> :app:assembleLiteRelease
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ABI="${1:-arm64-v8a}"
case "$ABI" in
    arm64-v8a)
        GO_PLATFORM="android/arm64"
        RUST_TARGET="aarch64-linux-android"
        ;;
    x86_64)
        GO_PLATFORM="android/amd64"
        RUST_TARGET="x86_64-linux-android"
        ;;
    *)
        echo "usage: $0 [arm64-v8a|x86_64]" >&2
        exit 2
        ;;
esac

: "${ANDROID_HOME:?set ANDROID_HOME to the Android SDK path}"
NDK_VERSION="28.0.13004108"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"

# Lite keeps only VLESS/Reality over TCP, TUN, the mixed inbound and the
# Clash API. No QUIC, OpenVPN, V2Ray transports or extra protocols.
LITE_TAGS="protogrid_lite,with_utls,with_clash_api,with_gvisor"

FORK="$ROOT/forks/sing-box"

# The `.libbox-abi` marker can lie after a manually replaced AAR. Check the
# archive itself, so an `android/amd64` build cannot be picked up for arm64.
if ! command -v jar >/dev/null 2>&1 && ! command -v unzip >/dev/null 2>&1; then
    echo "error: jar or unzip is required to inspect libbox.aar" >&2
    exit 1
fi

aar_has_abi() {
    local aar="$1" abi="$2"
    [ -f "$aar" ] || return 1
    if command -v jar >/dev/null 2>&1; then
        jar tf "$aar" 2>/dev/null | grep -x "jni/$abi/libbox.so" >/dev/null
    else
        unzip -Z1 "$aar" 2>/dev/null | grep -x "jni/$abi/libbox.so" >/dev/null
    fi
}

if [ ! -d "$FORK/.git" ]; then
    echo "==> cloning sing-box v1.14.4"
    git clone --depth 1 --branch v1.14.4 https://github.com/SagerNet/sing-box.git "$FORK"
    git -C "$FORK" config user.name "ProtoGrid"
    git -C "$FORK" config user.email "dev@protogrid.local"
    git -C "$FORK" am "$ROOT"/docs/patches/*.patch
fi

need_full=0
if [ "$(cat "$FORK/.libbox-abi" 2>/dev/null || true)" != "$ABI" ] ||
    ! aar_has_abi "$FORK/libbox.aar" "$ABI"; then
    need_full=1
fi
need_lite=0
if [ "$(cat "$FORK/.libbox-lite-abi" 2>/dev/null || true)" != "$ABI" ] ||
    ! aar_has_abi "$FORK/libbox-lite.aar" "$ABI"; then
    need_lite=1
fi

if [ "$need_full" = 1 ] || [ "$need_lite" = 1 ]; then
    echo "==> preparing libbox build environment"
    (
        cd "$FORK"
        # Go 1.27 rejects cronet-go's `-l:libcronet.a` unless explicitly allowed.
        export CGO_LDFLAGS_ALLOW='-l:.*'
        make lib_install
    )
fi

if [ "$need_full" = 1 ]; then
    echo "==> building full libbox ($ABI)"
    (
        cd "$FORK"
        export CGO_LDFLAGS_ALLOW='-l:.*'
        go run ./cmd/internal/build_libbox -target android -platform "$GO_PLATFORM" \
            -output libbox.aar
    )
    if ! aar_has_abi "$FORK/libbox.aar" "$ABI"; then
        echo "error: libbox.aar does not contain jni/$ABI/libbox.so" >&2
        exit 1
    fi
    echo "$ABI" > "$FORK/.libbox-abi"
fi

if [ "$need_lite" = 1 ]; then
    echo "==> building lite libbox ($ABI)"
    (
        cd "$FORK"
        export CGO_LDFLAGS_ALLOW='-l:.*'
        go run ./cmd/internal/build_libbox -target android -platform "$GO_PLATFORM" \
            -tags "$LITE_TAGS" -output libbox-lite.aar -legacy=false
    )
    if ! aar_has_abi "$FORK/libbox-lite.aar" "$ABI"; then
        echo "error: libbox-lite.aar does not contain jni/$ABI/libbox.so" >&2
        exit 1
    fi
    echo "$ABI" > "$FORK/.libbox-lite-abi"
fi

mkdir -p apps/protogrid-android/app/libs
cp -f "$FORK/libbox.aar" apps/protogrid-android/app/libs/libbox.aar
cp -f "$FORK/libbox-lite.aar" apps/protogrid-android/app/libs/libbox-lite.aar
aar_has_abi apps/protogrid-android/app/libs/libbox.aar "$ABI" || {
    echo "error: staged libbox.aar does not contain jni/$ABI/libbox.so" >&2
    exit 1
}
aar_has_abi apps/protogrid-android/app/libs/libbox-lite.aar "$ABI" || {
    echo "error: staged libbox-lite.aar does not contain jni/$ABI/libbox.so" >&2
    exit 1
}

echo "==> building Rust JNI bridge ($ABI)"
rustup target add "$RUST_TARGET"
cargo ndk -t "$ABI" \
    -o apps/protogrid-android/app/src/main/jniLibs \
    build -p protogrid-mobile --release

echo "==> assembling APKs ($ABI)"
cd apps/protogrid-android
PROTOGRID_ABIS="$ABI" ./gradlew :app:assembleStandardRelease :app:assembleLiteRelease

echo "APKs:"
ls -lh app/build/outputs/apk/standard/release/*.apk
ls -lh app/build/outputs/apk/lite/release/*.apk
