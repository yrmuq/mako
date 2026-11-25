// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use slint::Image;

use crate::config::{config_home, ini_value};

pub struct Icon {
    pub image: Image,
    pub tint: bool,
}

#[derive(PartialEq, Eq)]
pub struct Theme {
    name: String,
    roots: Vec<PathBuf>,
}

impl Theme {
    pub fn load() -> Self {
        let config = config_home();
        let kde = fs::read_to_string(config.join("kdeglobals")).unwrap_or_default();
        let gtk = fs::read_to_string(config.join("gtk-3.0/settings.ini")).unwrap_or_default();
        let name = ini_value(&kde, "Icons", "Theme")
            .or_else(|| ini_value(&gtk, "Settings", "gtk-icon-theme-name"))
            .filter(|name| !name.is_empty())
            .unwrap_or("breeze");
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut roots = Vec::new();
        if let Some(data) = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            roots.push(data.join("icons"));
        } else if let Some(home) = &home {
            roots.push(home.join(".local/share/icons"));
        }
        if let Some(home) = &home {
            roots.push(home.join(".icons"));
        }
        let system = std::env::var_os("XDG_DATA_DIRS")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        roots.extend(
            std::env::split_paths(&system)
                .filter(|p| p.is_absolute())
                .map(|p| p.join("icons")),
        );
        Self {
            name: name.into(),
            roots,
        }
    }

    pub fn icon(&self, name: &str) -> Option<Icon> {
        let mut visited = HashSet::new();
        let path = [&*self.name, "breeze", "hicolor"]
            .into_iter()
            .find_map(|theme| self.find(theme, name, &mut visited))?;
        if path.extension()?.to_str()? == "svg" {
            let svg = fs::read(path).ok()?;
            let tint = symbolic_svg(&String::from_utf8_lossy(&svg));
            Some(Icon {
                image: Image::load_from_svg_data(&svg).ok()?,
                tint,
            })
        } else {
            Some(Icon {
                image: Image::load_from_path(&path).ok()?,
                tint: false,
            })
        }
    }

    fn find(&self, theme: &str, name: &str, visited: &mut HashSet<String>) -> Option<PathBuf> {
        if !relative(Path::new(theme)) || !visited.insert(theme.into()) {
            return None;
        }
        let paths: Vec<_> = self.roots.iter().map(|root| root.join(theme)).collect();
        let index = paths
            .iter()
            .find_map(|path| fs::read_to_string(path.join("index.theme")).ok())?;
        let mut dirs: Vec<_> = ["Directories", "ScaledDirectories"]
            .into_iter()
            .flat_map(|key| list(ini_value(&index, "Icon Theme", key)))
            .filter(|dir| relative(Path::new(dir)))
            .collect();
        dirs.sort_by_key(|dir| {
            let size = ini_value(&index, dir, "Size")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(22);
            let scale = ini_value(&index, dir, "Scale")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(1);
            (scale != 1, size.abs_diff(22))
        });
        for dir in dirs {
            for root in &paths {
                for ext in ["svg", "png"] {
                    let path = root.join(dir).join(format!("{name}.{ext}"));
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
        let mut parents = list(ini_value(&index, "Icon Theme", "Inherits"));
        parents.find_map(|parent| self.find(parent, name, visited))
    }
}

fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn list(value: Option<&str>) -> impl Iterator<Item = &str> {
    value
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn symbolic_svg(svg: &str) -> bool {
    let svg = svg.to_ascii_lowercase();
    if svg.contains("currentcolor") || svg.contains("colorscheme-") {
        return true;
    }
    let colors: HashSet<_> = [
        "fill=\"",
        "fill='",
        "fill:",
        "stroke=\"",
        "stroke='",
        "stroke:",
    ]
    .into_iter()
    .flat_map(|key| svg.split(key).skip(1))
    .map(|color| {
        color
            .trim_start()
            .split(|c: char| matches!(c, '"' | '\'' | ';' | '}') || c.is_whitespace())
            .next()
            .unwrap_or("")
    })
    .filter(|color| !matches!(*color, "" | "none" | "transparent"))
    .collect();
    colors.len() <= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themed_icon_lookup_follows_inheritance_and_preserves_multicolor_art() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path();
        for (theme, parent) in [("custom", "base"), ("base", "")] {
            fs::create_dir_all(root.join(theme).join("22/actions")).unwrap();
            fs::write(
                root.join(theme).join("index.theme"),
                format!(
                "[Icon Theme]\nDirectories=22/actions\nInherits={parent}\n[22/actions]\nSize=22\n"
            ),
            )
            .unwrap();
        }
        let pin = root.join("base/22/actions/window-pin.svg");
        fs::write(
            &pin,
            r##"<svg><path fill="#123456"/><path fill="#abcdef"/></svg>"##,
        )
        .unwrap();
        let theme = Theme {
            name: "custom".into(),
            roots: vec![root.to_owned()],
        };
        assert_eq!(
            theme.find("custom", "window-pin", &mut HashSet::new()),
            Some(pin)
        );
        assert!(!symbolic_svg(
            r##"<svg><path fill="#123456"/><path fill="#abcdef"/></svg>"##
        ));
        assert!(symbolic_svg(r#"<svg><path fill="currentColor"/></svg>"#));
    }
}
