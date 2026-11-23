# Map

- `src/sim.rs` — pet activity, floor and screen edges. No display code.
- `src/pack.rs` — chibi archive (`heart`, `body`, `anima`, `creature`, `pocket`). Bear girl is the builtin pack and the fallback skeleton and animations.
- `src/draw.rs` — draws the skeleton and menu and picks body parts using their rendered transforms. Reads the bundled font.
- `src/rag.rs` — suspended and free ragdoll motion, joint limits and landing pose.
- `src/i18n.rs` — `languages/*.ini` catalogs and system-language detection.
- `src/config.rs` — `language` and `theme` in the XDG config file.
- `src/app.rs` — pointer gestures, menu actions and the frame the hosts present.
- `src/host_wayland.rs` — one anchored KWin layer, input limited to the pet, shared-memory buffer.
- `src/host_x11.rs` — override-redirect ARGB window for an X11 session.
- `packaging/appimage/` — `x86_64` AppImage build and draft checks.
- `packaging/draft.sh` — updates the untagged-version `dev` draft after a dev build.
