#!/usr/bin/env bash
# Собирает статический сайт в _site: страница из site/ плюс изображения из docs/.
# Локальный просмотр: ./scripts/build-site.sh && python3 -m http.server -d _site 8000
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$root/_site"

rm -rf "$out"
mkdir -p "$out/assets"

cp -R "$root/site/." "$out/"
cp "$root"/docs/screenshots/*.png "$out/assets/"
cp "$root/docs/favicon.png" "$out/assets/icon.png"
cp "$root/docs/logo.png" "$out/assets/logo.png"
cp "$root/docs/og-brand.png" "$out/assets/og-brand.png"

# GitHub Pages не должен прогонять сайт через Jekyll.
touch "$out/.nojekyll"

printf 'Готово: %s\n' "$out"
