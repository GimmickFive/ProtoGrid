#!/usr/bin/env bash
# Build ProtoGrid-core binaries (sing-box fork).
#
# Usage:
#   ./scripts/build-fork.sh [output-dir]
#
# Requires Go 1.27+ for Reality ML-DSA-65 support.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FORK="$ROOT/forks/sing-box"
OUT="${1:-$ROOT/dist/fork}"
VERSION="1.14.4-protogrid.5"
TAGS="with_quic,with_utls,with_clash_api,with_gvisor,with_openvpn"
LITE_VERSION="1.14.4-protogrid.5-lite.1"
# Lite keeps only VLESS/Reality, TUN and the mixed inbound. No QUIC, OpenVPN,
# Clash extras beyond the API or V2Ray transports.
LITE_TAGS="protogrid_lite,with_utls,with_clash_api,with_gvisor"

if [ ! -d "$FORK" ]; then
  echo "fork not found: $FORK" >&2
  echo "clone SagerNet/sing-box v1.14.4 and apply docs/patches first" >&2
  exit 1
fi

mkdir -p "$OUT"
cd "$FORK"

for target in linux/amd64 linux/arm64 windows/amd64 darwin/amd64 darwin/arm64; do
  GOOS="${target%/*}"
  GOARCH="${target#*/}"
  name="sing-box-protogrid-${GOOS}-${GOARCH}"
  [ "$GOOS" = "windows" ] && name="${name}.exe"
  echo "== $GOOS/$GOARCH"
  CGO_ENABLED=0 GOOS="$GOOS" GOARCH="$GOARCH" go build \
    -tags "$TAGS" \
    -ldflags "-s -w -X github.com/sagernet/sing-box/constant.Version=$VERSION" \
    -o "$OUT/$name" ./cmd/sing-box

  lite_name="protogrid-core-lite-${GOOS}-${GOARCH}"
  [ "$GOOS" = "windows" ] && lite_name="${lite_name}.exe"
  echo "== lite $GOOS/$GOARCH"
  CGO_ENABLED=0 GOOS="$GOOS" GOARCH="$GOARCH" go build \
    -tags "$LITE_TAGS" \
    -ldflags "-s -w -X github.com/sagernet/sing-box/constant.Version=$LITE_VERSION" \
    -o "$OUT/$lite_name" ./cmd/sing-box
done

echo "built into $OUT"
