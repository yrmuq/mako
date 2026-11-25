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

use super::rewards::{Reward, Rewards};
use super::{gift_position, image_at};

#[derive(Default)]
enum Motion {
    #[default]
    Ground,
    Mounted(f32),
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
            Self::Mounted(lift) => *lift,
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
    bag_press: Option<Point>,
    bag_motion: Motion,
    dropped_bag_at: Option<Point>,
    motion: Vec<Motion>,
    opening: Option<Opening>,
    eating: Vec<(u64, f32)>,
    rewards: Rewards,
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
        match &self.bag_motion {
            Motion::Held { grab, .. } => Some(grab.cursor()),
            _ => self.held_slot().map(|(_, cursor)| cursor),
        }
    }

    pub fn bag_dropped(&mut self) {
        self.bag_motion = Motion::Ground;
        self.dropped_bag_at = None;
        self.bag_press = None;
        self.last_bag_click = None;
    }

    pub fn take_bag_drop(&mut self) -> Option<Point> {
        self.dropped_bag_at.take()
    }

    pub fn sync_mounted_bag(
        &mut self,
        world: &mut World,
        seat: Option<Point>,
        width: u32,
        floor: f32,
    ) {
        if !world.winter().santa_bag_on_sled || matches!(self.bag_motion, Motion::Held { .. }) {
            return;
        }
        if let Some(seat) = seat {
            let position = gift_position(seat.x, width.max(1) as f32);
            self.bag_motion = Motion::Mounted((floor - seat.y).max(0.0));
            if world.winter().santa_bag != Some(position) {
                world.winter_mut().santa_bag = Some(position);
                world.changed();
            }
        } else {
            self.bag_motion = airborne(self.bag_motion.lift(), None);
            world.winter_mut().santa_bag_on_sled = false;
            world.save_now();
        }
    }

    pub fn mount_bag(&mut self, world: &mut World, seat: Point, width: u32, floor: f32) {
        world.winter_mut().santa_bag_on_sled = true;
        self.sync_mounted_bag(world, Some(seat), width, floor);
        world.save_now();
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
            if let Some(x) = winter.santa_bag {
                let lift = self.bag_motion.lift();
                let base = Point::from_xy(x * width, floor - lift);
                if contains(bounds(base.x, base.y, 49.0, 47.0), point) {
                    self.bag_motion = Motion::Held {
                        grab: Grab::new(point, base, now),
                        lift,
                    };
                    self.bag_press = Some(point);
                    return true;
                }
            }
            self.last_bag_click = None;
            return false;
        };
        self.last_bag_click = None;
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
        if let Motion::Held { grab, lift } = &mut self.bag_motion {
            if !matches!(kind, Pointer::Motion | Pointer::LeftUp) || width == 0 {
                return;
            }
            let width = width as f32;
            if self
                .bag_press
                .is_some_and(|start| (point.x - start.x).hypot(point.y - start.y) > 5.0)
            {
                self.bag_press = None;
                self.last_bag_click = None;
            }
            if self.bag_press.is_none() {
                world.winter_mut().santa_bag_on_sled = false;
                let at = grab.move_to(point, now);
                *lift = (floor - at.y).clamp(0.0, (floor - 47.0).max(0.0));
                world.winter_mut().santa_bag = Some(gift_position(at.x, width));
                world.changed();
            }
            if matches!(kind, Pointer::LeftUp) {
                if self.bag_press.take().is_some() {
                    self.click_bag(world, now, width);
                } else if let Some(x) = world.winter().santa_bag {
                    self.dropped_bag_at = Some(Point::from_xy(x * width, floor - *lift));
                }
                self.release(world, None);
            }
            return;
        }
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

    fn click_bag(&mut self, world: &mut World, now: Instant, width: f32) {
        if self
            .last_bag_click
            .take()
            .is_some_and(|first| now.saturating_duration_since(first) <= Duration::from_millis(400))
        {
            let Some(x) = world.winter().santa_bag else {
                return;
            };
            let side = if x < 0.5 { 1.0 } else { -1.0 };
            let position = gift_position(x * width + side * (45.0 + fastrand::f32() * 30.0), width);
            world.winter_mut().santa_gifts.push(VisitorGift {
                position,
                box_color: fastrand::u8(0..4),
                ribbon_color: fastrand::u8(0..4),
            });
            self.motion
                .resize_with(world.winter().santa_gifts.len(), Motion::default);
            self.art.update(self.art.scale.max(1));
            world.save_now();
        } else {
            self.last_bag_click = Some(now);
        }
    }

    pub fn release(&mut self, world: &mut World, velocity: Option<Point>) {
        if let Motion::Held { lift, .. } = self.bag_motion {
            if self.bag_press.take().is_some() {
                self.last_bag_click = None;
            }
            self.bag_motion = if world.winter().santa_bag_on_sled {
                Motion::Mounted(lift)
            } else {
                airborne(lift, None)
            };
            world.save_now();
        } else if let Some((slot, _)) = self.held_slot() {
            let lift = self.motion[slot].lift();
            self.motion[slot] = airborne(lift, velocity);
            world.save_now();
        }
    }

    pub fn reset(&mut self, pets: &mut [Instance]) {
        self.cancel_opening(pets);
        for (id, _) in self.eating.drain(..) {
            stop(pets, id, Activity::Special(Special::EatCandy));
        }
        self.rewards.stop(pets);
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
        let mut changed = false;
        let mut landed = false;
        let winter = world.winter_mut();
        if let Some(position) = &mut winter.santa_bag {
            if let Some(grounded) = step_air(&mut self.bag_motion, position, width, dt) {
                changed = true;
                landed |= grounded;
            }
        }
        for (motion, gift) in self.motion.iter_mut().zip(&mut winter.santa_gifts) {
            if let Some(grounded) = step_air(motion, &mut gift.position, width, dt) {
                changed = true;
                landed |= grounded;
            }
        }
        if landed {
            world.save_now();
        } else if changed {
            world.changed();
        }
        self.eating.retain_mut(|(pet, left)| {
            *left -= dt;
            if *left <= 0.0 {
                stop(pets, *pet, Activity::Special(Special::EatCandy));
            }
            *left > 0.0
        });
        self.rewards.tick(pets, width, floor, scale, dt);
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
                    actor.app.pet.start(Activity::Idle);
                    match Reward::roll(fastrand::u8(0..100)) {
                        Reward::Candy => {
                            actor.app.pet.start(Activity::Special(Special::EatCandy));
                            self.eating.push((actor.id, 2.24));
                        }
                        Reward::Confetti => self.burst(Point::from_xy(x, floor - 25.0)),
                        Reward::Bomb => self.rewards.bomb(Point::from_xy(x, floor)),
                        Reward::Coal => self.rewards.throw_coal(actor),
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
            .map(move |x| bounds(x * width, floor - self.bag_motion.lift(), 49.0, 47.0))
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
            .map(|(x, image)| {
                image_at(
                    Point::from_xy(x * width, floor - self.bag_motion.lift()),
                    image,
                    scale,
                )
            });
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
            .chain(self.rewards.images())
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

fn airborne(lift: f32, velocity: Option<Point>) -> Motion {
    if lift > 0.0 || velocity.is_some() {
        Motion::Air {
            lift,
            velocity: velocity.unwrap_or_default(),
        }
    } else {
        Motion::Ground
    }
}

fn step_air(motion: &mut Motion, position: &mut f32, width: f32, dt: f32) -> Option<bool> {
    let Motion::Air { lift, velocity } = motion else {
        return None;
    };
    *position = gift_position(*position * width + velocity.x * dt, width);
    *lift -= (velocity.y + 450.0 * dt) * dt;
    velocity.y += 900.0 * dt;
    let landed = *lift <= 0.0 && velocity.y >= 0.0;
    if landed {
        *motion = Motion::Ground;
    }
    Some(landed)
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
        let from = Point::from_xy(400.0, floor - 24.0);
        let up = Point::from_xy(480.0, floor - 94.0);
        let now = Instant::now();
        assert!(gifts.click(&mut [], &mut world, from, now, width, floor));
        gifts.drag(
            &mut world,
            up,
            now + Duration::from_millis(100),
            width,
            floor,
            Pointer::Motion,
        );
        assert_eq!(gifts.bag_motion.lift(), 70.0);
        gifts.drag(
            &mut world,
            up,
            now + Duration::from_millis(150),
            width,
            floor,
            Pointer::LeftUp,
        );
        assert!(world.winter().santa_gifts.is_empty());
        for _ in 0..20 {
            gifts.tick(&mut [], &mut world, width, floor, 1, 0.03);
        }
        let click = Point::from_xy(480.0, floor - 24.0);
        for (tap, elapsed) in [640, 790].into_iter().enumerate() {
            let pressed = now + Duration::from_millis(elapsed);
            assert!(gifts.click(&mut [], &mut world, click, pressed, width, floor));
            gifts.drag(
                &mut world,
                click,
                pressed + Duration::from_millis(40),
                width,
                floor,
                Pointer::LeftUp,
            );
            assert_eq!(world.winter().santa_gifts.len(), tap);
        }
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
        assert_eq!(world.winter().santa_bag, Some(0.6));
    }
}
