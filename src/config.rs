// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::env;
use std::fs;
use std::path::PathBuf;

use crate::sim::SPRITE_H;
use crate::Fault;

const MIN_SCALE: f32 = 0.2;
const MAX_SCALE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguagePref {
    System,
    En,
    Ru,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePref {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    Main,
    Background,
    Auto,
}

impl InputMode {
    pub fn accepts_input(self, fullscreen: bool) -> bool {
        match self {
            Self::Main => true,
            Self::Background => false,
            Self::Auto => !fullscreen,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub language: LanguagePref,
    pub theme: ThemePref,
    pub input_mode: InputMode,
    pub precipitation: bool,
    pub snow_size: u32,
    pub snow_speed: u32,
    pub tiles: bool,
    pub chibi: String,
    pub scale: String,
    pub width: u32,
    pub height: u32,
    pub favorites: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: LanguagePref::System,
            theme: ThemePref::System,
            input_mode: InputMode::Main,
            precipitation: true,
            snow_size: 100,
            snow_speed: 100,
            tiles: false,
            chibi: "bear-girl".to_string(),
            scale: "0.5".to_string(),
            width: 80,
            height: 80,
            favorites: vec![crate::pack::BUILTIN_ID.into()],
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let Ok(text) = fs::read_to_string(path()) else {
            return Self::default();
        };
        Self::from_config(&text)
    }

    fn from_config(text: &str) -> Self {
        // Keep the original sizing defaults for existing files with missing fields.
        let mut settings = Self {
            scale: "1".into(),
            width: 100,
            height: 100,
            ..Self::default()
        };
        let mut has_favorites = false;
        for line in text.lines() {
            let line = line.trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches('"');
            match key.trim() {
                "language" => {
                    settings.language = match value {
                        "en" => LanguagePref::En,
                        "ru" => LanguagePref::Ru,
                        _ => LanguagePref::System,
                    }
                }
                "theme" => {
                    settings.theme = match value {
                        "light" => ThemePref::Light,
                        "dark" => ThemePref::Dark,
                        _ => ThemePref::System,
                    }
                }
                "input_mode" => {
                    settings.input_mode = match value {
                        "background" => InputMode::Background,
                        "auto" => InputMode::Auto,
                        _ => InputMode::Main,
                    }
                }
                "snow_size" => settings.snow_size = proportion(value),
                "snow_speed" => settings.snow_speed = proportion(value),
                "precipitation" => settings.precipitation = value != "false",
                "tiles" => settings.tiles = value == "true",
                "chibi" if crate::pack::safe_id(value) => settings.chibi = value.to_string(),
                "scale" => settings.scale = value.to_owned(),
                "width" => settings.width = proportion(value),
                "height" => settings.height = proportion(value),
                "favorites" => {
                    has_favorites = true;
                    settings.favorites.clear();
                    for id in value.split(',').filter(|id| crate::pack::safe_id(id)) {
                        if !settings.favorites.iter().any(|value| value == id) {
                            settings.favorites.push(id.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        if !has_favorites && settings.chibi != crate::pack::BUILTIN_ID {
            settings.favorites.push(settings.chibi.clone());
        }
        settings
    }

    pub fn save(&self) -> Result<(), Fault> {
        let path = path();
        let Some(parent) = path.parent() else {
            return Err(Fault::new(
                "MAKO-CFG-0001",
                "The config path has no directory",
            ));
        };
        fs::create_dir_all(parent).map_err(|error| {
            Fault::new(
                "MAKO-CFG-0001",
                format!("Could not create the config directory: {error}"),
            )
        })?;
        let body = format!(
            "language = \"{}\"\ntheme = \"{}\"\ninput_mode = \"{}\"\ntiles = {}\nchibi = \"{}\"\nscale = \"{}\"\nwidth = {}\nheight = {}\nfavorites = \"{}\"\nprecipitation = {}\nsnow_size = {}\nsnow_speed = {}\n",
            match self.language {
                LanguagePref::System => "system",
                LanguagePref::En => "en",
                LanguagePref::Ru => "ru",
            },
            match self.theme {
                ThemePref::System => "system",
                ThemePref::Light => "light",
                ThemePref::Dark => "dark",
            },
            match self.input_mode {
                InputMode::Main => "main",
                InputMode::Background => "background",
                InputMode::Auto => "auto",
            },
            self.tiles,
            self.chibi,
            self.pet_scale(),
            self.width,
            self.height,
            self.favorites.join(","),
            self.precipitation,
            self.snow_size,
            self.snow_speed,
        );
        let temporary = path.with_extension("toml.part");
        fs::write(&temporary, body).map_err(|error| {
            Fault::new(
                "MAKO-CFG-0001",
                format!("Could not write the config file: {error}"),
            )
        })?;
        fs::rename(&temporary, &path).map_err(|error| {
            Fault::new(
                "MAKO-CFG-0001",
                format!("Could not replace the config file: {error}"),
            )
        })
    }

    pub fn code(&self) -> String {
        match self.language {
            LanguagePref::System => crate::i18n::system_code().to_string(),
            LanguagePref::En => "en".to_string(),
            LanguagePref::Ru => "ru".to_string(),
        }
    }

    pub fn pet_scale(&self) -> f32 {
        self.scale
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .unwrap_or(1.0)
            .clamp(MIN_SCALE, MAX_SCALE)
    }

    pub fn scales(&self) -> (f32, f32) {
        let scale = self.pet_scale();
        (
            scale * self.width as f32 / 100.0,
            scale * self.height as f32 / 100.0,
        )
    }

    pub fn size_px(&self) -> f32 {
        crate::draw::scene_size(self.scales()).1 as f32
    }

    pub fn size_range_px(&self) -> (f32, f32) {
        let height = self.height_at_unit_scale();
        ((MIN_SCALE * height).round(), (MAX_SCALE * height).round())
    }

    pub fn set_size_px(&mut self, pixels: f32) {
        self.scale = (pixels.round() / self.height_at_unit_scale())
            .clamp(MIN_SCALE, MAX_SCALE)
            .to_string();
    }

    fn height_at_unit_scale(&self) -> f32 {
        SPRITE_H * self.height as f32 / 100.0
    }

    pub fn dark(&self) -> bool {
        match self.theme {
            ThemePref::System => system_dark(),
            ThemePref::Light => false,
            ThemePref::Dark => true,
        }
    }
}

fn proportion(value: &str) -> u32 {
    value.parse::<u32>().unwrap_or(100).clamp(25, 200)
}

fn kde_dark(text: &str) -> Option<bool> {
    if let Some(background) = ini_value(text, "Colors:Window", "BackgroundNormal") {
        let channels: Vec<_> = background
            .split(',')
            .map(|value| value.trim().parse::<u8>())
            .collect();
        if let [Ok(red), Ok(green), Ok(blue)] = channels.as_slice() {
            let brightness =
                2126 * u32::from(*red) + 7152 * u32::from(*green) + 722 * u32::from(*blue);
            return Some(brightness < 1_280_000);
        }
    }
    ini_value(text, "General", "ColorScheme")
        .map(|scheme| scheme.to_ascii_lowercase().contains("dark"))
}

fn system_dark() -> bool {
    let kde = fs::read_to_string(config_home().join("kdeglobals")).unwrap_or_default();
    kde_dark(&kde).unwrap_or_else(|| {
        env::var("GTK_THEME")
            .ok()
            .is_some_and(|theme| theme.to_ascii_lowercase().ends_with(":dark"))
    })
}

pub(crate) fn ini_value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            inside = name == section;
        } else if inside {
            if let Some((name, value)) = line.split_once('=') {
                if name.trim() == key {
                    return Some(value.trim());
                }
            }
        }
    }
    None
}

fn path() -> PathBuf {
    config_home().join("mako").join("config.toml")
}

pub(crate) fn config_home() -> PathBuf {
    if let Some(value) = env::var_os("XDG_CONFIG_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return path;
        }
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
}

/// Height of the bottom Plasma panel, so the pet stands on its top edge.
/// Zero when there is no bottom panel. X11 already uses the work area.
pub fn bottom_panel() -> f32 {
    let dir = config_home();
    let applets =
        fs::read_to_string(dir.join("plasma-org.kde.plasma.desktop-appletsrc")).unwrap_or_default();
    let shell = fs::read_to_string(dir.join("plasmashellrc")).unwrap_or_default();
    panel_inset(&applets, &shell)
}

pub fn panel_inset(applets: &str, shell: &str) -> f32 {
    let mut tallest = 0.0_f32;
    let mut found = false;
    for id in bottom_panel_ids(applets) {
        found = true;
        let header = format!("[PlasmaViews][Panel {id}][Defaults]");
        let thickness = group_value(shell, &header, "thickness")
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(44.0);
        tallest = tallest.max(thickness);
    }
    if !found {
        return 0.0;
    }
    tallest.clamp(20.0, 240.0)
}

fn bottom_panel_ids(applets: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut header = "";
    let mut plugin = "";
    let mut location = "";
    let flush = |header: &str, plugin: &str, location: &str, ids: &mut Vec<String>| {
        if plugin == "org.kde.panel" && location == "4" {
            if let Some(id) = containment_id(header) {
                ids.push(id);
            }
        }
    };
    for line in applets.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            flush(header, plugin, location, &mut ids);
            header = line;
            plugin = "";
            location = "";
            continue;
        }
        if let Some(value) = line.strip_prefix("plugin=") {
            plugin = value.trim();
        } else if let Some(value) = line.strip_prefix("location=") {
            location = value.trim();
        }
    }
    flush(header, plugin, location, &mut ids);
    ids
}

fn containment_id(header: &str) -> Option<String> {
    let rest = header.strip_prefix("[Containments][")?;
    let id = rest.strip_suffix(']')?;
    if id.is_empty() || id.contains('[') {
        return None;
    }
    Some(id.to_string())
}

fn group_value<'a>(text: &'a str, header: &str, key: &str) -> Option<&'a str> {
    let mut on = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            on = line == header;
            continue;
        }
        if on {
            if let Some((name, value)) = line.split_once('=') {
                if name.trim() == key {
                    return Some(value.trim());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size_matches_the_previous_manual_scale_and_proportions() {
        let settings = Settings::default();
        let manual = Settings::from_config("scale = \"0.5\"\nwidth = 80\nheight = 80\n");
        assert_eq!(settings.scales(), manual.scales());
        assert_eq!(crate::draw::scene_size(settings.scales()), (115, 108));
        assert_eq!(settings.size_px(), 108.0);
    }

    #[test]
    fn saved_configs_keep_their_scale_and_partial_configs_keep_the_old_defaults() {
        let settings = Settings::from_config("scale = \"0.75\"\nwidth = 90\n");
        assert_eq!(settings.pet_scale(), 0.75);
        assert_eq!((settings.width, settings.height), (90, 100));
        assert_eq!(Settings::from_config("tiles = true").scales(), (1.0, 1.0));
    }

    #[test]
    fn pixel_slider_round_trips_at_each_supported_proportion() {
        for height in 25..=200 {
            let mut settings = Settings {
                height,
                ..Settings::default()
            };
            let (min, max) = settings.size_range_px();
            for pixels in min as u32..=max as u32 {
                settings.set_size_px(pixels as f32);
                assert_eq!(settings.size_px(), pixels as f32, "height {height}%");
            }
        }
    }

    #[test]
    fn a_bottom_panel_raises_the_floor_inset() {
        let applets = "\
[Containments][2]
plugin=org.kde.panel
location=3
[Containments][7]
plugin=org.kde.panel
location=4
[Containments][7][General]
AppletOrder=1
";
        let shell = "\
[PlasmaViews][Panel 7][Defaults]
thickness=52
[PlasmaViews][Panel 2][Defaults]
thickness=80
";
        assert_eq!(panel_inset(applets, shell), 52.0);
        assert_eq!(panel_inset("", ""), 0.0);
    }
}
