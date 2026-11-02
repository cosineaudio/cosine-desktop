# Architecture

## The player

The desktop client opens the configured cosine server in a Tauri WebView.
It uses the original Vue app, CSS, fonts, audio engine, and API calls on the
server's real origin. The desktop shell does not render an iframe, rewrite
cookies, proxy streams, or maintain a separate player frontend.

This keeps the interface aligned with server updates and preserves the web
app's routing, authentication, library features, and live connections. It also
means the server must be reachable and the installed WebKitGTK must support
the browser features and codecs the web player uses.

## Connection lifecycle

On first launch, a bundled connection screen asks for an HTTP or HTTPS server
origin. Rust validates the address and checks `/auth/config` with a 10-second
timeout and a bounded response size. It then opens the player and closes the
connection screen.

Returning launches check the saved server and open the player directly, without
creating the connection WebView first. A failed check opens the connection
screen with a retryable error. There is no recurring connection polling.
The native menu can reopen connection settings while the player is running.

The single-instance plugin focuses an existing window on a second launch.
Closing the player exits the application; there is no tray or hidden playback
mode. Window dimensions are saved when quitting.

## Authentication and storage

Password login and OIDC use the existing server flow. OIDC redirects remain
inside the player window and return to the server's configured callback.
The desktop server address should match the OIDC public origin. Providers that
reject embedded browsers are not supported by a system-browser callback flow.

With the default Linux XDG paths:

| Data | Location |
| --- | --- |
| Server address and window settings | `~/.config/audio.cosine.desktop/settings.json` |
| Cookies, cache, and browser storage | `~/.local/share/audio.cosine.desktop/webview/<origin-hash>/` |

`XDG_CONFIG_HOME` and `XDG_DATA_HOME` override the respective base directories.
Different schemes, hosts, or ports receive different WebKit profiles. Desktop
settings contain no password; authenticated session cookies remain in WebKit's
on-disk profile. The server controls session expiry and revocation.

## Native media controls

An initialization script observes the player's existing MediaSession metadata,
state, and action handlers. It mirrors state to the native MPRIS service and
routes Linux media actions back to the existing handlers. Audio continues to
play through the web app and WebKit's media stack.

Position updates are coalesced to at most one native update per second. Native
metadata is updated only when it changes. The bridge does not poll. Desktop
artwork export is not currently implemented.

## Permissions

The local connection window can read and change desktop connection settings.
The remote player gets only the media-publication command, scoped to its
configured origin. Rust also checks the active window and origin on each media
update. Identity-provider pages have no desktop command permissions.

New-window HTTP/HTTPS links open in the system browser. Ordinary HTTP/HTTPS
navigation remains available within the player for OIDC. No filesystem, shell,
or general-purpose HTTP plugin is exposed to remote content.

## Resource use

The app uses system WebKitGTK rather than shipping Chromium or Node.js. No
local API server, music database, or server runtime is started by the client.
The server supplies cacheable frontend assets; WebKit retains its normal cache
and browser storage between launches.

The web app's virtualized lists and visibility-gated visualizer remain intact.
Actual CPU and memory use still depend on the open page, codec, library, GPU,
and WebKit version. Measure the complete process tree, including WebKit
subprocesses, rather than the Rust process alone.
