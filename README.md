<div align="center">
  <img src="assets/icon.svg" alt="mako logo" width="80">
  <h1>mako</h1>

  <a href="LICENSE"><img src="https://img.shields.io/badge/License-GPL%20v3-blue.svg" alt="License: GPL-3.0-only"></a>
</div>

---

> [!NOTE]
> **Active Development:** `mako` changes frequently and may contain bugs. Delete the config file to restore the default settings.

---

`mako` is a desktop pet for KDE Plasma. Bear girl walks along the bottom edge of the screen in a three-quarter view. It is a native Wayland client on a Wayland session and a native X11 client on an X11 session, without using XWayland as a fallback. Each frame is drawn from separate parts on one skeleton, not from a sprite sheet.

---

## Features

- Bear girl, Lion girl and Turtle boy, with separately animated body parts and point-based mouse grabs.
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
| General | Choose the language, theme, interaction mode and precipitation. The Size slider shows the chibi's base height in logical pixels, including its Dev height proportion. The default height is 99 px at 100% Width and Height. |
| Chibis | Use `+` and `−` on a tile to add or remove a chibi in the tray list. Drag a marked tile to reorder it. A red `×` opens the confirmation for uninstalling a custom chibi and removing its active copies. |
| About | View the project details and check for updates. Click the logo five times to reveal Dev. |
| Dev | Adjust chibi proportions and snowfall from 25% to 200%. Set a custom day, month and year to try a separate scene for the current session. Turn off the custom date to return to the ordinary scene. Trigger Santa, the UFO, the ceiling hand, a falling star or a rift. The hand moves a chibi to another spot on the same monitor. |

| Interaction mode | Pointer input |
| --- | --- |
| Main | The chibis and their props receive clicks. |
| Background | Clicks pass through the chibis and their props. |
| Auto | Clicks pass through on monitors with a fullscreen window. |

In General, **Precipitation → On** follows the winter calendar. **Off** hides falling particles. In **Auto**, choose a city from the suggestions. Mako uses that city's coordinates to check rain and snow through [Open-Meteo](https://open-meteo.com/) when the city is selected, at startup, and once an hour while running. A green check appears after the API returns weather data. The city selection stays in the local config. Seasonal objects still follow the calendar.

Bear girl randomly walks, sits or pauses on the upper edge of the bottom panel. Each walk lasts three to five animation cycles. She looks shocked while grabbed or flying, and a thrown chibi can send others tumbling on impact. Chibis bounce slightly when they hit the floor after a fall or a snowball hit. After landing on their front or back, a knocked-down chibi lies for 1.5–3 seconds and then gets back up in 1–2 seconds. A chibi becomes vulnerable again after getting back up. She pushes herself into a seated position from her back or onto her knees from her front before standing. Her hands and legs use the same parts while she types. Bear girl also stands and unfolds a phone with the screen facing her. Lion girl runs on all fours and can grab a nearby chibi, shake it for 1.5 seconds and toss it. Turtle boy flips upside down, hops on his head and rolls back onto his feet. A lying chibi has crossed-out eyes.

Chibis have a 10% chance to climb when they reach a screen edge. They grip the wall with hands and feet and pull themselves upward. When they reach the ceiling, they usually parachute down. In 5% of climbs that reach the ceiling, the chibi crawls to the center and a hand carries it down. Nearby chibis sometimes stop and take turns talking and gesturing.

Snow falls to the top of the bottom panel in the festive period (November 20–January 13) and winter period (January 14–February 28), using local dates. Both periods have slippery walls, snow piles and up to two shared snowmen. Chibis make snowballs from the piles and can knock each other over. A hit chibi has a 50% chance of going for a snowball to throw back after getting up. Any chibi can work on a snowman for 3–5 seconds at a time. Each snowman takes about ten minutes of active work to finish. Chibis bend to shape the base with both feet on the ground, stand upright for the middle, and raise their hands for the head. Drag either snowman to lift it and move it. A short drop keeps it intact and the chibis resume building at the new spot. A hard landing or a flick breaks it. Turning off precipitation keeps the snow piles and winter activities available.

Chibis build one shared snow wall in short shifts. Existing scenes with two walls keep both until they break or melt. A completed wall has a square window and a slide with a ladder. Drop a chibi on the summit to send it down the slide. A chibi that tries to climb the slope can slip and tumble down. The ladder end of the platform can hold one fir pot, gift bag or snowman. Drag a wall to move it with its load. A dropped wall and its snowman break together. The fir pot and gift bag fall intact. The original thrower runs to a completed wall within 300 logical pixels when another chibi prepares a counterthrow. A wall protects a chibi that has reached cover and blocks the returning shot, giving the chibi two more seconds to hide. A chibi standing beside the wall can be hit. Without nearby cover, the original thrower has a 50% chance to bat the snowball back. Snowballs can reach wall climbers and leave a flattened mark for five seconds on impact. A direct hit can cause one additional chibi to fall. A thrower is protected from their own snowball. Three hits within 30 seconds arm a snowball cannon that fires for 2.5 seconds.

During either snowy season, a chibi reaching a screen edge has a 5% chance to fetch a sled. A chibi passing a parked sled has a 5% chance to sit on it and wait up to ten seconds for the nearest available helper. The helper pulls the rider to one edge and then the other on the same screen. Pull the visible rope with the mouse to move the sled. Flick the rope to send the sled skidding into chibis and bouncing off the edge of its monitor. Drop a chibi near the seat to board. One gift bag, fir pot or snowman can ride on the deck with the chibi. The sled breaks after two completed chibi-led rides.

Chibis can plant one potted fir from November 20 through January 1. The seed starts to sprout on December 1. With a month/day system date it matures on December 10, gets its star on December 18 and has gifts on December 25. With a day/month date those milestones fall on December 15, 24 and 31. Drag the potted fir to move it and its gifts. During calendar growth, the chibis have a 5% chance to water it when choosing an activity. Later plantings mature with watering, which occurs half as often as snowman work and requires twice as much active time. On the gift day, one watering and one decorating visit finish a late planting and unlock gifts immediately. Gifts contain confetti and sometimes an oversized candy. Opened gifts reappear on the gift day. Chibis remove the decorations on January 14 and carry the fir offscreen on March 1.

Santa flies through in either snowy period. Three quick clicks on him drop his red bag. Drag the bag to move it and double-click it to make a present. Presents from the bag can contain confetti, a giant candy, a cartoon bomb that knocks nearby chibis away, or coal that a chibi throws. Gifts under the fir contain confetti or candy. The UFO visits year-round. Three quick clicks on it reveal Bob, who drops onto the desktop and stays available in Chibis. Add him to the quick panel with the plus button. Bob types on his wrist computer, can cloak or summon a ride, and uses a green ray pistol when caught in a scuffle or hit by a snowball. Ordinary chibis scuffle in a small cloud of dust.

A falling star reaches the panel about every 111 hours of runtime on average. Drag it to move it, or double-click for a five-second rainbow on that monitor. The rift opens about every 236 hours and pulls an active chibi through it. Santa, the UFO and the rift can each remove one active clone.

From September 1 to November 19, the On precipitation mode brings rain for 3–7 minutes about every two days of runtime. Auto follows the selected city's precipitation throughout the year. A rain episode has a 10% chance of lightning. A struck chibi flies back and briefly turns sooty. Once the same chibi gets up, it has a 10% chance of being struck again within ten seconds if the rain continues.

The active chibis, winter objects and fir are saved to `$XDG_STATE_HOME/mako/scene.json` (default `~/.local/state/mako/scene.json`). Chibis start on the floor. Snow piles, walls and snowmen keep their places and progress until the snow season ends. The sled keeps its position and completed ride count.

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
| Weather data | [Open-Meteo](https://open-meteo.com/) | CC BY 4.0 | City suggestions and current precipitation. |
