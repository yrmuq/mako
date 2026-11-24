// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClipOrdering, ConnectionExt, MapState, Rectangle, Window,
};

use crate::config::InputMode;

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        _NET_CLIENT_LIST,
        _NET_CURRENT_DESKTOP,
        _NET_WM_DESKTOP,
        _NET_WM_STATE,
        _NET_WM_STATE_FULLSCREEN,
        _NET_WM_STATE_HIDDEN,
    }
}

pub struct Fullscreen {
    atoms: Atoms,
    areas: Vec<Area>,
    check_at: Instant,
}

impl Fullscreen {
    pub fn new(atoms: Atoms) -> Self {
        Self {
            atoms,
            areas: Vec::new(),
            check_at: Instant::now(),
        }
    }

    pub fn refresh(&mut self, conn: &impl Connection, root: Window, mode: InputMode) {
        if mode != InputMode::Auto {
            self.areas.clear();
            self.check_at = Instant::now();
        } else if Instant::now() >= self.check_at {
            self.check_at = Instant::now() + Duration::from_millis(200);
            if let Some(areas) = self.read(conn, root) {
                self.areas = areas;
            }
        }
    }

    pub fn accepts_at(&self, mode: InputMode, x: f32, y: f32) -> bool {
        mode.accepts_input(self.areas.iter().any(|area| {
            x >= area.left as f32
                && x < area.right as f32
                && y >= area.top as f32
                && y < area.bottom as f32
        }))
    }

    pub fn region(&self, mode: InputMode, x: i32, y: i32, w: u32, h: u32) -> InputRegion {
        match mode {
            InputMode::Main => InputRegion::Full,
            InputMode::Background => InputRegion::Restricted(Vec::new()),
            InputMode::Auto => restrict(Area::new(x, y, w, h), &self.areas),
        }
    }

    pub fn overlay_region(&self, mode: InputMode, bounds: Option<tiny_skia::Rect>) -> InputRegion {
        let Some(bounds) = bounds else {
            return InputRegion::Restricted(Vec::new());
        };
        let x = bounds.left().floor() as i32;
        let y = bounds.top().floor() as i32;
        let w = (bounds.right().ceil() as i32 - x) as u32;
        let h = (bounds.bottom().ceil() as i32 - y) as u32;
        let rects = match self.region(mode, x, y, w, h) {
            InputRegion::Full => vec![Rectangle {
                x: 0,
                y: 0,
                width: w as u16,
                height: h as u16,
            }],
            InputRegion::Restricted(rects) => rects,
        };
        InputRegion::Restricted(
            rects
                .into_iter()
                .map(|rect| Rectangle {
                    x: (i32::from(rect.x) + x) as i16,
                    y: (i32::from(rect.y) + y) as i16,
                    ..rect
                })
                .collect(),
        )
    }

    fn read(&self, conn: &impl Connection, root: Window) -> Option<Vec<Area>> {
        let atoms = &self.atoms;
        let desktop = property(
            conn,
            root,
            atoms._NET_CURRENT_DESKTOP,
            AtomEnum::CARDINAL,
            1,
        )?
        .first()
        .copied();
        let windows = property(conn, root, atoms._NET_CLIENT_LIST, AtomEnum::WINDOW, 4096)?;
        let states: Vec<_> = windows
            .iter()
            .filter_map(|&window| {
                let request = conn
                    .get_property(false, window, atoms._NET_WM_STATE, AtomEnum::ATOM, 0, 64)
                    .ok()?;
                Some((window, request))
            })
            .collect();
        let mut fullscreens = Vec::new();
        for (window, request) in states {
            let Ok(reply) = request.reply() else { continue };
            let Some(state) = reply.value32() else {
                continue;
            };
            let state: Vec<_> = state.collect();
            if !state.contains(&atoms._NET_WM_STATE_FULLSCREEN)
                || state.contains(&atoms._NET_WM_STATE_HIDDEN)
            {
                continue;
            }
            let window_desktop =
                property(conn, window, atoms._NET_WM_DESKTOP, AtomEnum::CARDINAL, 1)
                    .and_then(|values| values.first().copied());
            if let (Some(current), Some(on)) = (desktop, window_desktop) {
                if on != u32::MAX && on != current {
                    continue;
                }
            }
            if let Some(area) = window_area(conn, root, window) {
                fullscreens.push(area);
            }
        }
        if fullscreens.is_empty() {
            return Some(fullscreens);
        }
        let monitors = conn
            .randr_get_monitors(root, true)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        if let Some(monitors) = monitors.filter(|reply| !reply.monitors.is_empty()) {
            Some(
                monitors
                    .monitors
                    .into_iter()
                    .map(|monitor| {
                        Area::new(
                            monitor.x.into(),
                            monitor.y.into(),
                            monitor.width.into(),
                            monitor.height.into(),
                        )
                    })
                    .filter(|monitor| {
                        fullscreens
                            .iter()
                            .any(|window| window.intersect(*monitor).is_some())
                    })
                    .collect(),
            )
        } else {
            Some(fullscreens)
        }
    }
}

fn property(
    conn: &impl Connection,
    window: Window,
    name: Atom,
    kind: AtomEnum,
    limit: u32,
) -> Option<Vec<u32>> {
    let reply = conn
        .get_property(false, window, name, kind, 0, limit)
        .ok()?
        .reply()
        .ok()?;
    if reply.type_ == u32::from(AtomEnum::NONE) {
        return Some(Vec::new());
    }
    let values = reply.value32()?;
    Some(values.collect())
}

fn window_area(conn: &impl Connection, root: Window, window: Window) -> Option<Area> {
    let attributes = conn.get_window_attributes(window).ok()?;
    let geometry = conn.get_geometry(window).ok()?;
    let position = conn.translate_coordinates(window, root, 0, 0).ok()?;
    if attributes.reply().ok()?.map_state != MapState::VIEWABLE {
        return None;
    }
    let geometry = geometry.reply().ok()?;
    let position = position.reply().ok()?;
    Some(Area::new(
        position.dst_x.into(),
        position.dst_y.into(),
        geometry.width.into(),
        geometry.height.into(),
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputRegion {
    Full,
    Restricted(Vec<Rectangle>),
}

impl InputRegion {
    pub fn apply(&self, conn: &impl Connection, window: Window) -> Result<(), ReplyError> {
        match self {
            Self::Full => conn.shape_mask(SO::SET, SK::INPUT, window, 0, 0, x11rb::NONE)?,
            Self::Restricted(areas) => conn.shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                window,
                0,
                0,
                areas,
            )?,
        }
        .check()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Area {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl Area {
    fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + w as i32,
            bottom: y + h as i32,
        }
    }

    fn nonempty(self) -> bool {
        self.left < self.right && self.top < self.bottom
    }

    fn intersect(self, other: Self) -> Option<Self> {
        let overlap = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        overlap.nonempty().then_some(overlap)
    }
}

fn restrict(window: Area, blocked: &[Area]) -> InputRegion {
    let mut remaining = vec![window];
    for &area in blocked {
        let mut next = Vec::new();
        for region in remaining {
            if let Some(overlap) = region.intersect(area) {
                next.extend(
                    [
                        Area {
                            bottom: overlap.top,
                            ..region
                        },
                        Area {
                            top: overlap.bottom,
                            ..region
                        },
                        Area {
                            top: overlap.top,
                            bottom: overlap.bottom,
                            right: overlap.left,
                            ..region
                        },
                        Area {
                            top: overlap.top,
                            bottom: overlap.bottom,
                            left: overlap.right,
                            ..region
                        },
                    ]
                    .into_iter()
                    .filter(|area| area.nonempty()),
                );
            } else {
                next.push(region);
            }
        }
        remaining = next;
    }
    if remaining == [window] {
        return InputRegion::Full;
    }
    InputRegion::Restricted(
        remaining
            .into_iter()
            .map(|area| Rectangle {
                x: (area.left - window.left) as i16,
                y: (area.top - window.top) as i16,
                width: (area.right - area.left) as u16,
                height: (area.bottom - area.top) as u16,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_only_blocks_the_part_on_that_monitor() {
        let monitor = Area::new(1920, 0, 1920, 1080);
        assert_eq!(
            restrict(Area::new(1810, 900, 90, 90), &[monitor]),
            InputRegion::Full
        );
        assert_eq!(
            restrict(Area::new(1930, 900, 110, 90), &[monitor]),
            InputRegion::Restricted(vec![])
        );
        assert_eq!(
            restrict(Area::new(1870, 900, 100, 90), &[monitor]),
            InputRegion::Restricted(vec![Rectangle {
                x: 0,
                y: 0,
                width: 50,
                height: 90
            }])
        );
    }
}
