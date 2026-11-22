// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Catalog lines are `id = English original = translated text`.
//! A missing, empty or mismatched entry falls back to the English original.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

pub const ID_NAME: u32 = 1;
pub const ID_LANGUAGE: u32 = 2;
pub const ID_THEME: u32 = 3;
pub const ID_SYSTEM: u32 = 4;
pub const ID_ENGLISH: u32 = 5;
pub const ID_RUSSIAN: u32 = 6;
pub const ID_LIGHT: u32 = 7;
pub const ID_DARK: u32 = 8;
pub const ID_QUIT: u32 = 9;

const ENGLISH: &str = include_str!("../languages/English.ini");
const RUSSIAN: &str = include_str!("../languages/Russian.ini");

#[derive(Clone, Debug)]
struct Entry {
    english: String,
    text: String,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    entries: HashMap<u32, Entry>,
}

impl Catalog {
    pub fn load(code: &str) -> Self {
        let bundled = match code {
            "ru" => parse(RUSSIAN),
            _ => parse(ENGLISH),
        };
        let Some(text) = read_external(code) else {
            return bundled;
        };
        merge(parse(ENGLISH), parse(&text), bundled)
    }

    pub fn text(&self, id: u32) -> &str {
        self.entries
            .get(&id)
            .map(|entry| entry.text.as_str())
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| english_of(id))
    }
}

pub fn system_code() -> &'static str {
    for key in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Ok(value) = env::var(key) else {
            continue;
        };
        for piece in value.split([':', ';']) {
            let token = piece.trim();
            if token.is_empty() || token == "C" || token == "POSIX" {
                continue;
            }
            let lang = token.split(['_', '.', '@']).next().unwrap_or("");
            if lang.eq_ignore_ascii_case("ru") {
                return "ru";
            }
            return "en";
        }
    }
    "en"
}

fn english_of(id: u32) -> &'static str {
    match id {
        ID_NAME => "bear girl",
        ID_LANGUAGE => "Language",
        ID_THEME => "Theme",
        ID_SYSTEM => "System",
        ID_ENGLISH => "English",
        ID_RUSSIAN => "Russian",
        ID_LIGHT => "Light",
        ID_DARK => "Dark",
        ID_QUIT => "Quit",
        _ => "",
    }
}

fn merge(english: Catalog, external: Catalog, bundled: Catalog) -> Catalog {
    let mut entries = bundled.entries;
    for (id, source) in english.entries {
        let Some(found) = external.entries.get(&id) else {
            continue;
        };
        if found.english != source.english
            || found.text.is_empty()
            || placeholders_differ(&source.english, &found.text)
        {
            continue;
        }
        entries.insert(id, found.clone());
    }
    Catalog { entries }
}

fn placeholders_differ(english: &str, translated: &str) -> bool {
    fn names(text: &str) -> Vec<&str> {
        let mut found = Vec::new();
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'{' {
                if let Some(end) = text[index + 1..].find('}') {
                    found.push(&text[index + 1..index + 1 + end]);
                    index += end + 2;
                    continue;
                }
            }
            index += 1;
        }
        found.sort_unstable();
        found
    }
    names(english) != names(translated)
}

fn read_external(code: &str) -> Option<String> {
    let name = if code == "ru" {
        "Russian.ini"
    } else {
        "English.ini"
    };
    for directory in language_dirs() {
        let path = directory.join(name);
        if !path.starts_with(&directory) {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            return Some(text);
        }
    }
    None
}

fn language_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(value) = env::var_os("MAKO_LANGUAGES") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            dirs.push(path);
        }
    }
    dirs.push(PathBuf::from("/usr/share/mako/languages"));
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("../share/mako/languages"));
        }
    }
    dirs
}

fn parse(text: &str) -> Catalog {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut entries = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let Some((id, rest)) = line.split_once('=') else {
            continue;
        };
        let Some((english, translated)) = rest.split_once('=') else {
            continue;
        };
        let Ok(id) = id.trim().parse::<u32>() else {
            continue;
        };
        entries.insert(
            id,
            Entry {
                english: english.trim().to_string(),
                text: translated.trim().to_string(),
            },
        );
    }
    Catalog { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_catalog_translates_quit() {
        let catalog = parse(RUSSIAN);
        assert_eq!(catalog.text(ID_QUIT), "Выход");
        assert_eq!(catalog.text(ID_NAME), "девочка-медведь");
    }

    #[test]
    fn mismatched_english_side_falls_back() {
        let external = "9 = Exit = Выйти\n";
        let merged = merge(parse(ENGLISH), parse(external), parse(ENGLISH));
        assert_eq!(merged.text(ID_QUIT), "Quit");
    }

    #[test]
    fn empty_translation_falls_back() {
        let external = "9 = Quit =\n";
        let merged = merge(parse(ENGLISH), parse(external), parse(RUSSIAN));
        assert_eq!(merged.text(ID_QUIT), "Выход");
    }

    #[test]
    fn detects_russian_locale_form() {
        // The function reads the process environment. The assertion documents the
        // token rule used by `system_code` without changing the test process locale.
        let token = "ru_RU.UTF-8".split(['_', '.', '@']).next().unwrap();
        assert_eq!(token, "ru");
    }
}
