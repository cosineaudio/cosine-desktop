# Development

## Requirements

- A Linux graphical session and a running cosine server.
- Stable Rust with Cargo.
- Node.js 22 or later and npm, used by the Tauri build CLI only.
- GTK 3, WebKitGTK 4.1, D-Bus, and the native build dependencies below.

The player is served by the server. There is no second Vue build or local API
process in this repository.

## Linux dependencies

Arch / EndeavourOS:

```bash
sudo pacman -S --needed base-devel rust nodejs npm webkit2gtk-4.1 gtk3 librsvg dbus gst-plugins-base gst-plugins-good gst-libav
```

Debian / Ubuntu, with Rust and Node.js installed separately:

```bash
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libdbus-1-dev patchelf gstreamer1.0-plugins-good gstreamer1.0-libav
```

Other distributions: follow [Tauri's Linux prerequisites](https://v2.tauri.app/start/prerequisites/)
and install the distribution's GStreamer audio codecs. Codec support depends
on those system packages.

## Run and build

```bash
npm ci
npm run dev
```

Build an optimized binary:

```bash
npm run build:binary
./scripts/install-local.sh
```

The binary is `src-tauri/target/release/cosine-desktop`. The installer copies it
into `~/.local/bin` and installs a desktop entry, scalable SVG, PNG icons from
16 to 512 pixels, and license notices. It leaves settings and login profiles
in place. Restart the app after installing an updated build.

Build `.deb` and `.rpm` packages:

```bash
npm run build
```

Packages are written to `src-tauri/target/release/bundle/deb/` and `rpm/`.
The GitHub Actions workflow builds on Ubuntu 22.04 and uploads both packages
as workflow artifacts. It does not publish releases.

Build on the oldest distribution you intend to support. A binary built on
rolling Arch can require a newer glibc than an older Ubuntu or Fedora system.
See [Tauri's Linux distribution notes](https://v2.tauri.app/distribute/appimage/)
for the shared system-library constraints; this project packages native DEB/RPM
files rather than an AppImage.

## Checks

```bash
npm test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

The automated checks cover server URL validation, settings, the connection
probe, and the MediaSession bridge. They do not require server credentials.

The optional graphical smoke test opens temporary windows and plays silent
audio using a local fixture server and an isolated profile. It requires
`xdotool`, ImageMagick, `gdbus`, X11/XWayland, and a graphical session:

```bash
cargo build --manifest-path src-tauri/Cargo.toml
dbus-run-session -- python3 tests/smoke.py
```

## Icons and branding

`assets/cosine.svg` is the unmodified logo from the main cosine repository.
It is the source for the bundled connection-screen logo and Linux app icons.
After updating the vector, regenerate the committed PNGs with:

```bash
npm run icons
```

This requires `rsvg-convert` (`librsvg` on Arch or `librsvg2-bin` on
Debian/Ubuntu). Regular builds use the committed icons and do not need it.
The 512-pixel image is first in the Tauri icon list so native windows receive
a sharp icon on high-density displays.

Use the main app's warm black, white accent, 4-pixel control radius, Outfit
headings, and Inter body text for desktop-specific screens. Keep the player
itself served by the main app.

## Performance measurements

```bash
./scripts/measure.py --seconds 20 -- ./src-tauri/target/release/cosine-desktop
```

Start with no running instance. The script launches the app, samples the
whole process tree, reports RSS/PSS and CPU, and terminates the instance it
started. First-window timing is available for X11/XWayland and does not measure
when the server's page becomes interactive. Pure Wayland runs report no
first-window estimate.

Measure idle, playback, and minimized playback separately on the same server,
page, codec, and display setup. RSS double-counts shared pages; use PSS when
available. Startup CPU is included in the overall average. Save local reports
and screenshots under `artifacts/`, which is ignored by version control.
