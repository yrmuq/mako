// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;

use tiny_skia::Rect;

use crate::game::Instance;
use crate::sim::{Activity, Special};
use crate::world::{SnowWall, MAX_SNOW_WALLS};

pub const HEIGHT: f32 = 84.0;
pub const HALF: f32 = 50.0;
pub const RUN: f32 = 88.0;
pub const ART_WIDTH: u32 = 288;

#[derive(Clone, Copy)]
pub struct Cover {
    pub bounds: Rect,
    pub window: Rect,
    slope_right: bool,
}

impl Cover {
    pub fn new(x: f32, floor: f32, slope_right: bool) -> Self {
        Self {
            bounds: Rect::from_ltrb(x - HALF, floor - HEIGHT, x + HALF, floor).unwrap(),
            slope_right,
            window: Rect::from_ltrb(x - 17.0, floor - 67.0, x + 17.0, floor - 33.0).unwrap(),
        }
    }

    pub fn solids(self) -> [Rect; 4] {
        let wall = self.bounds;
        let hole = self.window;
        [
            Rect::from_ltrb(wall.left(), wall.top(), wall.right(), hole.top()).unwrap(),
            Rect::from_ltrb(wall.left(), hole.bottom(), wall.right(), wall.bottom()).unwrap(),
            Rect::from_ltrb(wall.left(), hole.top(), hole.left(), hole.bottom()).unwrap(),
            Rect::from_ltrb(hole.right(), hole.top(), wall.right(), hole.bottom()).unwrap(),
        ]
    }

    pub fn slope_span(self, y: f32) -> Option<(f32, f32)> {
        if !(self.bounds.top()..self.bounds.bottom()).contains(&y) {
            return None;
        }
        let extent = RUN * (y - self.bounds.top()) / HEIGHT;
        Some(if self.slope_right {
            (self.bounds.right(), self.bounds.right() + extent)
        } else {
            (self.bounds.left() - extent, self.bounds.left())
        })
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        self.slope_span(y)
            .is_some_and(|(left, right)| (left..right).contains(&x))
            || self.solids().iter().any(|rect| {
                (rect.left()..rect.right()).contains(&x) && (rect.top()..rect.bottom()).contains(&y)
            })
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Ladder(f32),
    Wave(f32),
    Slide(f32),
    Scramble { progress: f32, limit: f32 },
    Slip { progress: f32, pose: Special },
}

impl Phase {
    fn pose(self) -> Special {
        match self {
            Self::Ladder(_) => Special::RampClimb,
            Self::Wave(_) => Special::RampWave,
            Self::Slide(_) => Special::RampSlide,
            Self::Scramble { .. } => Special::RampScramble,
            Self::Slip { pose, .. } => pose,
        }
    }
}

struct Ride {
    id: u64,
    centre: f32,
    phase: Phase,
}

#[derive(Default)]
pub struct Ramps {
    rides: [Option<Ride>; MAX_SNOW_WALLS],
    near: HashSet<(u64, usize, bool)>,
}

impl Ramps {
    pub fn clear(&mut self, pets: &mut [Instance]) {
        for ride in self.rides.iter().flatten() {
            if let Some(pet) = pets.iter_mut().find(|pet| {
                pet.id == ride.id && pet.app.pet.activity == Activity::Special(ride.phase.pose())
            }) {
                pet.app.pet.release(&pet.app.room, 0.0, 0.0);
            }
        }
        *self = Self::default();
    }

    pub fn tick(
        &mut self,
        pets: &mut [Instance],
        walls: &[SnowWall],
        grounded: [bool; MAX_SNOW_WALLS],
        width: f32,
        dt: f32,
    ) {
        let mut near = HashSet::new();
        for (slot, ready) in grounded.into_iter().enumerate() {
            let wall = walls.get(slot).filter(|wall| ready && wall.finished());
            if let Some(mut ride) = self.rides[slot].take() {
                if let Some(pet) = pets.iter_mut().find(|pet| pet.id == ride.id) {
                    if pet.app.pet.activity == Activity::Special(ride.phase.pose()) {
                        if let Some(wall) =
                            wall.filter(|wall| (wall.position * width - ride.centre).abs() < 1.0)
                        {
                            if ride.step(pet, wall.slope_right, dt) {
                                self.rides[slot] = Some(ride);
                            }
                        } else {
                            pet.app.pet.release(&pet.app.room, 0.0, 0.0);
                        }
                    }
                }
            }
            let Some(wall) = wall else { continue };
            let x = wall.position * width;
            let side = if wall.slope_right { 1.0 } else { -1.0 };
            for pet in pets.iter_mut() {
                let left = x - side * (HALF + 8.0);
                let right = x + side * (HALF + RUN);
                if left.min(right) < pet.app.room.left || left.max(right) > pet.app.room.right {
                    continue;
                }
                let ladder = (pet.app.pet.x - left).abs() <= 18.0;
                if (!ladder && (pet.app.pet.x - right).abs() > 18.0)
                    || pet.app.pet.activity != Activity::Walk
                {
                    continue;
                }
                let key = (pet.id, slot, ladder);
                near.insert(key);
                if self.rides[slot].is_some() || self.near.contains(&key) {
                    continue;
                }
                if fastrand::f32() < if ladder { 0.12 } else { 0.04 } {
                    let phase = if ladder {
                        Phase::Ladder(0.0)
                    } else {
                        Phase::Scramble {
                            progress: 0.0,
                            limit: (1.0 + fastrand::f32()) / 3.0,
                        }
                    };
                    pet.app.pet.start(Activity::Special(phase.pose()));
                    self.rides[slot] = Some(Ride {
                        id: pet.id,
                        centre: x,
                        phase,
                    });
                }
            }
        }
        self.near = near;
    }
}

impl Ride {
    fn step(&mut self, instance: &mut Instance, right: bool, dt: f32) -> bool {
        let pet = &mut instance.app.pet;
        let side = if right { 1.0 } else { -1.0 };
        let (offset, height, next) = match self.phase {
            Phase::Ladder(progress) => {
                let p = (progress + dt / 1.25).min(1.0);
                (
                    -HALF - 8.0 + (HALF + 8.0) * ((p - 0.8) * 5.0).max(0.0),
                    p * HEIGHT,
                    if p >= 1.0 {
                        Phase::Wave(0.0)
                    } else {
                        Phase::Ladder(p)
                    },
                )
            }
            Phase::Wave(time) => {
                let time = time + dt;
                (
                    0.0,
                    HEIGHT,
                    if time >= 0.9 {
                        Phase::Slide(0.0)
                    } else {
                        Phase::Wave(time)
                    },
                )
            }
            Phase::Slide(progress) => {
                let p = (progress + dt / 0.94).min(1.0);
                let offset = (HALF + RUN) * p;
                let height = HEIGHT * (1.0 - ((offset - HALF) / RUN).max(0.0));
                (offset, height, Phase::Slide(p))
            }
            Phase::Scramble { progress, limit } => {
                let p = (progress + dt / 1.65).min(limit);
                let next = if p >= limit {
                    Phase::Slip {
                        progress: p,
                        pose: if fastrand::bool() {
                            Special::RampSlipFront
                        } else {
                            Special::RampSlipBack
                        },
                    }
                } else {
                    Phase::Scramble { progress: p, limit }
                };
                (HALF + RUN * (1.0 - p), HEIGHT * p, next)
            }
            Phase::Slip { progress, pose } => {
                let p = (progress - dt * 1.15).max(0.0);
                (
                    HALF + RUN * (1.0 - p),
                    HEIGHT * p,
                    Phase::Slip { progress: p, pose },
                )
            }
        };
        pet.x = self.centre + side * offset;
        pet.y = instance.app.room.floor - height;
        pet.face = if matches!(next, Phase::Scramble { .. }) {
            -side
        } else {
            side
        };
        if matches!(next, Phase::Slide(1.0) | Phase::Slip { progress: 0.0, .. }) {
            pet.start(Activity::Special(Special::WallRecover));
            pet.face = -side;
            return false;
        }
        if self.phase.pose() != next.pose() {
            pet.start(Activity::Special(next.pose()));
        }
        if matches!(next, Phase::Slide(_)) {
            pet.vx = side * 150.0;
        }
        self.phase = next;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, config::Settings, pack::Pack, startup_room};

    #[test]
    fn the_ladder_and_slope_mirror_and_the_scramble_slips_before_the_crest() {
        for right in [false, true] {
            let mut pet = Instance {
                id: 1,
                app: App::new(
                    startup_room(900.0, 600.0),
                    Settings::default(),
                    Pack::builtin(),
                ),
            };
            let side = if right { 1.0 } else { -1.0 };
            let floor = pet.app.room.floor;
            let mut ride = Ride {
                id: pet.id,
                centre: 450.0,
                phase: Phase::Ladder(0.0),
            };
            assert!(ride.step(&mut pet, right, 1.25));
            assert_eq!(pet.app.pet.activity, Activity::Special(Special::RampWave));
            assert_eq!(pet.app.pet.y, floor - HEIGHT);
            assert!(ride.step(&mut pet, right, 0.9));
            assert!(ride.step(&mut pet, right, 0.6));
            assert_eq!(pet.app.pet.activity, Activity::Special(Special::RampSlide));
            assert!(side * (pet.app.pet.x - 450.0) > HALF);
            assert!(pet.app.pet.y > floor - HEIGHT && pet.app.pet.y < floor);
            assert!(!ride.step(&mut pet, right, 0.5));
            assert_eq!(pet.app.pet.y, floor);

            ride.phase = Phase::Scramble {
                progress: 0.0,
                limit: 0.51,
            };
            assert!(ride.step(&mut pet, right, 1.0));
            assert!(matches!(
                pet.app.pet.activity,
                Activity::Special(Special::RampSlipFront | Special::RampSlipBack)
            ));
            assert!(((floor - pet.app.pet.y) / HEIGHT - 0.51).abs() < 0.001);
            assert!(!ride.step(&mut pet, right, 0.8));
            assert_eq!(pet.app.pet.y, floor);
        }
    }
}
