// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use tiny_skia::{Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::config::{PrecipitationMode, Settings};
use crate::draw::rgba_to_bgra;
use crate::weather::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Season {
    Ordinary,
    Festive,
    Winter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub day: i32,
    pub month: i32,
    pub year: i32,
}

impl Date {
    pub fn new(day: i32, month: i32, year: i32) -> Self {
        let month = month.clamp(1, 12);
        let year = year.clamp(1, 9999);
        let mut date = Self { day, month, year };
        date.day = day.clamp(1, date.days());
        date
    }

    pub fn days(self) -> i32 {
        match self.month {
            2 if self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0) => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }

    pub fn today() -> Self {
        let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
        // localtime_r writes the supplied structure only on success.
        unsafe {
            let now = libc::time(std::ptr::null_mut());
            if libc::localtime_r(&now, local.as_mut_ptr()).is_null() {
                return Self::new(1, 3, 1970);
            }
            let date = local.assume_init();
            Self::new(date.tm_mday, date.tm_mon + 1, date.tm_year + 1900)
        }
    }

    fn winter_year(self) -> Option<i32> {
        Season::winter_year(self.year, self.month, self.day)
    }
}

impl Season {
    fn for_date(month: i32, day: i32) -> Self {
        match (month, day) {
            (11, 20..=30) | (12, 1..=31) | (1, 1..=13) => Self::Festive,
            (1, 14..=31) | (2, 1..=28) => Self::Winter,
            _ => Self::Ordinary,
        }
    }

    fn winter_year(year: i32, month: i32, day: i32) -> Option<i32> {
        (Self::for_date(month, day) != Self::Ordinary).then(|| year + i32::from(month >= 11))
    }
}

pub struct Snow {
    date: Option<Date>,
    effective_date: Date,
    winter_year: Option<i32>,
    check_at: Instant,
    flakes: Vec<Flake>,
    geometry: (u32, u32),
    art: Vec<OverlayImage>,
    art_scale: (u32, u32),
    observed: Kind,
    falling: Kind,
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
        let date = Date::today();
        Self {
            date: None,
            effective_date: date,
            winter_year: date.winter_year(),
            check_at: now + Duration::from_secs(10),
            flakes: Vec::new(),
            geometry: (0, 0),
            art: Vec::new(),
            art_scale: (0, 0),
            observed: Kind::Dry,
            falling: Kind::Dry,
        }
    }

    pub fn active(&self) -> bool {
        self.winter_year.is_some()
    }

    pub fn winter_year(&self) -> Option<i32> {
        self.winter_year
    }

    pub fn effective_date(&self) -> Date {
        self.effective_date
    }

    pub fn set_date(&mut self, date: Option<Date>) {
        if self.date != date {
            self.date = date;
            self.refresh_date(Instant::now());
        }
    }

    fn refresh_date(&mut self, now: Instant) {
        self.effective_date = self.date.unwrap_or_else(Date::today);
        self.winter_year = self.effective_date.winter_year();
        self.check_at = now + Duration::from_secs(10);
    }

    pub fn set_weather(&mut self, kind: Kind) {
        self.observed = kind;
    }

    pub fn falling(&self) -> bool {
        self.falling != Kind::Dry
    }

    pub fn tick(&mut self, dt: f32, width: u32, floor: u32, scale: u32, settings: &Settings) {
        let now = Instant::now();
        if now >= self.check_at {
            self.refresh_date(now);
        }
        let desired = if width == 0 || floor == 0 {
            Kind::Dry
        } else {
            match settings.precipitation {
                PrecipitationMode::On if self.active() => Kind::Snow,
                PrecipitationMode::Auto => self.observed,
                _ => Kind::Dry,
            }
        };
        if self.falling != desired {
            self.falling = desired;
            self.flakes.clear();
            self.art_scale = (0, 0);
        }
        if desired == Kind::Dry {
            return;
        }
        let rain = desired == Kind::Rain;
        let art_scale = (scale, if rain { 100 } else { settings.snow_size });
        if self.art_scale != art_scale {
            self.art_scale = art_scale;
            self.art = (0..6)
                .map(|kind| {
                    if rain {
                        rain_art(kind, scale)
                    } else {
                        flake_art(kind, scale, settings.snow_size)
                    }
                })
                .collect();
        }
        if self.flakes.is_empty() || self.geometry != (width, floor) {
            self.geometry = (width, floor);
            let count = if rain {
                (width / 16).clamp(35, 130)
            } else {
                (width / 43).clamp(12, 48)
            };
            self.flakes = (0..count)
                .map(|_| {
                    Flake::new(
                        width,
                        -fastrand::f32() * if rain { floor as f32 } else { 135.0 } - 4.0,
                        rain,
                    )
                })
                .collect();
        }
        let dt = dt.clamp(0.0, 0.05);
        let speed = if rain {
            1.0
        } else {
            settings.snow_speed as f32 / 100.0
        };
        for flake in &mut self.flakes {
            let radius = self.art[flake.kind].height as f32 / (2.0 * scale as f32);
            flake.y += flake.speed * speed * dt;
            flake.phase += dt * 1.15;
            if rain {
                flake.x = (flake.x + flake.speed * dt * 0.16).rem_euclid(width as f32);
            }
            if flake.y + radius >= floor as f32 {
                *flake = Flake::new(width, -12.0 - fastrand::f32() * 48.0, rain);
            }
        }
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let scale = self.art_scale.0 as f32;
        self.flakes.iter().map(move |flake| {
            let image = &self.art[flake.kind];
            let drift = if self.falling == Kind::Rain {
                0.0
            } else {
                flake.phase.sin() * 9.0
            };
            let x = ((flake.x + drift) * scale - image.width as f32 * 0.5).round() as i32;
            let y = (flake.y * scale - image.height as f32 * 0.5).round() as i32;
            (x, y, image)
        })
    }
}

impl Flake {
    fn new(width: u32, y: f32, rain: bool) -> Self {
        Self {
            x: fastrand::f32() * width as f32,
            y,
            phase: fastrand::f32() * std::f32::consts::TAU,
            speed: if rain {
                490.0 + fastrand::f32() * 230.0
            } else {
                23.0 + fastrand::f32() * 27.0
            },
            kind: fastrand::usize(0..6),
        }
    }
}

fn rain_art(kind: usize, scale: u32) -> OverlayImage {
    let mut image = Pixmap::new(7 * scale, (12 + kind as u32) * scale).unwrap();
    let s = scale as f32;
    let end = image.height() as f32 - s;
    let mut path = PathBuilder::new();
    path.move_to(s, s);
    path.line_to(4.0 * s, end);
    let mut paint = Paint::default();
    paint.set_color_rgba8(133, 193, 232, 118 + kind as u8 * 12);
    image.stroke_path(
        &path.finish().unwrap(),
        &paint,
        &Stroke {
            width: (0.9 + kind as f32 * 0.12) * s,
            line_cap: tiny_skia::LineCap::Round,
            ..Stroke::default()
        },
        Transform::identity(),
        None,
    );
    image.into()
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
    fn auto_precipitation_is_separate_from_the_calendar() {
        let mut snow = Snow::new();
        snow.set_date(Some(Date::new(12, 6, 2040)));
        let mut settings = Settings {
            precipitation: PrecipitationMode::Auto,
            ..Settings::default()
        };
        snow.set_weather(Kind::Rain);
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(!snow.active());
        assert!(snow.falling());
        assert!(snow.images().next().is_some());
        snow.set_weather(Kind::Snow);
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(snow.falling());
        settings.precipitation = PrecipitationMode::Off;
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(!snow.falling());
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
        settings.precipitation = PrecipitationMode::Off;
        snow.tick(0.05, 800, 550, 1, &settings);
        assert!(snow.active());
        assert!(snow.images().next().is_none());
    }
}
#[test]
fn changing_the_calendar_clamps_days_and_recomputes_the_season_immediately() {
    assert_eq!(Date::new(31, 2, 2028), Date::new(29, 2, 2028));
    assert_eq!(Date::new(31, 2, 2100), Date::new(28, 2, 2100));
    let mut snow = Snow::new();
    snow.set_date(Some(Date::new(20, 11, 2040)));
    assert_eq!(snow.winter_year(), Some(2041));
    snow.set_date(Some(Date::new(28, 2, 2041)));
    assert_eq!(snow.winter_year(), Some(2041));
    snow.set_date(Some(Date::new(1, 3, 2041)));
    assert_eq!(snow.winter_year(), None);
    snow.set_date(None);
    assert_eq!(snow.winter_year(), Date::today().winter_year());
}
