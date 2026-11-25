#!/usr/bin/env bash
# Build the Windows release, the per-user installer and a portable zip.
# The ProtoGrid fork is embedded into the executable by build.rs, so the
# resulting artifacts are self-contained (no separate core folder is required).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
EXE="target/x86_64-pc-windows-gnu/release/protogrid.exe"
OUT="dist/ProtoGrid-${VERSION}-windows-x64"

echo "==> release build"
cargo build --release -p protogrid-desktop --target x86_64-pc-windows-gnu

echo "==> installer (NSIS)"
makensis installer/protogrid.nsi

echo "==> portable package"
rm -rf "${OUT}-portable"
mkdir -p "${OUT}-portable"
cp "$EXE" "${OUT}-portable/ProtoGrid.exe"
cp LICENSE "${OUT}-portable/LICENSE.txt"
# Presence of this marker switches the profile to <exe dir>\data.
: > "${OUT}-portable/portable.txt"
python3 - "${OUT}-portable" <<'PY'
import pathlib, sys, zipfile
src = pathlib.Path(sys.argv[1])
out = src.parent / (src.name + ".zip")
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for path in sorted(src.rglob("*")):
        if path.is_file():
            z.write(path, path.relative_to(src.parent))
print(out)
PY

echo "==> done"
ls -lh dist/ProtoGrid-${VERSION}-windows-x64-setup.exe "${OUT}-portable.zip"
