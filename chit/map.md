# Map

- `src/sim.rs` — pet activity, floor and screen edges. No display code.
- `src/draw.rs` — raster of bear girl and the menu. Reads the bundled font.
- `src/i18n.rs` — `languages/*.ini` catalogs and system-language detection.
- `src/config.rs` — `language` and `theme` in the XDG config file.
- `src/app.rs` — pointer gestures, menu actions and the frame the hosts present.
- `src/host_wayland.rs` — KWin layer-shell surface and shared-memory buffer.
- `src/host_x11.rs` — override-redirect ARGB window for an X11 session.
- `packaging/appimage/` — `x86_64` AppImage build and draft checks.
- `packaging/draft.sh` — updates the untagged-version `dev` draft after a dev build.
