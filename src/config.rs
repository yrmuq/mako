// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::env;
use std::fs;
use std::path::PathBuf;

use crate::Fault;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub language: LanguagePref,
    pub theme: ThemePref,
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
            chibi: "bear-girl".to_string(),
            scale: "1".to_string(),
            width: 100,
            height: 100,
            favorites: vec![crate::pack::BUILTIN_ID.into()],
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let Ok(text) = fs::read_to_string(path()) else {
            return Self::default();
        };
        let mut settings = Self::default();
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
            "language = \"{}\"\ntheme = \"{}\"\nchibi = \"{}\"\nscale = \"{}\"\nwidth = {}\nheight = {}\nfavorites = \"{}\"\n",
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
            self.chibi,
            self.pet_scale(),
            self.width,
            self.height,
            self.favorites.join(","),
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
            .clamp(0.2, 2.0)
    }

    pub fn scales(&self) -> (f32, f32) {
        let scale = self.pet_scale();
        (
            scale * self.width as f32 / 100.0,
            scale * self.height as f32 / 100.0,
        )
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

pub fn system_dark() -> bool {
    if env::var("GTK_THEME")
        .ok()
        .is_some_and(|theme| theme.to_ascii_lowercase().ends_with(":dark"))
    {
        return true;
    }
    let path = config_home().join("kdeglobals");
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("ColorScheme=") {
            return value.to_ascii_lowercase().contains("dark");
        }
    }
    false
}

fn path() -> PathBuf {
    config_home().join("mako").join("config.toml")
}

fn config_home() -> PathBuf {
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
