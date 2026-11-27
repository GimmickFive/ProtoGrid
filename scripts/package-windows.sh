#!/usr/bin/env bash
# Build the Windows release, the per-user installer and a portable zip.
# ProtoGrid-core and wintun.dll are shipped next to the launcher, not
# embedded into it, so the launcher stays small in both packages.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
EXE="target/x86_64-pc-windows-gnu/release/protogrid.exe"
CORE="dist/fork/sing-box-protogrid-windows-amd64.exe"
LITE="dist/fork/protogrid-core-lite-windows-amd64.exe"
WINTUN="dist/fork/wintun.dll"
OUT="dist/ProtoGrid-${VERSION}-windows-x64"

for file in "$EXE" "$CORE" "$LITE" "$WINTUN"; do
    if [[ ! -f "$file" ]]; then
        echo "missing $file — build the fork first (scripts/build-fork.sh)" >&2
        exit 1
    fi
done

echo "==> release build"
cargo build --release -p protogrid-desktop --target x86_64-pc-windows-gnu

echo "==> installer (NSIS)"
makensis installer/protogrid.nsi

echo "==> portable package"
# Stage the archive first: the local portable folder may be in use by a
# running ProtoGrid.exe, and that must not break packaging.
PORTABLE="${OUT}-portable"
STAGE="dist/.portable-stage-${VERSION}"
ZIP="${OUT}-portable.zip"
rm -rf "$STAGE"
mkdir -p "$STAGE"
cp -f "$EXE" "$STAGE/ProtoGrid.exe"
cp -f "$CORE" "$STAGE/protogrid-core.exe"
cp -f "$LITE" "$STAGE/protogrid-core-lite.exe"
cp -f "$WINTUN" "$STAGE/wintun.dll"
# Optional recovery tool for stale Wintun adapters left by older builds.
[ -f reset-tun.bat ] && cp -f reset-tun.bat "$STAGE/reset-tun.bat"
[ -f scripts/reset-tun.ps1 ] && cp -f scripts/reset-tun.ps1 "$STAGE/reset-tun.ps1"
cp -f LICENSE "$STAGE/LICENSE.txt"
cp -f THIRD_PARTY_NOTICES.md "$STAGE/THIRD_PARTY_NOTICES.txt"
mkdir -p "$STAGE/licenses"
cp -f installer/licenses/*.txt "$STAGE/licenses/"
# Presence of this marker switches the profile to <exe dir>\data.
: > "$STAGE/portable.txt"
python3 - "$STAGE" "$ZIP" <<'PY'
import pathlib, sys, zipfile
src = pathlib.Path(sys.argv[1])
out = pathlib.Path(sys.argv[2])
prefix = pathlib.Path(out.stem)
# Only the launcher files go into the archive; data/ is user-specific.
files = [
    "ProtoGrid.exe",
    "protogrid-core.exe",
    "protogrid-core-lite.exe",
    "wintun.dll",
    "reset-tun.bat",
    "reset-tun.ps1",
    "LICENSE.txt",
    "THIRD_PARTY_NOTICES.txt",
    "licenses/GPL-3.0.txt",
    "licenses/wintun-GPL-2.0.txt",
    "portable.txt",
]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for name in files:
        path = src / name
        if path.is_file():
            z.write(path, prefix / name)
print(out)
PY

# Keep an existing data/ directory: it holds the portable profile and logs.
mkdir -p "$PORTABLE/licenses"
for name in ProtoGrid.exe protogrid-core.exe protogrid-core-lite.exe wintun.dll reset-tun.bat reset-tun.ps1 LICENSE.txt THIRD_PARTY_NOTICES.txt portable.txt; do
    if [ -f "$STAGE/$name" ] && ! cp -f "$STAGE/$name" "$PORTABLE/$name" 2>/dev/null; then
        echo "note: $PORTABLE/$name is in use; archive is up to date" >&2
    fi
done
for name in GPL-3.0.txt wintun-GPL-2.0.txt; do
    cp -f "$STAGE/licenses/$name" "$PORTABLE/licenses/$name" 2>/dev/null || \
        echo "note: $PORTABLE/licenses/$name is in use; archive is up to date" >&2
done
rm -rf "$STAGE"

echo "==> done"
ls -lh dist/ProtoGrid-${VERSION}-windows-x64-setup.exe "${OUT}-portable.zip"
