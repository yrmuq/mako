// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use tiny_skia::{Point, Rect};

use crate::game::{swept_contact, Instance};
use crate::interactions::RELEASE_AT;
use crate::season::OverlayImage;
use crate::sim::{Activity, Special};
use crate::visitor_art;

use super::image_at;

const BLAST_RADIUS: f32 = 172.0;
const FUSE: f32 = 0.95;
const BLAST_TIME: f32 = 0.36;
const COAL_GRAVITY: f32 = 710.0;
const THROW: Activity = Activity::Special(Special::ThrowCoal);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reward {
    Confetti,
    Candy,
    Bomb,
    Coal,
}

impl Reward {
    pub fn roll(roll: u8) -> Self {
        match roll {
            0..60 => Self::Confetti,
            60..80 => Self::Candy,
            80..90 => Self::Bomb,
            _ => Self::Coal,
        }
    }
}

struct Bomb {
    at: Point,
    fuse: f32,
}

struct Throw {
    owner: u64,
    time: f32,
}

struct Coal {
    owner: u64,
    at: Point,
    velocity: Point,
}

#[derive(Default)]
struct Art {
    scale: u32,
    bomb: Vec<OverlayImage>,
    blast: Vec<OverlayImage>,
    coal: Option<OverlayImage>,
}

#[derive(Default)]
pub struct Rewards {
    bombs: Vec<Bomb>,
    throws: Vec<Throw>,
    coal: Vec<Coal>,
    art: Art,
}

impl Rewards {
    pub fn bomb(&mut self, at: Point) {
        self.bombs.push(Bomb { at, fuse: FUSE });
    }

    pub fn throw_coal(&mut self, actor: &mut Instance) {
        actor.app.pet.start(THROW);
        self.throws.push(Throw {
            owner: actor.id,
            time: 0.0,
        });
    }

    pub fn stop(&self, pets: &mut [Instance]) {
        for pet in pets {
            if pet.app.pet.activity == THROW && self.throws.iter().any(|job| job.owner == pet.id) {
                pet.app.pet.start(Activity::Idle);
            }
        }
    }

    pub fn tick(&mut self, pets: &mut [Instance], width: f32, floor: f32, scale: u32, dt: f32) {
        if self.bombs.is_empty() && self.coal.is_empty() && self.throws.is_empty() {
            return;
        }
        self.art.update(scale);
        self.bombs.retain_mut(|bomb| {
            let started = bomb.fuse > 0.0;
            bomb.fuse -= dt;
            if started && bomb.fuse <= 0.0 {
                bomb.explode(pets);
            }
            bomb.fuse > -BLAST_TIME
        });
        self.throws.retain_mut(|job| {
            let Some(actor) = pets
                .iter()
                .find(|pet| pet.id == job.owner && pet.app.pet.activity == THROW)
            else {
                return false;
            };
            let before = job.time;
            job.time += dt;
            if before < RELEASE_AT && job.time >= RELEASE_AT {
                self.coal.push(Coal::from_throw(actor, pets, width));
            }
            job.time < 0.76
        });
        for pet in pets.iter_mut().filter(|pet| pet.app.pet.activity == THROW) {
            if !self.throws.iter().any(|job| job.owner == pet.id) {
                pet.app.pet.start(Activity::Idle);
            }
        }
        self.coal
            .retain_mut(|coal| coal.fly(pets, width, floor, dt));
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let scale = self.art.scale as f32;
        self.bombs
            .iter()
            .filter_map(move |bomb| {
                let art = if bomb.fuse > 0.0 {
                    self.art.bomb.get((bomb.fuse * 9.0) as usize % 2)?
                } else {
                    self.art
                        .blast
                        .get(((-bomb.fuse / BLAST_TIME) * 3.0) as usize)?
                };
                let bottom = if bomb.fuse > 0.0 {
                    bomb.at.y
                } else {
                    bomb.at.y + art.height as f32 / (3.0 * scale)
                };
                Some(image_at(Point::from_xy(bomb.at.x, bottom), art, scale))
            })
            .chain(self.coal.iter().filter_map(move |coal| {
                let image = self.art.coal.as_ref()?;
                Some(image_at(
                    Point::from_xy(coal.at.x, coal.at.y + 6.0),
                    image,
                    scale,
                ))
            }))
    }
}

impl Bomb {
    fn explode(&self, pets: &mut [Instance]) {
        for pet in pets {
            let dx = pet.app.pet.x - self.at.x;
            let dy = pet.app.pet.y - self.at.y;
            let distance = dx.hypot(dy);
            if distance <= BLAST_RADIUS {
                let side = if dx.abs() > 0.01 {
                    dx.signum()
                } else {
                    pet.app.pet.face
                };
                let strength = (1.0 - distance / BLAST_RADIUS).max(0.0);
                pet.app
                    .blast(side * (615.0 + strength * 690.0), -375.0 - strength * 390.0);
            }
        }
    }
}

impl Coal {
    fn from_throw(actor: &Instance, pets: &[Instance], width: f32) -> Self {
        let at = actor.app.snowball_position();
        let side = actor.app.pet.face;
        let target = pets
            .iter()
            .filter_map(|pet| {
                if pet.id == actor.id || !pet.app.can_be_struck() {
                    return None;
                }
                let body = pet.app.collision_bounds()?;
                let point = Point::from_xy(
                    (body.left() + body.right()) * 0.5,
                    (body.top() + body.bottom()) * 0.5,
                );
                let distance = (point.x - at.x) * side;
                (70.0..=830.0)
                    .contains(&distance)
                    .then_some((point, distance))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let velocity = target.map_or_else(
            || {
                let distance = if side > 0.0 { width - at.x } else { at.x };
                Point::from_xy(if distance < 110.0 { -side } else { side } * 840.0, -305.0)
            },
            |(point, distance)| {
                let time = (distance / 850.0).clamp(0.16, 0.98);
                Point::from_xy(
                    (point.x - at.x) / time,
                    (point.y - at.y) / time - COAL_GRAVITY * time * 0.5,
                )
            },
        );
        Self {
            owner: actor.id,
            at,
            velocity,
        }
    }

    fn fly(&mut self, pets: &mut [Instance], width: f32, floor: f32, dt: f32) -> bool {
        let from = self.bounds();
        self.at.x += self.velocity.x * dt;
        self.at.y += (self.velocity.y + COAL_GRAVITY * dt * 0.5) * dt;
        self.velocity.y += COAL_GRAVITY * dt;
        let to = self.bounds();
        let hit = pets
            .iter_mut()
            .filter_map(|pet| {
                if pet.id == self.owner || !pet.app.can_be_struck() {
                    return None;
                }
                let body = pet.app.collision_bounds()?;
                Some((swept_contact(from, to, body, body)?, pet))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, pet)) = hit {
            pet.app
                .knock_back(self.velocity.x * 1.35, -285.0, Some(self.owner));
            return false;
        }
        self.at.y < floor && (-18.0..width + 18.0).contains(&self.at.x)
    }

    fn bounds(&self) -> Rect {
        Rect::from_xywh(self.at.x - 7.0, self.at.y - 6.0, 14.0, 12.0).unwrap()
    }
}

impl Art {
    fn update(&mut self, scale: u32) {
        if self.scale != scale {
            self.scale = scale;
            self.bomb = [false, true]
                .map(|lit| visitor_art::bomb(lit, scale).into())
                .into();
            self.blast = [0, 1, 2]
                .map(|frame| visitor_art::blast(frame, scale).into())
                .into();
            self.coal = Some(visitor_art::coal(scale).into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cartoon_blast_throws_every_nearby_chibi_away_from_the_gift() {
        use crate::{app::App, config::Settings, pack::Pack, startup_room};
        let mut pets = [230.0, 330.0, 740.0].map(|x| {
            let mut app = App::new(
                startup_room(900.0, 600.0),
                Settings::default(),
                Pack::builtin(),
            );
            app.pet.x = x;
            Instance { id: x as u64, app }
        });
        pets[0].app.pet.start(Activity::Down);
        let at = Point::from_xy(280.0, pets[0].app.room.floor);
        Bomb { at, fuse: 0.0 }.explode(&mut pets);
        assert!(pets[0].app.pet.vx < 0.0);
        assert!(pets[1].app.pet.vx > 0.0);
        assert_eq!(pets[2].app.pet.activity, Activity::Idle);
        assert_eq!(
            [0, 60, 80, 90].map(Reward::roll),
            [Reward::Confetti, Reward::Candy, Reward::Bomb, Reward::Coal]
        );
    }

    #[test]
    fn coal_skips_the_thrower_and_knocks_over_the_first_other_chibi() {
        use crate::{app::App, config::Settings, pack::Pack, startup_room};
        let mut pets = [120.0, 360.0, 520.0].map(|x| {
            let mut app = App::new(
                startup_room(900.0, 600.0),
                Settings::default(),
                Pack::builtin(),
            );
            app.pet.x = x;
            Instance { id: x as u64, app }
        });
        let body = pets[1].app.collision_bounds().unwrap();
        let mut coal = Coal {
            owner: pets[0].id,
            at: Point::from_xy(95.0, (body.top() + body.bottom()) * 0.5),
            velocity: Point::from_xy(840.0, -105.0),
        };
        for _ in 0..24 {
            if !coal.fly(&mut pets, 900.0, 600.0, 0.02) {
                break;
            }
        }
        assert_eq!(pets[0].app.pet.activity, Activity::Idle);
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        assert_eq!(pets[2].app.pet.activity, Activity::Idle);
    }
}
