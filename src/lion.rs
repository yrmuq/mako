// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;

use tiny_skia::Point;

use crate::game::Instance;
use crate::sim::{Activity, Special};

const SHAKE_TIME: f32 = 1.5;
const BITE: Activity = Activity::Special(Special::LionBite);
const HELD: Activity = Activity::Special(Special::LionHeld);

struct Bite {
    lion: u64,
    victim: u64,
    direction: f32,
    time: f32,
}

#[derive(Default)]
pub struct Lions {
    nearby: HashSet<(u64, u64)>,
    bites: Vec<Bite>,
}

impl Lions {
    pub fn tick(&mut self, pets: &mut [Instance], dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.bites.retain_mut(|bite| bite.advance(pets, dt));
        if !pets.iter().any(|pet| pet.app.lion_running()) {
            self.nearby.clear();
            return;
        }
        let mut nearby = HashSet::new();
        for lion in 0..pets.len() {
            if !pets[lion].app.lion_running() {
                continue;
            }
            let actor = &pets[lion].app;
            let lion_id = pets[lion].id;
            let reach = (actor.height() * 0.42).clamp(31.0, 98.0);
            let direction = actor.pet.face;
            let victim = pets
                .iter()
                .enumerate()
                .filter(|(other, pet)| {
                    let dx = (pet.app.pet.x - actor.pet.x) * direction;
                    *other != lion
                        && (0.0..=reach).contains(&dx)
                        && (pet.app.pet.y - actor.pet.y).abs() <= 12.0
                        && pet.app.pet.activity.routine()
                        && pet.app.can_be_struck()
                })
                .inspect(|(_, pet)| {
                    nearby.insert((lion_id, pet.id));
                })
                .filter(|(_, pet)| !self.nearby.contains(&(lion_id, pet.id)))
                .min_by(|(_, a), (_, b)| {
                    (a.app.pet.x - actor.pet.x)
                        .abs()
                        .total_cmp(&(b.app.pet.x - actor.pet.x).abs())
                })
                .map(|(index, _)| index);
            if let Some(victim) = victim {
                if fastrand::f32() < 0.05 {
                    let bite = Bite::start(pets, lion, victim, direction);
                    self.bites.push(bite);
                }
            }
        }
        self.nearby = nearby;
    }
}

impl Bite {
    fn start(pets: &mut [Instance], lion: usize, victim: usize, direction: f32) -> Self {
        pets[lion].app.pet.start(BITE);
        pets[victim].app.lift_by_lion(-direction);
        let mut bite = Self {
            lion: pets[lion].id,
            victim: pets[victim].id,
            direction,
            time: 0.0,
        };
        bite.advance(pets, 0.0);
        bite
    }

    fn advance(&mut self, pets: &mut [Instance], dt: f32) -> bool {
        let lion = pets
            .iter()
            .position(|pet| pet.id == self.lion && pet.app.pet.activity == BITE);
        let victim = pets
            .iter()
            .position(|pet| pet.id == self.victim && pet.app.pet.activity == HELD);
        self.time += dt;
        if let (Some(lion), Some(victim)) = (lion, victim) {
            if self.time < SHAKE_TIME {
                let app = &pets[lion].app;
                let mouth = app.part_world("mouth").unwrap_or_else(|| {
                    Point::from_xy(
                        app.pet.x + self.direction * 22.0,
                        app.pet.y - app.height() * 0.43,
                    )
                });
                let shake = (self.time * 32.0).sin();
                let app = &mut pets[victim].app;
                let anchor = app
                    .part_world("torso")
                    .unwrap_or_else(|| Point::from_xy(app.pet.x, app.pet.y - app.height() * 0.4));
                let x = app.pet.x + mouth.x + self.direction * (5.0 + shake * 7.0) - anchor.x;
                app.pet.x = x.clamp(app.room.left, app.room.right);
                app.pet.y = (app.pet.y + mouth.y + (self.time * 27.0).cos() * 5.0 - anchor.y)
                    .min(app.room.floor - 15.0);
                return true;
            }
        }
        if let Some(lion) = lion {
            pets[lion].app.pet.start(Activity::Idle);
        }
        if let Some(victim) = victim {
            let (vx, vy) = if lion.is_some() {
                (1425.0, -395.0)
            } else {
                (180.0, -85.0)
            };
            pets[victim].app.blast(vx * self.direction, vy);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, config::Settings, pack::Pack, startup_room};

    #[test]
    fn a_lion_holds_for_one_and_a_half_seconds_then_throws() {
        let mut pets = [330.0, 379.0]
            .into_iter()
            .enumerate()
            .map(|(id, x)| {
                let mut app = App::new(
                    startup_room(1100.0, 650.0),
                    Settings::default(),
                    Pack::builtin(),
                );
                app.pet.x = x;
                Instance { id: id as u64, app }
            })
            .collect::<Vec<_>>();
        let mut bite = Bite::start(&mut pets, 0, 1, 1.0);
        assert!(bite.advance(&mut pets, 1.49));
        assert_eq!(pets[1].app.pet.activity, HELD);
        assert!(!bite.advance(&mut pets, 0.02));
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        assert!(pets[1].app.pet.vx >= 1400.0);
    }
}
