// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use tiny_skia::{Point, Rect};

use crate::game::Instance;
use crate::season::OverlayImage;
use crate::sim::{Activity, Special};
use crate::visitor_art;
use crate::world::World;

mod gifts;
mod rewards;
pub use gifts::Gifts;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Santa,
    Ufo,
}

impl Kind {
    fn mean_seconds(self) -> f64 {
        3600.0
            * match self {
                Self::Santa => 15.0,
                Self::Ufo => 72.0,
            }
    }

    fn interval(self) -> f64 {
        -(-fastrand::f64()).ln_1p() * self.mean_seconds()
    }
}

pub struct Frame {
    pub width: u32,
    pub floor: f32,
    pub scale: u32,
    pub elapsed: f32,
}

pub struct LostChibi {
    pub pack: String,
    pub preview: u64,
}

#[derive(Clone, Copy)]
enum Scene {
    Ufo {
        x: f32,
        height: f32,
        flight: UfoFlight,
    },
    Santa {
        x: f32,
        age: f32,
        passenger: Option<(u64, f32)>,
    },
}

#[derive(Clone, Copy)]
enum UfoFlight {
    Arrive(f32),
    Beam(f32),
    Lift { id: u64, from: Point, time: f32 },
    Depart { time: f32, picked_up: Option<u64> },
}

impl UfoFlight {
    fn beam_on(self) -> bool {
        matches!(self, Self::Beam(_) | Self::Lift { .. })
    }
}

#[derive(Default)]
struct Art {
    scale: u32,
    ufo: Option<OverlayImage>,
    santa: Vec<OverlayImage>,
    beam: Option<(u32, OverlayImage)>,
}

pub struct Visitors {
    santa_in: f64,
    ufo_in: f64,
    pending: VecDeque<Kind>,
    scene: Option<Scene>,
    art: Art,
    floor: f32,
    santa_clicks: Option<(Instant, u8)>,
    santa_dropped: bool,
}

impl Default for Visitors {
    fn default() -> Self {
        Self {
            santa_in: Kind::Santa.interval(),
            ufo_in: Kind::Ufo.interval(),
            pending: VecDeque::new(),
            scene: None,
            art: Art::default(),
            floor: 0.0,
            santa_clicks: None,
            santa_dropped: false,
        }
    }
}

impl Visitors {
    pub fn call(&mut self, guest: Kind) {
        if !self.pending.contains(&guest) {
            self.pending.push_back(guest);
        }
    }

    pub fn active(&self) -> bool {
        self.scene.is_some() || !self.pending.is_empty()
    }

    pub fn tick(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        frame: Frame,
        snowy: bool,
    ) -> Option<u64> {
        let Frame {
            width,
            floor,
            scale,
            elapsed: dt,
        } = frame;
        if width == 0 || floor <= 0.0 {
            return None;
        }
        let width = width as f32;
        self.floor = floor;
        if !pets.is_empty() {
            let elapsed = f64::from(dt.clamp(0.0, 0.25));
            self.ufo_in -= elapsed;
            if snowy {
                self.santa_in -= elapsed;
            }
        }
        if self.scene.is_none() {
            let automatic = if !pets.is_empty() && snowy && self.santa_in <= 0.0 {
                Some(Kind::Santa)
            } else if !pets.is_empty() && self.ufo_in <= 0.0 {
                Some(Kind::Ufo)
            } else {
                None
            };
            let guest = if let Some(guest) = self.pending.pop_front() {
                Some(guest)
            } else {
                if let Some(guest) = automatic {
                    match guest {
                        Kind::Santa => self.santa_in = guest.interval(),
                        Kind::Ufo => self.ufo_in = guest.interval(),
                    }
                }
                automatic
            }?;
            self.scene = Some(match guest {
                Kind::Santa => {
                    self.santa_clicks = None;
                    self.santa_dropped = false;
                    Scene::Santa {
                        x: -visitor_art::SANTA_WIDTH * 0.5,
                        age: 0.0,
                        passenger: None,
                    }
                }
                Kind::Ufo => {
                    let candidate = pets
                        .iter()
                        .find(|pet| pet.app.pet.activity == Activity::Walk)
                        .or_else(|| pets.iter().find(|pet| pet.app.pet.activity.routine()));
                    let margin = (visitor_art::BEAM_WIDTH * 0.5).min(width * 0.5);
                    let x = candidate.map_or(width * 0.5, |pet| {
                        let actor = &pet.app.pet;
                        let lead = if actor.activity == Activity::Walk {
                            actor.face * 75.0
                        } else {
                            0.0
                        };
                        actor.x + lead
                    });
                    Scene::Ufo {
                        x: x.clamp(margin, width - margin),
                        height: (floor * 0.19).clamp(65.0, 190.0).min(floor * 0.65),
                        flight: UfoFlight::Arrive(0.0),
                    }
                }
            });
        }
        if self.art.scale != scale {
            self.art = Art {
                scale,
                ..Art::default()
            };
        }
        let dt = dt.clamp(0.0, 0.05);
        match self.scene.take()? {
            Scene::Ufo { x, height, flight } => {
                if self.art.ufo.is_none() {
                    self.art.ufo = Some(visitor_art::ufo(scale).into());
                }
                let ray_height = ((floor - height).max(1.0)).round() as u32;
                if flight.beam_on() && self.art.beam.as_ref().is_none_or(|(h, _)| *h != ray_height)
                {
                    self.art.beam = Some((ray_height, visitor_art::beam(ray_height, scale).into()));
                }
                let flight = match flight {
                    UfoFlight::Arrive(time) if time + dt >= 0.95 => UfoFlight::Beam(0.0),
                    UfoFlight::Arrive(time) => UfoFlight::Arrive(time + dt),
                    UfoFlight::Beam(time) => {
                        let target = pets
                            .iter_mut()
                            .filter(|pet| {
                                pet.app.pet.activity.routine()
                                    && pet.app.grab_position().is_none()
                                    && (pet.app.pet.x - x).abs() <= visitor_art::BEAM_WIDTH * 0.48
                            })
                            .min_by(|a, b| {
                                (a.app.pet.x - x).abs().total_cmp(&(b.app.pet.x - x).abs())
                            });
                        if let Some(pet) = target {
                            let from = Point::from_xy(pet.app.pet.x, pet.app.pet.y);
                            pet.app.pet.start(Activity::Special(Special::Captive));
                            UfoFlight::Lift {
                                id: pet.id,
                                from,
                                time: 0.0,
                            }
                        } else if time + dt >= 7.0 {
                            UfoFlight::Depart {
                                time: 0.0,
                                picked_up: None,
                            }
                        } else {
                            UfoFlight::Beam(time + dt)
                        }
                    }
                    UfoFlight::Lift { id, from, time } => {
                        let time = time + dt;
                        if let Some(pet) = captive(pets, id) {
                            let progress = smooth((time / 1.7).min(1.0));
                            let roof = pet.app.room.floor - floor + height - 10.0;
                            pet.app.pet.x = from.x + (x - from.x) * progress;
                            pet.app.pet.y = from.y + (roof - from.y) * progress;
                        }
                        if time >= 1.7 {
                            UfoFlight::Depart {
                                time: 0.0,
                                picked_up: Some(id),
                            }
                        } else {
                            UfoFlight::Lift { id, from, time }
                        }
                    }
                    UfoFlight::Depart { time, picked_up } => {
                        let time = time + dt;
                        if let Some(pet) = picked_up.and_then(|id| captive(pets, id)) {
                            pet.app.pet.x = x;
                            pet.app.pet.y = pet.app.room.floor - floor
                                + ufo_y(height, UfoFlight::Depart { time, picked_up })
                                - 10.0;
                        }
                        if time >= 0.96 {
                            return picked_up;
                        }
                        UfoFlight::Depart { time, picked_up }
                    }
                };
                if matches!(flight, UfoFlight::Beam(_)) && self.art.beam.is_none() {
                    self.art.beam = Some((ray_height, visitor_art::beam(ray_height, scale).into()));
                }
                self.scene = Some(Scene::Ufo { x, height, flight });
            }
            Scene::Santa {
                x,
                age,
                mut passenger,
            } => {
                if self.art.santa.is_empty() {
                    self.art.santa = (0..4)
                        .map(|leg| visitor_art::santa(leg, scale).into())
                        .collect();
                }
                let next = x + 335.0 * dt;
                let age = age + dt;
                if passenger.is_none() {
                    let grab = visitor_art::SANTA_GRAB;
                    let target = pets
                        .iter_mut()
                        .filter(|pet| {
                            pet.app.pet.activity.routine()
                                && pet.app.grab_position().is_none()
                                && (x + grab - 13.0..=next + grab + 13.0).contains(&pet.app.pet.x)
                        })
                        .min_by(|a, b| a.app.pet.x.total_cmp(&b.app.pet.x));
                    if let Some(pet) = target {
                        pet.app.pet.start(Activity::Special(Special::Captive));
                        passenger = Some((pet.id, age));
                        if !self.santa_dropped
                            && world.winter().santa_bag.is_none()
                            && fastrand::f32() < 0.05
                        {
                            self.drop_bag(world, pet.app.pet.x, width);
                        }
                    }
                }
                if let Some((id, since)) = passenger {
                    if let Some(pet) = captive(pets, id) {
                        let bottom = self.santa_y(age, passenger);
                        let lift = smooth(((age - since) / 0.22).min(1.0));
                        pet.app.pet.x = next + visitor_art::SANTA_GRAB;
                        pet.app.pet.y = pet.app.room.floor + (bottom - floor + 13.0) * lift;
                    }
                }
                if next >= width + visitor_art::SANTA_WIDTH {
                    return passenger.map(|(id, _)| id);
                }
                self.scene = Some(Scene::Santa {
                    x: next,
                    age,
                    passenger,
                });
            }
        }
        None
    }

    fn drop_bag(&mut self, world: &mut World, x: f32, width: f32) {
        self.santa_dropped = true;
        world.winter_mut().santa_bag = Some(gift_position(x, width));
        world.winter_mut().santa_bag_on_sled = false;
        world.save_now();
    }

    pub fn click_santa(
        &mut self,
        world: &mut World,
        point: Point,
        now: Instant,
        width: u32,
    ) -> Option<bool> {
        if width == 0
            || !self.input_rects().any(|rect| {
                (rect.left()..rect.right()).contains(&point.x)
                    && (rect.top()..rect.bottom()).contains(&point.y)
            })
        {
            return None;
        }
        let Some(Scene::Santa { x, .. }) = self.scene else {
            return None;
        };
        let mut dropped = false;
        if !self.santa_dropped {
            let (first, taps) = self
                .santa_clicks
                .filter(|(first, _)| {
                    now.saturating_duration_since(*first) <= Duration::from_millis(850)
                })
                .unwrap_or((now, 0));
            if taps == 2 {
                self.drop_bag(world, x + visitor_art::SANTA_GRAB, width as f32);
                dropped = true;
            } else {
                self.santa_clicks = Some((first, taps + 1));
            }
        }
        Some(dropped)
    }

    pub fn input_rects(&self) -> impl Iterator<Item = Rect> + '_ {
        self.santa_image()
            .into_iter()
            .flat_map(move |(x, y, image)| {
                let scale = self.art.scale.max(1) as i32;
                let s = scale as f32;
                image.ink.iter().filter_map(move |&(left, top, width)| {
                    (i32::from(top) % scale == scale / 2)
                        .then(|| {
                            Rect::from_xywh(
                                (x + i32::from(left)) as f32 / s,
                                (y + i32::from(top) - scale / 2) as f32 / s,
                                f32::from(width) / s,
                                1.0,
                            )
                        })
                        .flatten()
                })
            })
    }

    fn santa_image(&self) -> Option<(i32, i32, &OverlayImage)> {
        let Scene::Santa { x, age, passenger } = self.scene? else {
            return None;
        };
        let image = self.art.santa.get((age * 12.0) as usize % 4)?;
        Some(image_at(
            Point::from_xy(x, self.santa_y(age, passenger)),
            image,
            self.art.scale as f32,
        ))
    }

    fn santa_y(&self, age: f32, passenger: Option<(u64, f32)>) -> f32 {
        let height = passenger.map_or(0.0, |(_, since)| {
            55.0 * smooth(((age - since) / 1.4).min(1.0))
        });
        self.floor - 28.0 + (age * 9.0).sin() * 6.0 - height
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let scale = self.art.scale as f32;
        let (mut guest, mut ray) = (None, None);
        match self.scene {
            Some(Scene::Ufo { x, height, flight }) => {
                guest =
                    self.art.ufo.as_ref().map(|image| {
                        image_at(Point::from_xy(x, ufo_y(height, flight)), image, scale)
                    });
                if flight.beam_on() {
                    ray =
                        self.art.beam.as_ref().map(|(_, image)| {
                            image_at(Point::from_xy(x, self.floor), image, scale)
                        });
                }
            }
            Some(Scene::Santa { .. }) => guest = self.santa_image(),
            None => {}
        }
        ray.into_iter().chain(guest)
    }
}

fn captive(pets: &mut [Instance], id: u64) -> Option<&mut Instance> {
    pets.iter_mut()
        .find(|pet| pet.id == id && pet.app.pet.activity == Activity::Special(Special::Captive))
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn ufo_y(height: f32, flight: UfoFlight) -> f32 {
    match flight {
        UfoFlight::Arrive(time) => -4.0 + (height + 4.0) * smooth((time / 0.95).min(1.0)),
        UfoFlight::Depart { time, .. } => height - (height + 58.0) * smooth((time / 0.96).min(1.0)),
        _ => height,
    }
}

fn gift_position(x: f32, width: f32) -> f32 {
    let margin = (25.0 / width).min(0.5);
    (x / width).clamp(margin, 1.0 - margin)
}

fn image_at(point: Point, image: &OverlayImage, scale: f32) -> (i32, i32, &OverlayImage) {
    (
        (point.x * scale - image.width as f32 * 0.5).round() as i32,
        (point.y * scale - image.height as f32).round() as i32,
        image,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_third_quick_click_on_santa_drops_the_bag_during_the_flight() {
        let mut world = World::test(crate::pack::BUILTIN_ID);
        world.winter_mut().santa_bag = Some(0.75);
        let mut visitors = Visitors {
            scene: Some(Scene::Santa {
                x: 300.0,
                age: 0.0,
                passenger: None,
            }),
            floor: 500.0,
            ..Visitors::default()
        };
        visitors.art.scale = 1;
        visitors.art.santa = vec![visitor_art::santa(0, 1).into()];
        let ink = visitors
            .input_rects()
            .find(|r| r.width() > 5.0)
            .expect("visible Santa");
        let at = Point::from_xy(ink.left() + 2.0, ink.top() + 0.5);
        let start = Instant::now();
        for (tap, elapsed) in [0, 160, 310].into_iter().enumerate() {
            assert_eq!(
                visitors.click_santa(&mut world, at, start + Duration::from_millis(elapsed), 800),
                Some(tap == 2)
            );
        }
        assert_eq!(
            world.winter().santa_bag,
            Some(gift_position(300.0 + visitor_art::SANTA_GRAB, 800.0))
        );
        assert_eq!(
            visitors.click_santa(&mut world, at, start + Duration::from_millis(480), 800),
            Some(false)
        );
    }
}
