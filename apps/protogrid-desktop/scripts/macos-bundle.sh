#!/usr/bin/env bash
# Собирает ProtoGrid.app (и .dmg) из готового бинаря. Запускать НА macOS:
#   ./macos-bundle.sh <binary> <out-dir> [version]
#
# Зачем свой скрипт, а не cargo-bundle: нужен ровно один Info.plist, в котором
# объявлена схема protogrid:// (CFBundleURLTypes) — иначе macOS не знает, что
# приложение умеет открывать такие ссылки.
set -euo pipefail

BIN="${1:?usage: macos-bundle.sh <binary> <out-dir> [version]}"
OUT="${2:?usage: macos-bundle.sh <binary> <out-dir> [version]}"
VERSION="${3:-0.0.0}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="$(cd "$ROOT/../.." && pwd)"
APP="$OUT/ProtoGrid.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RES="$CONTENTS/Resources"

rm -rf "$APP"
mkdir -p "$MACOS" "$RES" "$OUT"

install -m 755 "$BIN" "$MACOS/ProtoGrid"

# Offline GeoIP/GeoSite seed. The engine copies these into the profile cache
# on first run, then refreshes them from GitHub every 7 days.
if [[ -d "$REPO/assets/rulesets" ]]; then
  mkdir -p "$RES/rulesets"
  cp -f "$REPO"/assets/rulesets/*.srs "$RES/rulesets/"
fi

# --- иконка: icon.rgba (256x256 RGBA) -> iconset -> icon.icns -----------------
if [[ -f "$ROOT/assets/icon.rgba" ]]; then
  python3 - "$ROOT/assets/icon.rgba" "$RES/icon.iconset" <<'PY'
import struct, sys, zlib, os, pathlib

src, dst = sys.argv[1], sys.argv[2]
raw = open(src, "rb").read()
size = int((len(raw) // 4) ** 0.5)
if len(raw) != size * size * 4:
    sys.exit("icon.rgba: ожидался квадратный RGBA")

def png(path, pixels, w, h):
    stride = w * 4
    body = b"".join(b"\x00" + pixels[y * stride:(y + 1) * stride] for y in range(h))
    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))
    hdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    open(path, "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", hdr)
                           + chunk(b"IDAT", zlib.compress(body, 9)) + chunk(b"IEND", b""))

def resize(px, src_n, dst_n):
    if dst_n == src_n:
        return px
    out = bytearray(dst_n * dst_n * 4)
    for y in range(dst_n):
        sy = min(src_n - 1, y * src_n // dst_n)
        for x in range(dst_n):
            sx = min(src_n - 1, x * src_n // dst_n)
            s = (sy * src_n + sx) * 4
            d = (y * dst_n + x) * 4
            out[d:d + 4] = px[s:s + 4]
    return bytes(out)

os.makedirs(dst, exist_ok=True)
for base in (16, 32, 128, 256, 512):
    for scale, suffix in ((1, ""), (2, "@2x")):
        px = resize(raw, size, base * scale)
        png(os.path.join(dst, f"icon_{base}x{base}{suffix}.png"), px, base * scale, base * scale)
print("iconset:", dst)
PY
  if command -v iconutil >/dev/null; then
    iconutil -c icns "$RES/icon.iconset" -o "$RES/icon.icns"
    rm -rf "$RES/icon.iconset"
  fi
fi

# --- Info.plist ---------------------------------------------------------------
cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>                    <string>ProtoGrid</string>
    <key>CFBundleDisplayName</key>             <string>ProtoGrid</string>
    <key>CFBundleIdentifier</key>              <string>app.protogrid.client</string>
    <key>CFBundleExecutable</key>              <string>ProtoGrid</string>
    <key>CFBundlePackageType</key>             <string>APPL</string>
    <key>CFBundleShortVersionString</key>      <string>${VERSION}</string>
    <key>CFBundleVersion</key>                 <string>${VERSION}</string>
    <key>CFBundleInfoDictionaryVersion</key>   <string>6.0</string>
    <key>CFBundleIconFile</key>                <string>icon.icns</string>
    <key>LSMinimumSystemVersion</key>          <string>11.0</string>
    <key>NSHighResolutionCapable</key>         <true/>
    <key>LSApplicationCategoryType</key>       <string>public.app-category.utilities</string>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>    <string>app.protogrid.client.link</string>
            <key>CFBundleTypeRole</key>   <string>Viewer</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>protogrid</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
PLIST

plutil -lint "$CONTENTS/Info.plist" >/dev/null

# --- подпись (ad-hoc) и карантин ---------------------------------------------
xattr -cr "$APP" 2>/dev/null || true
codesign --force --deep --sign - "$APP" >/dev/null

# --- dmg ---------------------------------------------------------------------
ARCH="$(uname -m)"
DMG="$OUT/ProtoGrid-${VERSION}-macos-${ARCH}.dmg"
if command -v hdiutil >/dev/null; then
  rm -f "$DMG"
  hdiutil create -quiet -volname "ProtoGrid" -srcfolder "$APP" -ov -format UDZO "$DMG"
fi

echo "app: $APP"
[[ -f "$DMG" ]] && echo "dmg: $DMG"
