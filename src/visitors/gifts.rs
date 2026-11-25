// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use tiny_skia::{Point, Rect};

use crate::app::Pointer;
use crate::game::Instance;
use crate::props;
use crate::season::OverlayImage;
use crate::sim::{Activity, Special};
use crate::snowman::Grab;
use crate::visitor_art;
use crate::world::{VisitorGift, World};

use super::{gift_position, image_at};

#[derive(Default)]
enum Motion {
    #[default]
    Ground,
    Held {
        grab: Grab,
        lift: f32,
    },
    Air {
        lift: f32,
        velocity: Point,
    },
}

impl Motion {
    fn lift(&self) -> f32 {
        match self {
            Self::Ground => 0.0,
            Self::Held { lift, .. } | Self::Air { lift, .. } => *lift,
        }
    }
}

struct Opening {
    pet: u64,
    slot: usize,
    phase: OpenPhase,
}

#[derive(Clone, Copy)]
enum OpenPhase {
    Approach,
    Unwrap(f32),
}

impl OpenPhase {
    fn activity(self) -> Activity {
        match self {
            Self::Approach => Activity::Approach,
            Self::Unwrap(_) => Activity::Special(Special::OpenGift),
        }
    }
}

struct Spark {
    at: Point,
    speed: Point,
    life: f32,
    color: usize,
}

#[derive(Default)]
pub struct Gifts {
    last_bag_click: Option<Instant>,
    motion: Vec<Motion>,
    opening: Option<Opening>,
    eating: Option<(u64, f32)>,
    confetti: Vec<Spark>,
    art: Art,
}

#[derive(Default)]
struct Art {
    scale: u32,
    bag: Option<OverlayImage>,
    boxes: Vec<OverlayImage>,
    confetti: Vec<OverlayImage>,
}

const RIBBONS: [[u8; 4]; 4] = [
    [255, 226, 153, 255],
    [171, 249, 205, 255],
    [255, 235, 252, 255],
    [115, 60, 103, 255],
];

impl Gifts {
    pub fn grabbed_at(&self) -> Option<Point> {
        self.held_slot().map(|(_, cursor)| cursor)
    }

    fn held_slot(&self) -> Option<(usize, Point)> {
        self.motion.iter().enumerate().find_map(|(slot, motion)| {
            if let Motion::Held { grab, .. } = motion {
                Some((slot, grab.cursor()))
            } else {
                None
            }
        })
    }

    pub fn click(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        point: Point,
        now: Instant,
        width: u32,
        floor: f32,
    ) -> bool {
        let width = width as f32;
        let winter = world.winter();
        if let Some(x) = winter
            .santa_bag
            .filter(|x| contains(bounds(*x * width, floor, 49.0, 47.0), point))
        {
            if self.last_bag_click.take().is_some_and(|first| {
                now.saturating_duration_since(first) <= Duration::from_millis(400)
            }) {
                let side = if x < 0.5 { 1.0 } else { -1.0 };
                let position =
                    gift_position(x * width + side * (45.0 + fastrand::f32() * 30.0), width);
                world.winter_mut().santa_gifts.push(VisitorGift {
                    position,
                    box_color: fastrand::u8(0..4),
                    ribbon_color: fastrand::u8(0..4),
                });
                self.motion.push(Motion::Ground);
                self.art.update(self.art.scale.max(1));
                world.save_now();
            } else {
                self.last_bag_click = Some(now);
            }
            return true;
        }
        self.last_bag_click = None;
        let Some(slot) = winter
            .santa_gifts
            .iter()
            .enumerate()
            .rposition(|(slot, gift)| {
                let lift = self.motion.get(slot).map_or(0.0, Motion::lift);
                contains(
                    bounds(gift.position * width, floor - lift, 33.0, 33.0),
                    point,
                )
            })
        else {
            return false;
        };
        if self.opening.as_ref().is_some_and(|job| job.slot == slot) {
            self.cancel_opening(pets);
        }
        self.motion
            .resize_with(winter.santa_gifts.len(), Motion::default);
        let lift = self.motion[slot].lift();
        let base = Point::from_xy(winter.santa_gifts[slot].position * width, floor - lift);
        self.motion[slot] = Motion::Held {
            grab: Grab::new(point, base, now),
            lift,
        };
        true
    }

    pub fn drag(
        &mut self,
        world: &mut World,
        point: Point,
        now: Instant,
        width: u32,
        floor: f32,
        kind: Pointer,
    ) {
        let Some((slot, _)) = self.held_slot() else {
            return;
        };
        let Motion::Held { grab, lift } = &mut self.motion[slot] else {
            return;
        };
        if matches!(kind, Pointer::Motion | Pointer::LeftUp) && width > 0 {
            let at = grab.move_to(point, now);
            *lift = (floor - at.y).clamp(0.0, (floor - 33.0).max(0.0));
            world.winter_mut().santa_gifts[slot].position = gift_position(at.x, width as f32);
            world.changed();
        }
        if matches!(kind, Pointer::LeftUp) {
            let velocity = grab.throw(now);
            self.release(world, velocity);
        }
    }

    pub fn release(&mut self, world: &mut World, velocity: Option<Point>) {
        if let Some((slot, _)) = self.held_slot() {
            let lift = self.motion[slot].lift();
            self.motion[slot] = if lift > 0.0 || velocity.is_some() {
                Motion::Air {
                    lift,
                    velocity: velocity.unwrap_or_default(),
                }
            } else {
                Motion::Ground
            };
            world.save_now();
        }
    }

    pub fn reset(&mut self, pets: &mut [Instance]) {
        self.cancel_opening(pets);
        if let Some((id, _)) = self.eating.take() {
            stop(pets, id, Activity::Special(Special::EatCandy));
        }
        *self = Self::default();
    }

    fn cancel_opening(&mut self, pets: &mut [Instance]) {
        if let Some(opening) = self.opening.take() {
            stop(pets, opening.pet, opening.phase.activity());
        }
    }

    pub fn tick(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        width: u32,
        floor: f32,
        scale: u32,
        dt: f32,
    ) {
        if width == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        let width = width as f32;
        self.motion
            .resize_with(world.winter().santa_gifts.len(), Motion::default);
        if world.winter().santa_bag.is_some()
            || !self.motion.is_empty()
            || !self.confetti.is_empty()
        {
            self.art.update(scale);
        }
        for (slot, motion) in self.motion.iter_mut().enumerate() {
            if let Motion::Air { lift, velocity } = motion {
                let old = world.winter().santa_gifts[slot].position;
                let next = gift_position(old * width + velocity.x * dt, width);
                *lift -= (velocity.y + 450.0 * dt) * dt;
                velocity.y += 900.0 * dt;
                world.winter_mut().santa_gifts[slot].position = next;
                if *lift <= 0.0 && velocity.y >= 0.0 {
                    *motion = Motion::Ground;
                    world.save_now();
                } else {
                    world.changed();
                }
            }
        }
        if let Some((pet, left)) = &mut self.eating {
            *left -= dt;
            if *left <= 0.0 {
                stop(pets, *pet, Activity::Special(Special::EatCandy));
                self.eating = None;
            }
        }
        self.confetti.retain_mut(|spark| {
            spark.life -= dt;
            spark.speed.y += 320.0 * dt;
            spark.at.x += spark.speed.x * dt;
            spark.at.y += spark.speed.y * dt;
            spark.life > 0.0 && spark.at.y <= floor + 4.0
        });
        self.open(pets, world, width, floor, dt);
    }

    fn open(&mut self, pets: &mut [Instance], world: &mut World, width: f32, floor: f32, dt: f32) {
        if let Some(mut work) = self.opening.take() {
            let Some(actor) = pets
                .iter_mut()
                .find(|pet| pet.id == work.pet && pet.app.pet.activity == work.phase.activity())
            else {
                return;
            };
            let Some(gift) = world.winter().santa_gifts.get(work.slot) else {
                actor.app.pet.start(Activity::Idle);
                return;
            };
            let x = gift.position * width;
            let side = if actor.app.pet.x < x { -1.0 } else { 1.0 };
            match work.phase {
                OpenPhase::Approach
                    if actor.app.pet.approach(&actor.app.room, x + side * 31.0, dt) =>
                {
                    actor.app.pet.face = -side;
                    actor.app.pet.start(Activity::Special(Special::OpenGift));
                    work.phase = OpenPhase::Unwrap(0.0);
                }
                OpenPhase::Unwrap(time) if time + dt >= 1.4 => {
                    world.winter_mut().santa_gifts.remove(work.slot);
                    self.motion.remove(work.slot);
                    world.save_now();
                    if self.eating.is_none() && fastrand::f32() < 0.20 {
                        actor.app.pet.start(Activity::Special(Special::EatCandy));
                        self.eating = Some((actor.id, 2.24));
                    } else {
                        actor.app.pet.start(Activity::Idle);
                        self.burst(Point::from_xy(x, floor - 25.0));
                    }
                    return;
                }
                OpenPhase::Unwrap(time) => work.phase = OpenPhase::Unwrap(time + dt),
                OpenPhase::Approach => {}
            }
            self.opening = Some(work);
            return;
        }
        for (slot, gift) in world.winter().santa_gifts.iter().enumerate() {
            if !matches!(self.motion[slot], Motion::Ground) {
                continue;
            }
            let x = gift.position * width;
            let Some(actor) = pets
                .iter_mut()
                .filter(|pet| pet.app.pet.activity.routine() && (pet.app.pet.x - x).abs() <= 118.0)
                .min_by(|a, b| (a.app.pet.x - x).abs().total_cmp(&(b.app.pet.x - x).abs()))
            else {
                continue;
            };
            actor.app.pet.start(Activity::Approach);
            self.opening = Some(Opening {
                pet: actor.id,
                slot,
                phase: OpenPhase::Approach,
            });
            break;
        }
    }

    fn burst(&mut self, at: Point) {
        for i in 0..18 {
            self.confetti.push(Spark {
                at,
                speed: Point::from_xy(
                    (fastrand::f32() - 0.5) * 200.0,
                    -78.0 - fastrand::f32() * 165.0,
                ),
                life: 0.55 + fastrand::f32() * 0.8,
                color: i % props::FESTIVE_COLORS.len(),
            });
        }
    }

    pub fn input_rects<'a>(
        &'a self,
        world: &'a World,
        width: u32,
        floor: f32,
    ) -> impl Iterator<Item = Rect> + 'a {
        let width = width as f32;
        let winter = world.winter();
        winter
            .santa_bag
            .into_iter()
            .map(move |x| bounds(x * width, floor, 49.0, 47.0))
            .chain(
                winter
                    .santa_gifts
                    .iter()
                    .enumerate()
                    .map(move |(slot, gift)| {
                        let lift = self.motion.get(slot).map_or(0.0, Motion::lift);
                        bounds(gift.position * width, floor - lift, 33.0, 33.0)
                    }),
            )
    }

    pub fn images<'a>(
        &'a self,
        world: &'a World,
        width: u32,
        floor: f32,
    ) -> impl Iterator<Item = (i32, i32, &'a OverlayImage)> + Clone {
        let width = width as f32;
        let scale = self.art.scale as f32;
        let winter = world.winter();
        let bag = winter
            .santa_bag
            .zip(self.art.bag.as_ref())
            .map(|(x, image)| image_at(Point::from_xy(x * width, floor), image, scale));
        bag.into_iter()
            .chain(
                winter
                    .santa_gifts
                    .iter()
                    .enumerate()
                    .filter_map(move |(slot, gift)| {
                        let image = self.art.boxes.get(
                            usize::from(gift.box_color) * RIBBONS.len()
                                + usize::from(gift.ribbon_color),
                        )?;
                        let lift = self.motion.get(slot).map_or(0.0, Motion::lift);
                        Some(image_at(
                            Point::from_xy(gift.position * width, floor - lift),
                            image,
                            scale,
                        ))
                    }),
            )
            .chain(self.confetti.iter().filter_map(move |spark| {
                Some(image_at(
                    spark.at,
                    self.art.confetti.get(spark.color)?,
                    scale,
                ))
            }))
    }
}

impl Art {
    fn update(&mut self, scale: u32) {
        if self.scale == scale {
            return;
        }
        self.scale = scale;
        self.bag = Some(visitor_art::bag(scale).into());
        self.boxes = (0..props::FESTIVE_COLORS.len())
            .flat_map(|color| {
                RIBBONS
                    .into_iter()
                    .map(move |ribbon| props::colored_gift(color, ribbon, scale).into())
            })
            .collect();
        self.confetti = (0..props::FESTIVE_COLORS.len())
            .map(|color| props::confetti(color, scale).into())
            .collect();
    }
}

fn stop(pets: &mut [Instance], id: u64, activity: Activity) {
    if let Some(pet) = pets
        .iter_mut()
        .find(|pet| pet.id == id && pet.app.pet.activity == activity)
    {
        pet.app.pet.start(Activity::Idle);
    }
}

fn bounds(x: f32, floor: f32, width: f32, height: f32) -> Rect {
    Rect::from_xywh(x - width * 0.5, floor - height, width, height).unwrap()
}

fn contains(rect: Rect, point: Point) -> bool {
    (rect.left()..rect.right()).contains(&point.x) && (rect.top()..rect.bottom()).contains(&point.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_double_click_spawns_a_gift_that_can_be_picked_up_and_opened() {
        use crate::{app::App, config::Settings, pack::Pack, startup_room};
        let mut world = World::test("bear-girl");
        let mut gifts = Gifts::default();
        let (width, floor) = (800, 600.0);
        world.winter_mut().santa_bag = Some(0.5);
        let click = Point::from_xy(400.0, floor - 24.0);
        let now = Instant::now();
        assert!(gifts.click(&mut [], &mut world, click, now, width, floor));
        assert!(world.winter().santa_gifts.is_empty());
        assert!(gifts.click(
            &mut [],
            &mut world,
            click,
            now + Duration::from_millis(140),
            width,
            floor
        ));
        assert_eq!(world.winter().santa_gifts.len(), 1);
        let initial = world.winter().santa_gifts[0].position * width as f32;
        let gift = Point::from_xy(initial, floor - 15.0);
        assert!(gifts.click(&mut [], &mut world, gift, now, width, floor));
        let raised = Point::from_xy(initial + 85.0, floor - 105.0);
        gifts.drag(
            &mut world,
            raised,
            now + Duration::from_millis(100),
            width,
            floor,
            Pointer::Motion,
        );
        assert_eq!(gifts.motion[0].lift(), 90.0);
        gifts.drag(
            &mut world,
            raised,
            now + Duration::from_millis(300),
            width,
            floor,
            Pointer::LeftUp,
        );
        let mut pets = [Instance {
            id: 1,
            app: App::new(
                startup_room(width as f32, floor),
                Settings::default(),
                Pack::builtin(),
            ),
        }];
        pets[0].app.pet.x = initial + 85.0;
        for _ in 0..80 {
            gifts.tick(&mut pets, &mut world, width, floor, 1, 0.04);
        }
        assert!(world.winter().santa_gifts.is_empty());
        assert_eq!(world.winter().santa_bag, Some(0.5));
    }
}
