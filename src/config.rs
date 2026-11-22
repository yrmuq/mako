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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub language: LanguagePref,
    pub theme: ThemePref,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: LanguagePref::System,
            theme: ThemePref::System,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let Ok(text) = fs::read_to_string(path()) else {
            return Self::default();
        };
        let mut settings = Self::default();
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
                _ => {}
            }
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
            "language = \"{}\"\ntheme = \"{}\"\n",
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
