#!/bin/sh
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
prefix=${PREFIX:-"$HOME/.local"}
binary="$root/src-tauri/target/release/cosine-desktop"
if [ ! -f "$binary" ]; then
  echo 'Build first with: npm run build:binary' >&2
  exit 1
fi
install -Dm755 "$binary" "$prefix/bin/cosine-desktop"
for size in 16 24 32 48 64 128; do
  install -Dm644 "$root/src-tauri/icons/${size}x${size}.png" "$prefix/share/icons/hicolor/${size}x${size}/apps/audio.cosine.desktop.png"
done
install -Dm644 "$root/src-tauri/icons/128x128@2x.png" "$prefix/share/icons/hicolor/256x256/apps/audio.cosine.desktop.png"
install -Dm644 "$root/src-tauri/icons/icon.png" "$prefix/share/icons/hicolor/512x512/apps/audio.cosine.desktop.png"
install -Dm644 "$root/assets/cosine.svg" "$prefix/share/icons/hicolor/scalable/apps/audio.cosine.desktop.svg"
install -Dm644 "$root/LICENSE" "$prefix/share/licenses/cosine-desktop/LICENSE"
install -Dm644 "$root/licenses/Outfit-OFL.txt" "$prefix/share/licenses/cosine-desktop/Outfit-OFL.txt"
install -Dm644 "$root/licenses/Inter-OFL.txt" "$prefix/share/licenses/cosine-desktop/Inter-OFL.txt"
install -Dm644 "$root/THIRD_PARTY_NOTICES.md" "$prefix/share/licenses/cosine-desktop/THIRD_PARTY_NOTICES.md"
mkdir -p "$prefix/share/applications"
desktop="$prefix/share/applications/audio.cosine.desktop.desktop"
# Desktop-entry quoting must protect paths containing spaces, quotes or percent signs.
escaped=$(printf '%s' "$prefix/bin/cosine-desktop" | sed 's/\\/\\\\/g;s/"/\\"/g;s/`/\\`/g;s/\$/\\$/g;s/%/%%/g')
cat > "$desktop" <<EOF
[Desktop Entry]
Type=Application
Name=cosine
Comment=Your music. Your server.
GenericName=Music Player
Exec="$escaped"
Icon=audio.cosine.desktop
Terminal=false
Categories=AudioVideo;Audio;Music;Player;
StartupWMClass=audio.cosine.desktop
StartupNotify=true
Keywords=cosine;Music;Audio;Player;Streaming;Subsonic;
EOF
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache --force --ignore-theme-index "$prefix/share/icons/hicolor" >/dev/null
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$prefix/share/applications"
fi
printf 'Installed %s\n' "$prefix/bin/cosine-desktop"
