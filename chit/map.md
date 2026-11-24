# Map

- `src/sim.rs` — pet activity, floor and screen edges. No display code.
- `src/pack.rs` — chibi archive (`heart`, `body`, `anima`, `creature`, `pocket`). Bear girl is the builtin pack and the fallback skeleton and animations.
- `src/draw.rs` — draws the skeleton, builds face thumbnails and picks body parts using their rendered transforms.
- `src/props.rs` — parachute and hand artwork for ceiling exits.
- `src/season.rs` — local-date seasons and snowfall.
- `src/rag.rs` — suspended and free ragdoll motion, joint limits and landing pose.
- `src/i18n.rs` — `languages/*.ini` catalogs and system-language detection.
- `src/config.rs` — XDG preferences for appearance and tray order.
- `src/desktop.rs` and `ui/app.slint` — tray, quick panel, preferences and ZIP import.
- `src/icons.rs` — desktop icon-theme lookup and symbolic icon tint detection.
- `src/place.rs` and `ui/place.js` — tray popup positioning and fullscreen output tracking on KWin.
- `src/game.rs` — live chibi instances, pointer routing and swept body collisions.
- `src/app.rs` — pointer gestures and geometry for one chibi.
- `src/host_wayland.rs` — one anchored KWin layer, input limited to live chibis, shared-memory buffer.
- `src/host_x11.rs` — one override-redirect ARGB window per chibi on X11.
- `src/x11_input.rs` — EWMH/RandR fullscreen detection and X Shape input regions.
- `packaging/appimage/` — `x86_64` AppImage build and draft checks.
- `packaging/draft.sh` — updates the untagged-version `dev` draft after a dev build.
