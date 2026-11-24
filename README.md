<div align="center">
  <img src="assets/icon.svg" alt="mako logo" width="80">
  <h1>mako</h1>

  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL%20v3-blue.svg" alt="License: GPL-3.0-only"></a>
</div>

---

> [!NOTE]
> **Active Development:** `mako` changes frequently and may contain bugs. Delete the config file to restore the default settings.

---

`mako` is a desktop pet for KDE Plasma. bear girl walks along the bottom edge of the screen in a three-quarter view. It is a native Wayland client on a Wayland session and a native X11 client on an X11 session, without using XWayland as a fallback. Each frame is drawn from separate parts on one skeleton, not from a sprite sheet.

---

## Features

- Bear girl and Lion girl with separately animated body parts and point-based mouse grabs.
- Multiple live chibis, ZIP character packs and a quick list in the KDE tray.
- English and Russian settings with native Wayland and X11 support.

---

<details>
<summary><h2>Controls & Shortcuts</h2></summary>

| Input | Action |
| --- | --- |
| Left button | Grab a chibi by its visible silhouette. Flick to throw and tumble or throw down for a series of diminishing bounces. |
| Tray icon | Open the quick panel. Use `+` or `−` to add or remove one copy of a chibi. |
| Gear and pin | Open Settings or keep the quick panel open when it loses focus. |
| General | Choose the language, theme, interaction mode and tray layout (List or Tiles). Toggle precipitation to show or hide falling snow. The Size slider shows the chibi's base height in logical pixels, including its Dev height proportion. The default size is 115 × 108 px. |
| Chibis | Use `+` and `−` on a tile to add or remove a chibi in the tray list. Drag a marked tile to reorder it. A red `×` opens the confirmation for uninstalling a custom chibi and removing its active copies. |
| About | View the project details and check for updates. Click the logo five times to reveal Dev. |
| Dev | Adjust chibi proportions and snowfall from 25% to 200%. Set a custom day, month and year to try a separate scene for the current session. Turn off the custom date to return to the ordinary scene. |

| Interaction mode | Pointer input |
| --- | --- |
| Main | The chibis and their props receive clicks. |
| Background | Clicks pass through the chibis and their props. |
| Auto | Clicks pass through on monitors with a fullscreen window. |

Bear girl randomly walks, sits or pauses on the upper edge of the bottom panel. Each walk lasts three to five animation cycles. She looks shocked while grabbed or flying, and a thrown chibi can send others tumbling on impact. After landing on their front or back, a knocked-down chibi lies for 1.5–3 seconds and then gets back up in 1–2 seconds. A chibi can be hit again after getting back up. She pushes herself into a seated position from her back or onto her knees from her front before standing. Her hands and legs use the same parts while she types. The laptop rests on the same edge with its screen facing her.

Chibis have a 10% chance to climb when they reach a screen edge. They grip the wall with hands and feet and pull themselves upward. When they reach the ceiling, they usually parachute down. In 5% of climbs that reach the ceiling, the chibi crawls to the center and a hand carries it down. Nearby chibis sometimes stop and take turns talking and gesturing.

Snow falls to the top of the bottom panel in the festive period (November 20–January 13) and winter period (January 14–February 28), using local dates. Both periods have slippery walls, snow piles and a shared snowman. Chibis make snowballs from the piles and can knock each other over. A hit chibi has a 50% chance of going for a snowball to throw back after getting up. Any chibi can work on the snowman for 3–5 seconds at a time, and it takes about ten minutes of active work to finish. Chibis bend to shape the base with both feet on the ground, stand upright for the middle, and raise their hands for the head. Drag the snowman to lift it and move it. A short drop keeps it intact and the chibis resume building at the new spot. A hard landing or a flick breaks it. Turning off precipitation keeps the snow piles and winter activities available.

The active chibis and snow objects are saved to `$XDG_STATE_HOME/mako/scene.json` (default `~/.local/state/mako/scene.json`). Chibis start on the floor. The piles and snowman, including its progress, keep their places until the snow season ends.

The system language follows `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG`. The system theme follows the KDE color scheme. The config stores the quick-list order, language, theme, overall scale and proportions.

### Character archives

Use **Chibis → Import** to install a ZIP. Installed archives are stored in `$XDG_DATA_HOME/mako/chibis/`, or `~/.local/share/mako/chibis/` by default. A pack requires a name in `heart`. Missing `body` or `anima` files use bear girl's skeleton or animations. A pack can add optional `creature/eyes-shock.png` and `creature/mouth-shock.png` sprites for bones named `eyes` and `mouth`.

| Path | Role |
| --- | --- |
| `heart` | `name`, and other lines are ignored for now |
| `body` | Skeleton: `canvas`, `feet`, and one `bone` line per joint |
| `anima` | Animations: `anim`, `rot`, `image`, `bob`. Add `?` to a frame name to use the bone's default image if that frame is missing. |
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
| Slint | [slint-ui/slint](https://github.com/slint-ui/slint) | `GPL-3.0-only` | Tray panel and preferences. |
| Breeze | [KDE/breeze-icons](https://github.com/KDE/breeze-icons) | `LGPL-3.0-or-later` | Tray panel controls. |
| shinpi | [yrmuq/shinpi](https://github.com/yrmuq/shinpi) | `GPL-3.0-only` | Tray and preferences layout. |
| `ksni` | [iovxw/ksni](https://github.com/iovxw/ksni) | `Unlicense` | StatusNotifier tray icon. |
| `smithay-client-toolkit` | [Smithay/client-toolkit](https://github.com/Smithay/client-toolkit) | `MIT` | Wayland layer-shell host. |
| `x11rb` | [psychon/x11rb](https://github.com/psychon/x11rb) | `MIT OR Apache-2.0` | X11 host. |
| `tiny-skia` | [linebender/tiny-skia](https://github.com/linebender/tiny-skia) | `BSD-3-Clause` | CPU drawing. |
