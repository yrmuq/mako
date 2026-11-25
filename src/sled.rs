// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;

use tiny_skia::{IntRect, Point};

use crate::app::App;
use crate::game::Instance;
use crate::props;
use crate::season::OverlayImage;
use crate::sim::{Activity, Special};
use crate::world::{Sled, World};

const SEAT_HEIGHT: f32 = 20.0;
const WAIT_SECONDS: f32 = 10.0;
const PULL_SPEED: f32 = 180.0;

#[derive(Clone, Copy)]
struct Span {
    left: f32,
    right: f32,
}

impl Span {
    fn for_x(x: f32, width: f32, monitors: &[(f32, f32)]) -> Self {
        monitors
            .iter()
            .find(|&&(left, right)| (left..=right).contains(&x))
            .map_or(
                Self {
                    left: 0.0,
                    right: width,
                },
                |&(left, right)| Self { left, right },
            )
    }

    fn edge(self, side: f32) -> f32 {
        if side < 0.0 {
            self.left
        } else {
            self.right
        }
    }

    fn toward_edge(self, x: f32) -> f32 {
        if x - self.left <= self.right - x {
            -1.0
        } else {
            1.0
        }
    }

    fn walking_bounds(self, app: &App) -> (f32, f32) {
        let inset = app.edge_clearance().min((self.right - self.left) * 0.35);
        let left = (self.left + inset).max(app.room.left);
        let right = (self.right - inset).min(app.room.right);
        if left <= right {
            (left, right)
        } else {
            let centre = ((self.left + self.right) * 0.5).clamp(app.room.left, app.room.right);
            (centre, centre)
        }
    }

    fn sled_gap(self, app: &App) -> f32 {
        let (left, right) = self.walking_bounds(app);
        ((right - left) * 0.24).min(84.0)
    }
}

pub struct SledFrame<'a> {
    pub width: u32,
    pub floor: f32,
    pub scale: u32,
    pub elapsed: f32,
    pub winter: bool,
    pub monitors: &'a [(f32, f32)],
}

#[derive(Clone, Copy)]
struct Ride {
    rider: u64,
    helper: u64,
    span: Span,
    edge: f32,
    leg: Leg,
}

#[derive(Clone, Copy)]
enum Leg {
    FirstEdge,
    Turn,
    Across,
}

#[derive(Clone, Copy, Default)]
enum Journey {
    #[default]
    Parked,
    Fetch {
        walker: u64,
        span: Span,
        edge: f32,
        returning: bool,
    },
    Wait {
        rider: u64,
        helper: Option<u64>,
        seconds: f32,
    },
    Ride(Ride),
    StepOff {
        rider: u64,
        goal: f32,
    },
}

#[derive(Default)]
pub struct SledScene {
    journey: Journey,
    nearby: HashSet<u64>,
    at: Option<f32>,
    direction: f32,
    floor: f32,
    broken: Option<(f32, f32)>,
    art: Art,
}

impl SledScene {
    pub fn clear(&mut self, pets: &mut [Instance]) {
        for pet in pets {
            if matches!(
                pet.app.pet.activity,
                Activity::Special(Special::FetchSled | Special::PullSled | Special::RideSled)
            ) || (matches!(self.journey, Journey::StepOff { rider, .. } if rider == pet.id)
                && pet.app.pet.activity == Activity::Approach)
            {
                pet.app.pet.start(Activity::Idle);
                pet.app.pet.y = pet.app.room.floor;
            }
        }
        *self = Self::default();
    }

    pub fn tick(&mut self, pets: &mut [Instance], world: &mut World, frame: SledFrame<'_>) {
        if !frame.winter || frame.width == 0 {
            return;
        }
        let width = frame.width as f32;
        let dt = frame.elapsed.clamp(0.0, 0.05);
        self.floor = frame.floor;
        self.at = world.winter().sled.map(|sled| sled.position * width);
        self.art.rope_origin = None;
        if let Some((_, age)) = self.broken.as_mut() {
            *age += dt;
            if *age >= 0.45 {
                self.broken = None;
            }
        }
        let mut hand = None;
        self.journey = match self.journey {
            Journey::Parked => {
                if let Some(x) = self.at {
                    self.try_board(pets, x)
                } else if self.broken.is_none() {
                    pets.iter_mut()
                        .filter(|pet| pet.app.pet.activity.routine())
                        .find_map(|pet| {
                            let edge = if pet.app.pet.take_sled_edge()? {
                                1.0
                            } else {
                                -1.0
                            };
                            let span = Span::for_x(pet.app.pet.x, width, frame.monitors);
                            pet.app.pet.start(Activity::Special(Special::FetchSled));
                            pet.app.pet.face = edge;
                            Some(Journey::Fetch {
                                walker: pet.id,
                                span,
                                edge,
                                returning: false,
                            })
                        })
                        .unwrap_or_default()
                } else {
                    Journey::Parked
                }
            }
            Journey::Fetch {
                walker,
                span,
                edge,
                returning,
            } => {
                let action = if returning {
                    Special::PullSled
                } else {
                    Special::FetchSled
                };
                if let Some(index) = active(pets, walker, action) {
                    let app = &mut pets[index].app;
                    let boundary = span.edge(edge);
                    if returning {
                        self.direction = -edge;
                        let (left, right) = span.walking_bounds(app);
                        let goal =
                            (boundary - edge * (170.0 + span.sled_gap(app))).clamp(left, right);
                        let arrived = move_to(&mut app.pet.x, goal, PULL_SPEED * dt);
                        let x = app.pet.x + edge * span.sled_gap(app);
                        self.at = Some(x);
                        hand = Some(app.snowball_position());
                        if arrived {
                            let x = x.clamp(span.left, span.right);
                            self.at = Some(x);
                            world.winter_mut().sled = Some(Sled {
                                position: (x / width).clamp(0.0, 1.0),
                                rides: 0,
                            });
                            world.save_now();
                            self.nearby.clear();
                            idle(app);
                            hand = None;
                            Journey::Parked
                        } else {
                            self.journey
                        }
                    } else {
                        let hidden = boundary + edge * (app.edge_clearance() + 16.0);
                        if move_to(&mut app.pet.x, hidden, 138.0 * dt) {
                            app.pet.face = -edge;
                            app.pet.start(Activity::Special(Special::PullSled));
                            Journey::Fetch {
                                walker,
                                span,
                                edge,
                                returning: true,
                            }
                        } else {
                            self.journey
                        }
                    }
                } else {
                    Journey::Parked
                }
            }
            Journey::Wait {
                rider,
                helper,
                seconds,
            } => self.wait(pets, &frame, rider, helper, seconds),
            Journey::Ride(ride) => self.pull(pets, world, &frame, ride, &mut hand),
            Journey::StepOff { rider, goal } => {
                if let Some(pet) = pets
                    .iter_mut()
                    .find(|pet| pet.id == rider && pet.app.pet.activity == Activity::Approach)
                {
                    if !pet.app.pet.approach_at(&pet.app.room, goal, 125.0, dt) {
                        self.journey
                    } else {
                        idle(&mut pet.app);
                        Journey::Parked
                    }
                } else {
                    Journey::Parked
                }
            }
        };
        if self.at.is_some() || self.broken.is_some() {
            self.art.update(frame.scale);
        }
        let direction = if self.direction < 0.0 { -1.0 } else { 1.0 };
        if let Some((x, hand)) = self.at.zip(hand) {
            self.art.rope(
                Point::from_xy(x + direction * 44.0, frame.floor - 16.0),
                hand,
            );
        }
    }

    fn try_board(&mut self, pets: &mut [Instance], x: f32) -> Journey {
        self.nearby.retain(|id| {
            pets.iter().any(|pet| {
                pet.id == *id && (pet.app.pet.x - x).abs() <= boarding_distance(&pet.app)
            })
        });
        let mut chosen = None;
        for pet in pets {
            if (pet.app.pet.x - x).abs() <= boarding_distance(&pet.app)
                && self.nearby.insert(pet.id)
                && chosen.is_none()
                && pet.app.pet.activity == Activity::Walk
                && fastrand::u8(0..100) < 5
            {
                pet.app.pet.start(Activity::Special(Special::RideSled));
                seat(&mut pet.app, x, self.floor, self.direction);
                chosen = Some(Journey::Wait {
                    rider: pet.id,
                    helper: None,
                    seconds: 0.0,
                });
            }
        }
        chosen.unwrap_or_default()
    }

    fn wait(
        &self,
        pets: &mut [Instance],
        frame: &SledFrame<'_>,
        rider: u64,
        helper: Option<u64>,
        seconds: f32,
    ) -> Journey {
        let rider_index = active(pets, rider, Special::RideSled);
        let helper_index = helper.and_then(|id| active(pets, id, Special::FetchSled));
        let seconds = seconds + frame.elapsed.max(0.0);
        let Some((rider_index, x)) = rider_index.zip(self.at).filter(|_| seconds < WAIT_SECONDS)
        else {
            if let Some(i) = rider_index {
                idle(&mut pets[i].app);
            }
            if let Some(i) = helper_index {
                idle(&mut pets[i].app);
            }
            return Journey::Parked;
        };
        seat(&mut pets[rider_index].app, x, frame.floor, self.direction);
        let span = Span::for_x(x, frame.width as f32, frame.monitors);
        let edge = span.toward_edge(x);
        let helper_index = helper_index.or_else(|| {
            let i = pets
                .iter()
                .enumerate()
                .filter(|(i, pet)| {
                    *i != rider_index
                        && pet.app.pet.activity.routine()
                        && (span.left..=span.right).contains(&pet.app.pet.x)
                })
                .min_by(|(_, a), (_, b)| {
                    (a.app.pet.x - x).abs().total_cmp(&(b.app.pet.x - x).abs())
                })
                .map(|(i, _)| i)?;
            pets[i].app.pet.start(Activity::Special(Special::FetchSled));
            Some(i)
        });
        let Some(index) = helper_index else {
            return Journey::Wait {
                rider,
                helper: None,
                seconds,
            };
        };
        let helper = &mut pets[index];
        let (left, right) = span.walking_bounds(&helper.app);
        let goal = (x + edge * span.sled_gap(&helper.app)).clamp(left, right);
        let dt = frame.elapsed.clamp(0.0, 0.05);
        if helper
            .app
            .pet
            .approach_at(&helper.app.room, goal, 210.0, dt)
        {
            helper.app.pet.start(Activity::Special(Special::PullSled));
            helper.app.pet.face = edge;
            Journey::Ride(Ride {
                rider,
                helper: helper.id,
                span,
                edge,
                leg: Leg::FirstEdge,
            })
        } else {
            Journey::Wait {
                rider,
                helper: Some(helper.id),
                seconds,
            }
        }
    }

    fn pull(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        frame: &SledFrame<'_>,
        mut ride: Ride,
        hand: &mut Option<Point>,
    ) -> Journey {
        let rider = active(pets, ride.rider, Special::RideSled);
        let helper = active(pets, ride.helper, Special::PullSled);
        let Some((rider, helper, mut x)) = rider
            .zip(helper)
            .zip(self.at)
            .map(|((rider, helper), x)| (rider, helper, x))
        else {
            if let Some(i) = rider {
                idle(&mut pets[i].app);
            }
            if let Some(i) = helper {
                idle(&mut pets[i].app);
            }
            world.changed();
            return Journey::Parked;
        };
        let dt = frame.elapsed.clamp(0.0, 0.05);
        let (left, right) = ride.span.walking_bounds(&pets[helper].app);
        let gap = ride.span.sled_gap(&pets[helper].app);
        let direction = if matches!(ride.leg, Leg::FirstEdge) {
            ride.edge
        } else {
            -ride.edge
        };
        let reached = match ride.leg {
            Leg::Turn => {
                let goal = (x + direction * gap).clamp(left, right);
                if pets[helper]
                    .app
                    .pet
                    .approach_at(&pets[helper].app.room, goal, 210.0, dt)
                {
                    ride.leg = Leg::Across;
                }
                false
            }
            _ => {
                let end = if direction < 0.0 {
                    left + gap
                } else {
                    right - gap
                };
                let reached = move_to(&mut x, end, PULL_SPEED * dt);
                pets[helper].app.pet.x = (x + direction * gap).clamp(left, right);
                reached
            }
        };
        pets[helper].app.pet.face = direction;
        self.direction = direction;
        seat(&mut pets[rider].app, x, frame.floor, direction);
        *hand = Some(pets[helper].app.snowball_position());
        self.at = Some(x);
        if let Some(sled) = world.winter_mut().sled.as_mut() {
            sled.position = (x / frame.width as f32).clamp(0.0, 1.0);
            world.changed();
        }
        if reached {
            if matches!(ride.leg, Leg::FirstEdge) {
                ride.leg = Leg::Turn;
            } else {
                idle(&mut pets[helper].app);
                let app = &mut pets[rider].app;
                let (left, right) = ride.span.walking_bounds(app);
                let goal = (x - direction * 55.0).clamp(left, right);
                app.pet.start(Activity::Approach);
                *hand = None;
                if let Some(sled) = world.winter_mut().sled.as_mut() {
                    sled.rides += 1;
                    if sled.rides >= 2 {
                        world.winter_mut().sled = None;
                        self.broken = Some((x, 0.0));
                        self.at = None;
                    }
                }
                world.save_now();
                return Journey::StepOff {
                    rider: ride.rider,
                    goal,
                };
            }
        }
        Journey::Ride(ride)
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let s = self.art.scale as f32;
        let at = move |x: f32, y: f32, image: &OverlayImage| {
            (
                (x * s - image.width as f32 * 0.5).round() as i32,
                (y * s - image.height as f32).round() as i32,
            )
        };
        let orientation = usize::from(self.direction < 0.0);
        let sleigh = self
            .at
            .zip(self.art.sleds.get(orientation))
            .map(move |(x, image)| {
                let (x, y) = at(x, self.floor, image);
                (x, y, image)
            });
        let rope = self
            .art
            .rope_origin
            .zip(self.art.rope.as_ref())
            .map(|((x, y), (_, image))| (x, y, image));
        let pieces = self.broken.into_iter().flat_map(move |(x, age)| {
            self.art
                .pieces
                .iter()
                .skip(orientation * 2)
                .take(2)
                .enumerate()
                .map(move |(i, image)| {
                    let side = if i == 0 { -1.0 } else { 1.0 };
                    let up = age * (120.0 - 270.0 * age);
                    let (x, y) = at(x + side * (25.0 + 100.0 * age), self.floor - up, image);
                    (x, y, image)
                })
        });
        rope.into_iter().chain(sleigh).chain(pieces)
    }
}

fn active(pets: &[Instance], id: u64, special: Special) -> Option<usize> {
    pets.iter()
        .position(|pet| pet.id == id && pet.app.pet.activity == Activity::Special(special))
}

fn seat(app: &mut App, x: f32, floor: f32, direction: f32) {
    app.pet.x = x;
    app.pet.y = floor - SEAT_HEIGHT;
    app.pet.face = if direction < 0.0 { -1.0 } else { 1.0 };
}

fn idle(app: &mut App) {
    app.pet.start(Activity::Idle);
    app.pet.y = app.room.floor;
}

fn boarding_distance(app: &App) -> f32 {
    (app.height() * 0.40).clamp(25.0, 60.0)
}

fn move_to(x: &mut f32, goal: f32, distance: f32) -> bool {
    let delta = goal - *x;
    *x += delta.clamp(-distance, distance);
    delta.abs() <= distance
}

#[derive(Default)]
struct Art {
    scale: u32,
    sleds: Vec<OverlayImage>,
    pieces: Vec<OverlayImage>,
    rope: Option<((i32, i32), OverlayImage)>,
    rope_origin: Option<(i32, i32)>,
}

impl Art {
    fn update(&mut self, scale: u32) {
        if scale != self.scale {
            self.scale = scale;
            let sleds = [
                props::sled_image(false, scale),
                props::sled_image(true, scale),
            ];
            self.pieces = sleds
                .iter()
                .flat_map(|sled| {
                    let half = sled.width() / 2;
                    [0, half].into_iter().map(move |x| {
                        sled.clone_rect(
                            IntRect::from_xywh(x as i32, 0, half, sled.height()).unwrap(),
                        )
                        .unwrap()
                        .into()
                    })
                })
                .collect();
            self.sleds = sleds.into_iter().map(OverlayImage::from).collect();
            self.rope = None;
        }
    }

    fn rope(&mut self, from: Point, to: Point) {
        let (x, y) = (from.x.round() as i32, from.y.round() as i32);
        let dx = (to.x.round() as i32 - x).clamp(-210, 210);
        let dy = (to.y.round() as i32 - y).clamp(-210, 210);
        if self.rope.as_ref().is_none_or(|(key, _)| *key != (dx, dy)) {
            self.rope = Some(((dx, dy), props::sled_rope(dx, dy, self.scale).into()));
        }
        let s = self.scale as i32;
        self.rope_origin = Some(((x + dx.min(0) - 4) * s, (y + dy.min(0) - 4) * s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Settings, pack::Pack, startup_room};

    fn pet(id: u64, x: f32) -> Instance {
        let mut app = App::new(
            startup_room(1200.0, 650.0),
            Settings::default(),
            Pack::builtin(),
        );
        app.pet.x = x;
        Instance { id, app }
    }

    fn frame(elapsed: f32) -> SledFrame<'static> {
        SledFrame {
            width: 1200,
            floor: 650.0,
            scale: 1,
            elapsed,
            winter: true,
            monitors: &[(0.0, 600.0), (600.0, 1200.0)],
        }
    }

    #[test]
    fn a_walker_can_fetch_a_sled_from_beyond_the_screen_edge() {
        let mut world = World::test("bear-girl");
        let mut scene = SledScene::default();
        let mut pets = [pet(0, 100.0)];
        let app = &mut pets[0].app;
        app.pet.x = app.room.left + 1.0;
        app.pet.face = -1.0;
        app.pet.start(Activity::Walk);
        let seed = (0..100)
            .find(|&seed| (10..15).contains(&fastrand::Rng::with_seed(seed).u8(0..100)))
            .unwrap();
        fastrand::seed(seed);
        app.pet.step(&app.room, 0.05, true);
        scene.tick(&mut pets, &mut world, frame(0.05));
        assert!(matches!(
            scene.journey,
            Journey::Fetch {
                returning: false,
                ..
            }
        ));
        let mut left_screen = false;
        for _ in 0..120 {
            scene.tick(&mut pets, &mut world, frame(0.05));
            if pets[0].app.pet.x < -pets[0].app.edge_clearance() {
                left_screen = true;
                assert!(pets[0].app.collision_bounds().is_none());
            }
            if world.winter().sled.is_some() {
                break;
            }
        }
        assert!(left_screen);
        assert!(pets[0].app.pet.activity.routine());
        let sled = world.winter().sled.unwrap();
        assert!((130.0..=240.0).contains(&(sled.position * 1200.0)));
        assert_eq!(sled.rides, 0);
    }

    #[test]
    fn the_nearest_helper_has_to_reach_the_sled_before_ten_seconds() {
        let mut world = World::test("bear-girl");
        world.winter_mut().sled = Some(Sled {
            position: 0.24,
            rides: 0,
        });
        let mut pets = [pet(0, 288.0), pet(1, 530.0), pet(2, 350.0)];
        pets[0].app.pet.start(Activity::Special(Special::RideSled));
        let mut scene = SledScene {
            journey: Journey::Wait {
                rider: 0,
                helper: None,
                seconds: 9.90,
            },
            ..SledScene::default()
        };
        scene.tick(&mut pets, &mut world, frame(0.05));
        assert!(matches!(
            scene.journey,
            Journey::Wait {
                helper: Some(2),
                ..
            }
        ));
        scene.tick(&mut pets, &mut world, frame(0.06));
        assert!(matches!(scene.journey, Journey::Parked));
        assert_eq!(pets[0].app.pet.activity, Activity::Idle);
        assert_eq!(pets[2].app.pet.activity, Activity::Idle);
        assert_eq!(world.winter().sled.unwrap().rides, 0);
    }

    #[test]
    fn a_sled_stays_on_its_monitor_and_breaks_after_two_trips() {
        let mut world = World::test("bear-girl");
        world.winter_mut().sled = Some(Sled {
            position: 0.23,
            rides: 0,
        });
        let mut scene = SledScene::default();
        let mut pets = [pet(0, 276.0), pet(1, 340.0), pet(2, 610.0)];
        let seed = (0..100)
            .find(|&seed| fastrand::Rng::with_seed(seed).u8(0..100) < 5)
            .unwrap();
        for trip in 1..=2 {
            let x = world.winter().sled.unwrap().position * 1200.0;
            pets[0].app.pet.start(Activity::Walk);
            pets[0].app.pet.x = x + 130.0;
            scene.tick(&mut pets, &mut world, frame(0.05));
            pets[0].app.pet.x = x + 15.0;
            fastrand::seed(seed);
            scene.tick(&mut pets, &mut world, frame(0.05));
            assert!(matches!(scene.journey, Journey::Wait { rider: 0, .. }));
            let (mut left, mut right) = (600.0_f32, 0.0_f32);
            let mut rope = false;
            for _ in 0..200 {
                scene.tick(&mut pets, &mut world, frame(0.05));
                if let Some(x) = scene.at {
                    assert!((0.0..=600.0).contains(&x));
                    left = left.min(x);
                    right = right.max(x);
                }
                assert!(pets[2].app.pet.activity.routine());
                rope |= scene.art.rope_origin.is_some();
                if matches!(scene.journey, Journey::StepOff { .. }) {
                    break;
                }
            }
            assert!(rope);
            assert!(matches!(scene.journey, Journey::StepOff { .. }));
            assert!(left < 175.0 && right > 425.0, "left={left}, right={right}");
            if trip == 1 {
                assert_eq!(world.winter().sled.unwrap().rides, 1);
                assert!(scene.broken.is_none());
                for _ in 0..10 {
                    scene.tick(&mut pets, &mut world, frame(0.05));
                }
            } else {
                assert!(world.winter().sled.is_none());
                assert!(scene.broken.is_some());
                assert_eq!(scene.images().count(), 2);
                for _ in 0..12 {
                    scene.tick(&mut pets, &mut world, frame(0.05));
                }
                assert_eq!(scene.images().count(), 0);
            }
        }
    }
}
