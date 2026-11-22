<div align="center">
  <img src="assets/icon.svg" alt="mako logo" width="80">
  <h1>mako</h1>

  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL%20v3-blue.svg" alt="License: GPL-3.0-only"></a>
</div>

---

> [!NOTE]
> **Active Development:** `mako` changes frequently and may contain bugs. Saved language and theme settings may become outdated; delete the config file to restore the defaults.

---

`mako` is a desktop pet for KDE Plasma. bear girl walks along the bottom edge of the screen. It is a native Wayland client on a Wayland session and a native X11 client on an X11 session, without using XWayland as a fallback. The picture is drawn each frame from a pose, not from a sprite sheet.

---

## Features

- One pet, bear girl, in a brown bear onesie.
- Walking, falling, and dragging on the screen edges.
- English and Russian text, with system, light and dark menu colors.
- Separate Wayland and X11 backends for KDE Plasma.

---

## Controls & Shortcuts

<details>
<summary><h2>Controls & Shortcuts</h2></summary>

| Input | Action |
| --- | --- |
| Left button | Drag bear girl. Releasing her in the air makes her fall. |
| Right button | Open or close the menu. |
| Menu rows | Choose language, theme, or quit. |

The menu language follows `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` until a manual choice is saved. The menu colors follow the KDE color scheme until a manual theme is saved. The onesie itself stays brown.

</details>

---

## Installation & Usage

### Architecture & Format Support

| Format | `x86_64` | `arm64` |
| --- | :---: | :---: |
| `AppImage` | ✅ | ❌ |
| `Flatpak` | ❌ | ❌ |

`Flatpak` is not built. `arm64` packages are not built.

Stable releases use `X.Y.0`. Versions with a non-zero final component, such as `X.Y.1`, are automatic dependency-update prereleases. No stable release has been published yet. Download the development `AppImage` from the [dev draft](https://github.com/yrmuq/mako/releases/tag/dev), or browse [all releases](https://github.com/yrmuq/mako/releases) after the first stable tag exists. The [latest stable release](https://github.com/yrmuq/mako/releases/latest) is that future tag.

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
