#!/bin/sh
# The original vector is the source of truth for every icon size.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo 'Icon generation requires rsvg-convert (librsvg on Arch, librsvg2-bin on Debian/Ubuntu).' >&2
  exit 1
fi
mkdir -p "$root/src-tauri/icons"
for size in 16 24 32 48 64 128; do
  rsvg-convert -w "$size" -h "$size" "$root/assets/cosine.svg" -o "$root/src-tauri/icons/${size}x${size}.png"
done
rsvg-convert -w 256 -h 256 "$root/assets/cosine.svg" -o "$root/src-tauri/icons/128x128@2x.png"
rsvg-convert -w 512 -h 512 "$root/assets/cosine.svg" -o "$root/src-tauri/icons/icon.png"
cp "$root/assets/cosine.svg" "$root/ui/cosine.svg"
printf 'Updated Linux icons from assets/cosine.svg\n'
