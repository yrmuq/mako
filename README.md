<div align="center">
  <img src="assets/icon.svg" alt="mako logo" width="80">
  <h1>mako</h1>

  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL%20v3-blue.svg" alt="License: GPL-3.0-only"></a>
</div>

---

> [!NOTE]
> **Active Development:** `mako` changes frequently and may contain bugs. Saved language, theme and chibi settings may become outdated; delete the config file to restore the defaults.

---

`mako` is a desktop pet for KDE Plasma. bear girl walks along the bottom edge of the screen in a three-quarter view. It is a native Wayland client on a Wayland session and a native X11 client on an X11 session, without using XWayland as a fallback. Each frame is drawn from separate parts on one skeleton, not from a sprite sheet.

---

## Features

- One pet, bear girl, in a brown bear onesie. Parts such as the eyes can change without a new drawing of the whole body.
- Another chibi can be chosen from a zip. No second chibi is included.
- Walking, falling, and dragging on the screen edges.
- English and Russian text, with system, light and dark menu colors.
- Separate Wayland and X11 backends for KDE Plasma.

---

## Controls & Shortcuts

<details>
<summary><h2>Controls & Shortcuts</h2></summary>

| Input | Action |
| --- | --- |
| Left button | Drag bear girl. Releasing her throws her, twice as hard as the flick. A flight ends face-down, and she slides along the floor. |
| Right button | Open or close the menu. |
| Menu rows | Choose the chibi, size, language, theme, or quit. |

Sometimes, between walks, she sits on the floor and types. Each paw stays on its own side of the keyboard. The laptop is a separate item, `pocket/it-1.png`, not part of the body. The screen faces her, so the back of the lid faces you.

The menu language follows `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` until a manual choice is saved. The menu colors follow the KDE color scheme until a manual theme is saved. The onesie itself stays brown. The chosen chibi is saved as `chibi`. The size is `1`, `0.8`, `0.6`, `0.4` or `0.2`, saved as `scale`.

Zip archives in `~/.local/share/mako/chibis/` are listed, or under `XDG_DATA_HOME` when that is an absolute path. At most six archives are shown. An archive without a `heart` name is ignored. If `body` is missing, bear girl's skeleton is used. If `anima` is missing, bear girl's animations are used.

| Path | Role |
| --- | --- |
| `heart` | `name`, and other lines are ignored for now |
| `body` | Skeleton: `canvas`, `feet`, and one `bone` line per joint |
| `anima` | Animations: `anim`, `rot`, `image`, `bob` |
| `creature/*.png` | Parts, named `head`, `left-hand`, `eyes-open` |
| `pocket/*.png` | Items, named `it-1`, `it-2` |

</details>

---

## Installation & Usage

### Architecture & Format Support

| Format | `x86_64` | `arm64` |
| --- | :---: | :---: |
| `AppImage` | ✅ | ❌ |
| `Flatpak` | ❌ | ❌ |

`Flatpak` is not built. `arm64` packages are not built.

Stable releases use `X.Y.0`. Versions with a non-zero final component, such as `X.Y.1`, are automatic dependency-update prereleases. No stable release has been published yet. The development `AppImage` is the unpublished draft named `dev` on [all releases](https://github.com/yrmuq/mako/releases). The [latest stable release](https://github.com/yrmuq/mako/releases/latest) is that future tag.

### AppImage

Requires `x86_64` Linux with glibc 2.39 or newer and KDE Plasma. A Wayland session needs `zwlr_layer_shell_v1`, which KWin 6.7 provides. An X11 session is used only when the session itself is X11.

The development draft has no update metadata and no `.zsync` file. It is not installed through [Gear Lever](https://flathub.org/apps/it.mijorus.gearlever). A stable `AppImage`, once published, embeds a GitHub Releases update source and ships a `.zsync` for delta updates. Only that stable file should be imported into Gear Lever.

### Flatpak

`Flatpak` packages are not available.

---

## License & Attribution

`mako` is licensed under [GPL-3.0-only](LICENSE). See [NOTICE](NOTICE).

| Component | Upstream | License | Integration Details |
| --- | --- | --- | --- |
| DejaVu Sans | [dejavu-fonts/dejavu-fonts](https://github.com/dejavu-fonts/dejavu-fonts) | `Bitstream-Vera` and public domain | Embedded for menu text. |
| `smithay-client-toolkit` | [Smithay/client-toolkit](https://github.com/Smithay/client-toolkit) | `MIT` | Wayland layer-shell host. |
| `x11rb` | [psychon/x11rb](https://github.com/psychon/x11rb) | `MIT OR Apache-2.0` | X11 host. |
| `tiny-skia` | [linebender/tiny-skia](https://github.com/linebender/tiny-skia) | `BSD-3-Clause` | CPU drawing. |
| `ab_glyph` | [alexheretic/ab-glyph](https://github.com/alexheretic/ab-glyph) | `Apache-2.0` | Menu text rasterization. |
