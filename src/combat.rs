// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;

use tiny_skia::{Pixmap, Point};

use crate::game::Instance;
use crate::props::{oval, paint, resize_art};
use crate::season::OverlayImage;
use crate::sim::{Activity, Special};
use crate::sky::at;

const FIGHT: Activity = Activity::Special(Special::Scuffle);
const LASER: Activity = Activity::Special(Special::BobLaser);
const SHOTS: [f32; 4] = [0.31, 0.59, 0.87, 1.15];

struct Fight {
    pair: (u64, u64),
    center: Point,
    until: f32,
    age: f32,
}

struct Volley {
    owner: u64,
    target: u64,
    fired: usize,
}

struct Bolt {
    owner: u64,
    at: Point,
    speed: Point,
    left: f32,
}

struct Impact {
    at: Point,
    left: f32,
}

struct CombatArt {
    scale: u32,
    smoke: Vec<OverlayImage>,
    bolts: Vec<OverlayImage>,
    impact: OverlayImage,
}

#[derive(Default)]
pub struct Combat {
    contacts: HashSet<(u64, u64)>,
    fights: Vec<Fight>,
    volleys: Vec<Volley>,
    bolts: Vec<Bolt>,
    impacts: Vec<Impact>,
    art: Option<CombatArt>,
}

impl Combat {
    pub fn tick(&mut self, pets: &mut [Instance], floor: f32, width: u32, scale: u32, dt: f32) {
        if width == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        self.check_fights(pets, dt);
        for i in 0..pets.len() {
            if let Some(target) = pets[i].app.take_laser_target() {
                if let Some(x) = pets
                    .iter()
                    .find(|pet| pet.id == target)
                    .map(|pet| pet.app.pet.x)
                {
                    self.open_fire(&mut pets[i], target, x);
                }
            }
        }
        self.meet(pets, floor);
        self.fire(pets);
        if (self.art.as_ref().is_none_or(|art| art.scale != scale))
            && (!self.fights.is_empty() || !self.bolts.is_empty())
        {
            self.art = Some(CombatArt {
                scale,
                smoke: (0..5).map(|frame| smoke(frame, scale)).collect(),
                bolts: (0..16).map(|angle| bolt_art(angle, scale)).collect(),
                impact: impact_art(scale),
            });
        }
        self.impacts.retain_mut(|impact| {
            impact.left -= dt;
            impact.left > 0.0
        });
        self.fly(pets, width as f32, floor, dt);
    }

    fn check_fights(&mut self, pets: &mut [Instance], dt: f32) {
        self.fights.retain_mut(|fight| {
            fight.age += dt;
            let present = |id| {
                pets.iter()
                    .any(|pet| pet.id == id && pet.app.pet.activity == FIGHT)
            };
            let complete = fight.age >= fight.until;
            if !complete && present(fight.pair.0) && present(fight.pair.1) {
                for (id, side) in [(fight.pair.0, -1.0), (fight.pair.1, 1.0)] {
                    if let Some(pet) = pets.iter_mut().find(|pet| pet.id == id) {
                        let wobble = (fight.age * 23.0).sin() * 9.0;
                        pet.app.pet.x = (fight.center.x + side * (14.0 + wobble))
                            .clamp(pet.app.room.left, pet.app.room.right);
                        pet.app.pet.face = -side;
                    }
                }
                return true;
            }
            for (id, side) in [(fight.pair.0, -1.0), (fight.pair.1, 1.0)] {
                if let Some(pet) = pets
                    .iter_mut()
                    .find(|pet| pet.id == id && pet.app.pet.activity == FIGHT)
                {
                    if complete {
                        pet.app.blast(side * 245.0, -125.0);
                    } else {
                        pet.app.pet.start(Activity::Idle);
                    }
                }
            }
            false
        });
    }

    fn meet(&mut self, pets: &mut [Instance], floor: f32) {
        let mut contacts = HashSet::new();
        for i in 0..pets.len() {
            let (before, rest) = pets.split_at_mut(i + 1);
            let a = &mut before[i];
            for b in rest {
                let distance = (a.app.pet.x - b.app.pet.x).abs();
                let margin = ((a.app.height() + b.app.height()) * 0.22).clamp(26.0, 63.0);
                if distance > margin || (a.app.pet.y - b.app.pet.y).abs() > 14.0 {
                    continue;
                }
                let pair = (a.id.min(b.id), a.id.max(b.id));
                if self.contacts.contains(&pair) {
                    contacts.insert(pair);
                    continue;
                }
                if !a.app.pet.activity.routine()
                    || !b.app.pet.activity.routine()
                    || !(a.app.pet.activity == Activity::Walk
                        || b.app.pet.activity == Activity::Walk)
                {
                    continue;
                }
                contacts.insert(pair);
                if fastrand::f32() < 0.04 {
                    self.start_encounter(a, b, floor);
                }
            }
        }
        self.contacts = contacts;
    }

    fn start_encounter(&mut self, a: &mut Instance, b: &mut Instance, floor: f32) {
        if a.app.is_bob() {
            self.open_fire(a, b.id, b.app.pet.x);
        } else if b.app.is_bob() {
            self.open_fire(b, a.id, a.app.pet.x);
        } else {
            let pair = if a.app.pet.x <= b.app.pet.x {
                (a.id, b.id)
            } else {
                (b.id, a.id)
            };
            let x = (a.app.pet.x + b.app.pet.x) * 0.5;
            let y = floor - (a.app.height() + b.app.height()) * 0.21;
            a.app.pet.start(FIGHT);
            b.app.pet.start(FIGHT);
            self.fights.push(Fight {
                pair,
                center: Point::from_xy(x, y),
                until: 1.3 + fastrand::f32() * 0.55,
                age: 0.0,
            });
        }
    }

    fn open_fire(&mut self, bob: &mut Instance, target: u64, x: f32) {
        bob.app.aim_laser(x);
        self.volleys.retain(|volley| volley.owner != bob.id);
        self.volleys.push(Volley {
            owner: bob.id,
            target,
            fired: 0,
        });
    }

    fn fire(&mut self, pets: &mut [Instance]) {
        self.volleys.retain_mut(|volley| {
            let Some(owner) = pets
                .iter()
                .position(|pet| pet.id == volley.owner && pet.app.pet.activity == LASER)
            else {
                return false;
            };
            if pets[owner].app.pet.time >= 1.45 {
                pets[owner].app.pet.start(Activity::Idle);
                return false;
            }
            if SHOTS
                .get(volley.fired)
                .is_none_or(|shot| pets[owner].app.pet.time < *shot)
            {
                return true;
            }
            let target = pets
                .iter()
                .find(|pet| pet.id == volley.target)
                .and_then(|pet| pet.app.collision_bounds());
            let app = &mut pets[owner].app;
            if let Some(target) = target {
                let side = (target.left() + target.width() * 0.5) - app.pet.x;
                if side.abs() > 6.0 {
                    app.pet.face = side.signum();
                }
            }
            let hand = app.snowball_position();
            let to = target.map_or(
                Point::from_xy(hand.x + app.pet.face * 160.0, hand.y),
                |bounds| {
                    Point::from_xy(
                        bounds.left() + bounds.width() * 0.5,
                        bounds.top() + bounds.height() * 0.5,
                    )
                },
            );
            let mut dx = to.x - hand.x;
            if dx.abs() < 4.0 {
                dx = app.pet.face * 4.0;
            }
            let dy = to.y - hand.y;
            let length = dx.hypot(dy).max(1.0);
            let unit = Point::from_xy(dx / length, dy / length);
            self.bolts.push(Bolt {
                owner: volley.owner,
                at: Point::from_xy(hand.x + unit.x * 7.0, hand.y + unit.y * 7.0),
                speed: Point::from_xy(unit.x * 1300.0, unit.y * 1300.0),
                left: 0.62,
            });
            volley.fired += 1;
            true
        });
    }

    fn fly(&mut self, pets: &mut [Instance], width: f32, floor: f32, dt: f32) {
        self.bolts.retain_mut(|bolt| {
            let from = bolt.at;
            let step = Point::from_xy(bolt.speed.x * dt, bolt.speed.y * dt);
            bolt.at = Point::from_xy(from.x + step.x, from.y + step.y);
            bolt.left -= dt;
            let target = pets
                .iter_mut()
                .filter(|pet| pet.id != bolt.owner && pet.app.can_be_struck())
                .filter_map(|pet| {
                    Some((
                        segment_contact(from, step, pet.app.collision_bounds()?)?,
                        pet,
                    ))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((hit, pet)) = target {
                self.impacts.push(Impact {
                    at: Point::from_xy(from.x + step.x * hit, from.y + step.y * hit),
                    left: 0.19,
                });
                pet.app.blast(bolt.speed.x.signum() * 1080.0, -285.0);
                return false;
            }
            bolt.left > 0.0
                && (0.0..width).contains(&bolt.at.x)
                && (0.0..=floor + 30.0).contains(&bolt.at.y)
        });
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let art = self.art.as_ref();
        let smoke = self.fights.iter().filter_map(move |fight| {
            let art = art?;
            let frame = (fight.age * 22.0) as usize % art.smoke.len();
            Some(at(fight.center, &art.smoke[frame], art.scale as f32))
        });
        let bolts = self.bolts.iter().filter_map(move |bolt| {
            let art = art?;
            let raw = bolt.speed.y.atan2(bolt.speed.x) / std::f32::consts::TAU;
            let angle = ((raw * 16.0).round() as i32).rem_euclid(16) as usize;
            let center = Point::from_xy(
                bolt.at.x - bolt.speed.x * 0.011,
                bolt.at.y - bolt.speed.y * 0.011,
            );
            Some(at(center, &art.bolts[angle], art.scale as f32))
        });
        let impacts = self.impacts.iter().filter_map(move |impact| {
            let art = art?;
            Some(at(impact.at, &art.impact, art.scale as f32))
        });
        smoke.chain(bolts).chain(impacts)
    }
}

fn segment_contact(from: Point, step: Point, rect: tiny_skia::Rect) -> Option<f32> {
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (value, delta, a, b) in [
        (from.x, step.x, rect.left() - 2.5, rect.right() + 2.5),
        (from.y, step.y, rect.top() - 2.5, rect.bottom() + 2.5),
    ] {
        if delta.abs() <= f32::EPSILON {
            if value < a || value > b {
                return None;
            }
        } else {
            let start = (a - value) / delta;
            let end = (b - value) / delta;
            enter = enter.max(start.min(end));
            exit = exit.min(start.max(end));
            if enter > exit {
                return None;
            }
        }
    }
    Some(enter)
}

fn smoke(frame: usize, scale: u32) -> OverlayImage {
    let mut image = Pixmap::new(166, 122).unwrap();
    let phase = frame as f32 * std::f32::consts::TAU / 5.0;
    let puffs = [
        (64.0, 16.0, 52.0),
        (28.0, 26.0, 48.0),
        (93.0, 39.0, 45.0),
        (46.0, 51.0, 56.0),
        (78.0, 54.0, 54.0),
        (22.0, 49.0, 37.0),
    ];
    for (i, (x, y, w)) in puffs.into_iter().enumerate() {
        let sway = (phase + i as f32 * 2.0).sin() * 6.0;
        oval(
            &mut image,
            (x + sway - 2.0, y - 2.0, w + 4.0, w * 0.79 + 4.0),
            [129, 124, 158, 169],
        );
        oval(&mut image, (x + sway, y, w, w * 0.79), [236, 228, 241, 222]);
        oval(
            &mut image,
            (x + sway + 9.0, y + 5.0, w * 0.52, w * 0.32),
            [250, 245, 250, 125],
        );
    }
    for (x, y) in [(19.0, 28.0), (139.0, 38.0), (74.0, 10.0)] {
        let y = y + (phase + x).sin() * 6.0;
        paint(&mut image, [255, 218, 118, 233], Some(2.7), |p| {
            p.move_to(x, y - 5.0);
            p.line_to(x, y + 5.0);
            p.move_to(x - 5.0, y);
            p.line_to(x + 5.0, y);
        });
    }
    resize_art(image, scale).into()
}

fn bolt_art(angle: usize, scale: u32) -> OverlayImage {
    let (y, x) = (angle as f32 * std::f32::consts::TAU / 16.0).sin_cos();
    let mut image = Pixmap::new(52, 52).unwrap();
    for (width, hue) in [
        (13.0, [19, 240, 106, 67]),
        (7.0, [84, 255, 156, 205]),
        (2.3, [227, 255, 216, 248]),
    ] {
        paint(&mut image, hue, Some(width), |p| {
            p.move_to(26.0 - x * 17.0, 26.0 - y * 17.0);
            p.line_to(26.0 + x * 18.0, 26.0 + y * 18.0);
        });
    }
    resize_art(image, scale).into()
}

fn impact_art(scale: u32) -> OverlayImage {
    let mut image = Pixmap::new(56, 56).unwrap();
    oval(&mut image, (3.0, 3.0, 50.0, 50.0), [69, 255, 139, 55]);
    for (width, tint) in [(9.0, [101, 255, 172, 152]), (3.0, [227, 255, 212, 243])] {
        paint(&mut image, tint, Some(width), |p| {
            for (x, y) in [
                (12.0, 12.0),
                (8.0, 30.0),
                (17.0, 43.0),
                (35.0, 11.0),
                (46.0, 32.0),
                (39.0, 46.0),
            ] {
                p.move_to(28.0 + (x - 28.0) * 0.25, 28.0 + (y - 28.0) * 0.25);
                p.line_to(x, y);
            }
        });
    }
    resize_art(image, scale).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, config::Settings, pack, startup_room};

    #[test]
    fn bob_answers_a_scuffle_with_a_ray_and_keeps_the_punch_animation_for_other_chibis() {
        let room = startup_room(900.0, 600.0);
        let mut bear = Instance {
            id: 0,
            app: App::new(room.clone(), Settings::default(), pack::Pack::builtin()),
        };
        let mut bob = Instance {
            id: 1,
            app: App::new(room, Settings::default(), pack::bob()),
        };
        bear.app.pet.x = 535.0;
        bob.app.pet.x = 430.0;
        let mut fight = Combat::default();
        fight.start_encounter(&mut bear, &mut bob, 600.0);
        assert_eq!(bear.app.pet.activity, Activity::Idle);
        assert_eq!(bob.app.pet.activity, LASER);
        assert!(fight.fights.is_empty());
        let mut twin = Instance {
            id: 2,
            app: App::new(
                bear.app.room.clone(),
                Settings::default(),
                pack::Pack::builtin(),
            ),
        };
        let mut scuffle = Combat::default();
        scuffle.start_encounter(&mut bear, &mut twin, 600.0);
        assert_eq!(bear.app.pet.activity, FIGHT);
        assert_eq!(twin.app.pet.activity, FIGHT);
        assert!(scuffle.volleys.is_empty());
        bear.app.pet.start(Activity::Idle);
        let mut pets = [bear, bob];
        for _ in 0..17 {
            for pet in &mut pets {
                pet.app.tick(0.04, false);
            }
            fight.tick(&mut pets, 600.0, 900, 1, 0.04);
        }
        assert_eq!(pets[0].app.pet.activity, Activity::Fall);
        assert_eq!(pets[1].app.pet.activity, LASER);
    }
}
