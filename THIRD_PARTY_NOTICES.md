# Third-party notices

cosine desktop is licensed under AGPL-3.0-only. Third-party components retain
their own licenses; the project license does not relicense them.

## Branding, fonts, and screenshots

The logo, design tokens, and album-library screenshot originate in
[cosine](https://github.com/cosineaudio/cosine) at commit
`a3ae50ed033162e1583eb35e23e6c722c3d2168f`. The SVG logo is copied unmodified;
the PNG app icons are rasterizations of that vector.

| Component | Source | License text |
| --- | --- | --- |
| Outfit, unmodified Latin variable font | [Outfit](https://github.com/Outfitio/Outfit-Fonts), as vendored by cosine | [SIL OFL 1.1](licenses/Outfit-OFL.txt) |
| Inter, unmodified Latin variable font | [Inter](https://github.com/rsms/inter), distributed by @fontsource-variable/inter | [SIL OFL 1.1](licenses/Inter-OFL.txt) |

Fonts are bundled locally; the connection screen does not load fonts from a CDN.
Copyright notices are preserved in the accompanying license files.

The screenshot in `assets/screenshots/` comes from the main cosine repository
and shows its web interface. Album artwork, artist photographs, and other
third-party content visible in screenshots belong to their respective rights
holders and are not covered by the project's source license.

## Dependencies and runtime software

Rust dependencies are declared in `src-tauri/Cargo.toml`, with exact versions
recorded in `src-tauri/Cargo.lock`. JavaScript build dependencies are declared
in `package.json` and locked in `package-lock.json`.

Tauri, reqwest, serde, serde_json, url, and sha2 are licensed under
MIT OR Apache-2.0; souvlaki is licensed under MIT. Their transitive dependencies
retain their own licenses. This is not a complete transitive license inventory.

System GTK, WebKitGTK, GStreamer, and their dependencies are supplied by the
Linux distribution and retain their respective licenses.
