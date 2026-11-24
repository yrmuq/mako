// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::config::Settings;
use crate::draw::rgba_to_bgra;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Season {
    Ordinary,
    Festive,
    Winter,
}

impl Season {
    fn for_date(month: i32, day: i32) -> Self {
        match (month, day) {
            (11, 20..=30) | (12, 1..=31) | (1, 1..=13) => Self::Festive,
            (1, 14..=31) | (2, 1..=28) => Self::Winter,
            _ => Self::Ordinary,
        }
    }

    fn local() -> Option<i32> {
        let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
        // localtime_r writes the supplied structure only on success.
        unsafe {
            let now = libc::time(std::ptr::null_mut());
            if libc::localtime_r(&now, local.as_mut_ptr()).is_null() {
                return None;
            }
            let date = local.assume_init();
            Self::winter_year(date.tm_year + 1900, date.tm_mon + 1, date.tm_mday)
        }
    }

    fn winter_year(year: i32, month: i32, day: i32) -> Option<i32> {
        (Self::for_date(month, day) != Self::Ordinary).then(|| year + i32::from(month >= 11))
    }
}

pub struct Snow {
    winter_year: Option<i32>,
    check_at: Instant,
    flakes: Vec<Flake>,
    geometry: (u32, u32),
    art: Vec<OverlayImage>,
    art_scale: (u32, u32),
}

struct Flake {
    x: f32,
    y: f32,
    phase: f32,
    speed: f32,
    kind: usize,
}

pub struct OverlayImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub ink: Vec<(i16, i16, u16)>,
}

impl From<Pixmap> for OverlayImage {
    fn from(pixmap: Pixmap) -> Self {
        let mut ink = Vec::new();
        for (y, row) in pixmap
            .pixels()
            .chunks_exact(pixmap.width() as usize)
            .enumerate()
        {
            let mut start = None;
            for (x, alpha) in row
                .iter()
                .map(|pixel| pixel.alpha())
                .chain(std::iter::once(0))
                .enumerate()
            {
                if alpha != 0 {
                    start.get_or_insert(x);
                } else if let Some(from) = start.take() {
                    ink.push((from as i16, y as i16, (x - from) as u16));
                }
            }
        }
        Self {
            width: pixmap.width(),
            height: pixmap.height(),
            bgra: rgba_to_bgra(pixmap.data()),
            ink,
        }
    }
}

impl Snow {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            winter_year: Season::local(),
            check_at: now + Duration::from_secs(10),
            flakes: Vec::new(),
            geometry: (0, 0),
            art: Vec::new(),
            art_scale: (0, 0),
        }
    }

    pub fn active(&self) -> bool {
        self.winter_year.is_some()
    }

    pub fn winter_year(&self) -> Option<i32> {
        self.winter_year
    }

    pub fn tick(&mut self, dt: f32, width: u32, floor: u32, scale: u32, settings: &Settings) {
        let now = Instant::now();
        if now >= self.check_at {
            self.winter_year = Season::local();
            self.check_at = now + Duration::from_secs(10);
        }
        if !self.active() || !settings.precipitation || width == 0 || floor == 0 {
            self.flakes.clear();
            return;
        }
        let art_scale = (scale, settings.snow_size);
        if self.art_scale != art_scale {
            self.art_scale = art_scale;
            self.art = (0..6)
                .map(|kind| flake_art(kind, scale, settings.snow_size))
                .collect();
        }
        if self.flakes.is_empty() || self.geometry != (width, floor) {
            self.geometry = (width, floor);
            let count = (width / 43).clamp(12, 48);
            self.flakes = (0..count)
                .map(|_| Flake::new(width, -4.0 - fastrand::f32() * 135.0))
                .collect();
        }
        let dt = dt.clamp(0.0, 0.05);
        let speed = settings.snow_speed as f32 / 100.0;
        for flake in &mut self.flakes {
            let radius = self.art[flake.kind].height as f32 / (2.0 * scale as f32);
            flake.y += flake.speed * speed * dt;
            flake.phase += dt * 1.15;
            if flake.y + radius >= floor as f32 {
                *flake = Flake::new(width, -12.0 - fastrand::f32() * 48.0);
            }
        }
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let scale = self.art_scale.0 as f32;
        self.flakes.iter().map(move |flake| {
            let image = &self.art[flake.kind];
            let half = image.width as f32 * 0.5;
            let x = ((flake.x + flake.phase.sin() * 9.0) * scale - half).round() as i32;
            let y = (flake.y * scale - half).round() as i32;
            (x, y, image)
        })
    }
}

impl Flake {
    fn new(width: u32, y: f32) -> Self {
        Self {
            x: fastrand::f32() * width as f32,
            y,
            phase: fastrand::f32() * std::f32::consts::TAU,
            speed: 23.0 + fastrand::f32() * 27.0,
            kind: fastrand::usize(0..6),
        }
    }
}

fn flake_art(kind: usize, scale: u32, size_percent: u32) -> OverlayImage {
    let length = (4.0 + kind as f32 * 0.78) * size_percent as f32 / 100.0 * scale as f32;
    let size = length.ceil() as u32 + 2;
    let mut pixmap = Pixmap::new(size, size).expect("snowflake dimensions");
    let center = size as f32 * 0.5;
    let radius = length * 0.5;
    let branches = if kind.is_multiple_of(3) { 4 } else { 6 };
    let mut crystal = PathBuilder::new();
    for arm in 0..branches {
        let angle = std::f32::consts::TAU * arm as f32 / branches as f32 + kind as f32 * 0.18;
        let (sin, cos) = angle.sin_cos();
        let pos = |distance: f32, side: f32| {
            (
                center + cos * distance - sin * side,
                center + sin * distance + cos * side,
            )
        };
        let (x, y) = pos(radius, 0.0);
        crystal.move_to(center, center);
        crystal.line_to(x, y);
        if kind > 1 {
            let (x, y) = pos(radius * 0.58, 0.0);
            for side in [-0.33, 0.33] {
                let (tip_x, tip_y) = pos(radius * 0.84, radius * side);
                crystal.move_to(x, y);
                crystal.line_to(tip_x, tip_y);
            }
        }
    }
    let mut paint = Paint::default();
    paint.set_color_rgba8(235, 247, 255, 135 + kind as u8 * 17);
    let stroke = Stroke {
        width: (length * 0.15).max(0.65),
        ..Stroke::default()
    };
    if let Some(path) = crystal.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
    pixmap.into()
}

#[cfg(test)]
mod tests {
    use super::{Season::*, *};

    #[test]
    fn the_season_uses_local_calendar_boundaries() {
        for (month, day, season) in [
            (11, 19, Ordinary),
            (11, 20, Festive),
            (12, 31, Festive),
            (1, 1, Festive),
            (1, 13, Festive),
            (1, 14, Winter),
            (2, 28, Winter),
            (2, 29, Ordinary),
            (3, 1, Ordinary),
        ] {
            assert_eq!(super::Season::for_date(month, day), season);
        }
        assert_eq!(Season::winter_year(2026, 11, 20), Some(2027));
        assert_eq!(Season::winter_year(2027, 1, 13), Some(2027));
        assert_eq!(Season::winter_year(2027, 1, 14), Some(2027));
        assert_eq!(Season::winter_year(2027, 3, 1), None);
    }

    #[test]
    fn snowfall_controls_apply_and_flakes_restart_above_the_panel() {
        let mut snow = Snow::new();
        snow.winter_year = Some(2027);
        let mut settings = Settings::default();
        snow.tick(0.0, 800, 550, 1, &settings);
        let original = snow.art[0].width;
        let y = snow.flakes[0].y;
        let speed = snow.flakes[0].speed;
        settings.snow_size = 200;
        settings.snow_speed = 50;
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(snow.art[0].width > original);
        assert!((snow.flakes[0].y - y - speed * 0.025).abs() < 0.001);

        snow.flakes[0].y = 550.0;
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(snow.flakes[0].y < 0.0);
        settings.precipitation = false;
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(snow.active());
        assert!(snow.images().next().is_none());
    }
}
