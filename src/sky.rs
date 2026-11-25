// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::time::{Duration, Instant};

use tiny_skia::{Point, Rect};

use crate::app::Pointer;
use crate::event_art as art;
use crate::game::Instance;
use crate::season::{event_delay, OverlayImage};
use crate::sim::{Activity, Special};
use crate::snowman::Grab;
use crate::world::World;

pub struct Frame<'a> {
    pub width: u32,
    pub floor: f32,
    pub scale: u32,
    pub elapsed: f32,
    pub monitors: &'a [(f32, f32)],
    pub raining: bool,
}

const STAR_RADIUS: f32 = 19.0;
const DOUBLE_CLICK: Duration = Duration::from_millis(420);
const CAPTIVE: Activity = Activity::Special(Special::Captive);

pub struct Sky {
    star_due: Instant,
    rift_due: Instant,
    star_requested: bool,
    rift_requested: bool,
    falling: Option<FallingStar>,
    star: Option<Point>,
    drag: Option<StarDrag>,
    click: Option<Instant>,
    rift: Option<Rift>,
    rain: Lightning,
    star_art: Option<OverlayImage>,
    spark_art: Option<OverlayImage>,
    rainbow: Option<TimedImage>,
    scale: u32,
}

struct FallingStar {
    position: Point,
    goal: f32,
    speed: f32,
}

struct StarDrag {
    grab: Grab,
    initial: Point,
    moving: bool,
}

struct Rift {
    id: u64,
    center: Point,
    captured: Option<Point>,
    target: Point,
    age: f32,
    drawn: i32,
    art: Option<OverlayImage>,
    arm: Option<(Point, OverlayImage)>,
}

struct TimedImage {
    x: i32,
    art: OverlayImage,
    left: f32,
}

#[derive(Default)]
struct Lightning {
    raining: bool,
    next: Strike,
    flash: Option<TimedImage>,
}

#[derive(Default)]
enum Strike {
    #[default]
    None,
    First(f32),
    Recover(u64),
    Again(u64, f32),
}

impl Default for Sky {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            star_due: now + event_delay(111.0),
            rift_due: now + event_delay(236.0),
            star_requested: false,
            rift_requested: false,
            falling: None,
            star: None,
            drag: None,
            click: None,
            rift: None,
            rain: Lightning::default(),
            star_art: None,
            spark_art: None,
            rainbow: None,
            scale: 0,
        }
    }
}

impl Sky {
    pub fn call_star(&mut self) {
        self.star_requested = true;
    }

    pub fn call_rift(&mut self) {
        self.rift_requested = true;
    }

    pub fn wants_frames(&self, pets: bool, star_saved: bool) -> bool {
        self.star_requested
            || (pets && self.rift_requested)
            || self.falling.is_some()
            || self.rainbow.is_some()
            || self.rift.is_some()
            || self.rain.flash.is_some()
            || (star_saved && self.star.is_none())
            || (!star_saved && Instant::now() >= self.star_due)
            || (pets && Instant::now() >= self.rift_due)
    }

    pub fn tick(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        frame: Frame<'_>,
    ) -> Option<u64> {
        let Frame {
            width,
            floor,
            scale,
            elapsed,
            raining,
            ..
        } = frame;
        if width == 0 || floor <= 0.0 {
            return None;
        }
        let dt = elapsed.clamp(0.0, 0.05);
        let now = Instant::now();
        let width = width as f32;
        if self.scale != scale {
            self.scale = scale;
            self.star_art = None;
            self.spark_art = None;
            self.rainbow = None;
            self.rain.flash = None;
            if let Some(rift) = &mut self.rift {
                rift.drawn = -1;
            }
        }
        if self.drag.is_none()
            && self.falling.is_none()
            && (self.star_requested || (now >= self.star_due && world.star().is_none()))
        {
            self.star_requested = false;
            self.star_due = now + event_delay(111.0);
            let goal = bounded_x(width * (0.16 + 0.68 * fastrand::f32()), width);
            let side = if fastrand::bool() { -1.0 } else { 1.0 };
            let x = bounded_x(goal + side * floor * 0.36, width);
            self.falling = Some(FallingStar {
                position: Point::from_xy(x, -24.0),
                goal,
                speed: 460.0,
            });
            self.click = None;
        }
        if let Some(fall) = &mut self.falling {
            let target = floor - STAR_RADIUS;
            let distance = (target - fall.position.y).max(0.0);
            let step = fall.speed * dt;
            fall.speed += 155.0 * dt;
            if distance <= step {
                world.set_star(Some(fall.goal / width));
                self.falling = None;
            } else {
                fall.position.x += (fall.goal - fall.position.x) * step / distance;
                fall.position.y += step;
            }
        }
        if self.drag.is_none() {
            self.star = self.falling.as_ref().map(|fall| fall.position).or_else(|| {
                world
                    .star()
                    .map(|x| Point::from_xy(bounded_x(x * width, width), floor - STAR_RADIUS))
            });
        }
        if self.star.is_some() && self.star_art.is_none() {
            self.star_art = Some(art::star(scale));
            self.spark_art = Some(art::spark(scale));
        }
        if self.rainbow.as_mut().is_some_and(|rainbow| {
            rainbow.left -= dt;
            rainbow.left <= 0.0
        }) {
            self.rainbow = None;
        }
        self.rain.tick(pets, raining, dt, floor, scale);
        if self.rift.is_none() && (self.rift_requested || now >= self.rift_due) {
            if let Some(pet) = pets
                .iter()
                .filter(|pet| pet.app.pet.activity.routine() && pet.app.can_be_struck())
                .min_by_key(|_| fastrand::u32(..))
            {
                let x = pet.app.pet.x;
                let center = Point::from_xy(bounded_x(x, width), (floor * 0.52).max(70.0));
                self.rift = Some(Rift {
                    id: pet.id,
                    center,
                    captured: None,
                    target: Point::from_xy(x, floor - pet.app.height() * 0.65),
                    age: 0.0,
                    drawn: -1,
                    art: None,
                    arm: None,
                });
                self.rift_due = now + event_delay(236.0);
                self.rift_requested = false;
            } else if self.rift_requested && pets.is_empty() {
                self.rift_requested = false;
            }
        }
        let rift = self.rift.as_mut()?;
        let lost = rift.tick(pets, dt, floor, scale);
        if rift.age >= 5.25 {
            self.rift = None;
        }
        lost
    }

    pub fn grab_at(&self) -> Option<Point> {
        self.drag.as_ref().map(|drag| drag.grab.cursor())
    }

    pub fn input_bounds(&self) -> Option<Rect> {
        let star = self.star.filter(|_| self.falling.is_none())?;
        Rect::from_xywh(star.x - 16.0, star.y - 16.0, 32.0, 32.0)
    }

    pub fn press(&mut self, point: Point, now: Instant) -> bool {
        let Some(star) = self.star else { return false };
        if self.falling.is_some() || (star.x - point.x).hypot(star.y - point.y) > 17.0 {
            return false;
        }
        self.drag = Some(StarDrag {
            grab: Grab::new(point, star, now),
            initial: point,
            moving: false,
        });
        true
    }

    pub fn drag(
        &mut self,
        world: &mut World,
        point: Point,
        now: Instant,
        kind: Pointer,
        frame: Frame<'_>,
    ) {
        let Some(drag) = &mut self.drag else { return };
        if !matches!(kind, Pointer::Motion | Pointer::LeftUp) {
            return;
        }
        let width = frame.width.max(1) as f32;
        let base = drag.grab.move_to(point, now);
        if (point.x - drag.initial.x).hypot(point.y - drag.initial.y) > 5.0 {
            drag.moving = true;
        }
        if drag.moving {
            self.star = Some(Point::from_xy(
                bounded_x(base.x, width),
                base.y.clamp(0.0, frame.floor),
            ));
        }
        if matches!(kind, Pointer::LeftUp) {
            self.release(world, frame, Some(now));
        }
    }

    pub fn release(&mut self, world: &mut World, frame: Frame<'_>, clicked: Option<Instant>) {
        let Some(drag) = self.drag.take() else { return };
        let Some(star) = self.star else { return };
        let width = frame.width.max(1) as f32;
        if drag.moving {
            self.click = None;
            let goal = bounded_x(star.x, width);
            world.set_star(Some(goal / width));
            if star.y < frame.floor - STAR_RADIUS - 6.0 {
                self.falling = Some(FallingStar {
                    position: star,
                    goal,
                    speed: 240.0,
                });
            }
        } else if let Some(now) = clicked {
            if self
                .click
                .take()
                .is_some_and(|first| now.duration_since(first) <= DOUBLE_CLICK)
            {
                let (left, right) = frame
                    .monitors
                    .iter()
                    .copied()
                    .find(|&(left, right)| (left..=right).contains(&star.x))
                    .unwrap_or((0.0, width));
                self.rainbow = art::rainbow(
                    (right - left).round() as u32,
                    frame.floor as u32,
                    frame.scale,
                )
                .map(|art| TimedImage {
                    x: (left * frame.scale as f32).round() as i32,
                    art,
                    left: 5.0,
                });
                self.star = None;
                self.star_due = now + event_delay(111.0);
                world.set_star(None);
            } else {
                self.click = Some(now);
            }
        } else {
            self.click = None;
        }
    }

    pub fn backdrops(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        self.rainbow
            .as_ref()
            .into_iter()
            .chain(self.rain.flash.as_ref())
            .map(|image| (image.x, 0, &image.art))
    }

    pub fn images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let scale = self.scale as f32;
        let trails = self
            .falling
            .as_ref()
            .zip(self.spark_art.as_ref())
            .into_iter()
            .flat_map(move |(fall, spark)| {
                (1..7).map(move |i| {
                    let step = i as f32;
                    let dx = (fall.position.x - fall.goal).signum() * step * 6.0;
                    let x = fall.position.x + dx + (step * 2.28).sin() * 8.0;
                    at(
                        Point::from_xy(x, fall.position.y - step * 12.0),
                        spark,
                        scale,
                    )
                })
            });
        let rift = self
            .rift
            .as_ref()
            .and_then(|rift| rift.art.as_ref().map(|art| at(rift.center, art, scale)));
        let arm = self
            .rift
            .as_ref()
            .and_then(|rift| rift.arm.as_ref())
            .map(|(top, art)| {
                (
                    (top.x * scale).round() as i32,
                    (top.y * scale).round() as i32,
                    art,
                )
            });
        trails
            .chain(
                self.star
                    .zip(self.star_art.as_ref())
                    .map(|(star, art)| at(star, art, scale)),
            )
            .chain(arm)
            .chain(rift)
    }
}

impl Rift {
    fn tick(&mut self, pets: &mut [Instance], dt: f32, floor: f32, scale: u32) -> Option<u64> {
        let old = self.age;
        self.age += dt;
        let opening = smooth((self.age / 0.78).clamp(0.02, 1.0))
            * smooth(((5.25 - self.age) / 0.92).clamp(0.02, 1.0));
        if old < 2.45 && self.age >= 2.45 {
            let available = |pet: &Instance| {
                (pet.app.pet.x - self.center.x).abs() < 260.0
                    && pet.app.pet.activity.routine()
                    && pet.app.can_be_struck()
            };
            if !pets.iter().any(|pet| pet.id == self.id && available(pet)) {
                if let Some(pet) = pets.iter().filter(|pet| available(pet)).min_by(|a, b| {
                    (a.app.pet.x - self.center.x)
                        .abs()
                        .total_cmp(&(b.app.pet.x - self.center.x).abs())
                }) {
                    self.id = pet.id;
                }
            }
        }
        let pet = pets.iter_mut().find(|pet| pet.id == self.id);
        let mut lost = None;
        if let Some(pet) = pet {
            let head_height = pet.app.height() * 0.65;
            let actor = &pet.app.pet;
            let local = Point::from_xy(actor.x, actor.y - pet.app.room.floor + floor - head_height);
            if self.age < 2.45 {
                self.target = local;
            }
            if old < 2.45
                && self.age >= 2.45
                && pet.app.can_be_struck()
                && pet.app.pet.activity.routine()
                && (local.x - self.center.x).abs() < 260.0
            {
                self.captured = Some(Point::from_xy(pet.app.pet.x, pet.app.pet.y));
                pet.app.pet.start(CAPTIVE);
            }
            if let Some(from) = self.captured.filter(|_| pet.app.pet.activity == CAPTIVE) {
                let progress = smooth(((self.age - 2.45) / 1.75).clamp(0.0, 1.0));
                pet.app.pet.x = from.x + (self.center.x - from.x) * progress;
                let into = pet.app.room.floor - floor + self.center.y + head_height * 0.91;
                pet.app.pet.y = from.y + (into - from.y) * progress;
                if self.age > 3.75 {
                    pet.app
                        .fade_into_rift(1.0 - smooth(((self.age - 3.75) / 0.45).clamp(0.0, 1.0)));
                }
                if old < 4.20 && self.age >= 4.20 {
                    lost = Some(pet.id);
                }
            }
        }
        let redraw = (self.age * 23.0) as i32;
        if redraw != self.drawn {
            self.drawn = redraw;
            self.art = Some(art::rift(
                opening,
                ((self.target.x - self.center.x) / 115.0).clamp(-1.0, 1.0),
                scale,
            ));
            let reach = smooth(((self.age - 1.15) / 1.30).clamp(0.0, 1.0))
                * smooth(((4.20 - self.age) / 1.75).clamp(0.0, 1.0));
            let delta = Point::from_xy(
                (self.target.x - self.center.x) * reach,
                (self.target.y - self.center.y - 16.0) * reach,
            );
            self.arm = if reach > 0.04 {
                art::bone_arm(delta, scale).map(|art| {
                    (
                        Point::from_xy(
                            self.center.x - 31.0 + delta.x.min(0.0),
                            self.center.y - 31.0 + delta.y.min(0.0),
                        ),
                        art,
                    )
                })
            } else {
                None
            };
        }
        lost
    }
}

impl Lightning {
    fn tick(&mut self, pets: &mut [Instance], raining: bool, dt: f32, floor: f32, scale: u32) {
        if self.flash.as_mut().is_some_and(|flash| {
            flash.left -= dt;
            flash.left <= 0.0 || !raining
        }) {
            self.flash = None;
        }
        if !raining {
            self.next = Strike::None;
        } else if !self.raining {
            self.next = if fastrand::f32() < 0.10 {
                Strike::First(2.0 + fastrand::f32() * 9.0)
            } else {
                Strike::None
            };
        }
        self.raining = raining;
        let mut target = None;
        self.next = match std::mem::take(&mut self.next) {
            Strike::First(delay) if delay <= dt => {
                target = pets
                    .iter()
                    .filter(|pet| pet.app.can_be_struck())
                    .min_by_key(|_| fastrand::u32(..))
                    .map(|pet| pet.id);
                if let Some(id) = target {
                    Strike::Recover(id)
                } else {
                    Strike::First(0.8)
                }
            }
            Strike::First(delay) => Strike::First(delay - dt),
            Strike::Recover(id) => match pets.iter().find(|pet| pet.id == id) {
                Some(pet) if pet.app.pet.activity.routine() => {
                    if fastrand::f32() < 0.10 {
                        Strike::Again(id, 10.0 * fastrand::f32())
                    } else {
                        Strike::None
                    }
                }
                Some(_) => Strike::Recover(id),
                None => Strike::None,
            },
            Strike::Again(id, delay) if delay <= dt => {
                target = Some(id);
                Strike::None
            }
            Strike::Again(id, delay) => Strike::Again(id, delay - dt),
            Strike::None => Strike::None,
        };
        if let Some(pet) = target.and_then(|id| {
            pets.iter_mut()
                .find(|pet| pet.id == id && pet.app.can_be_struck())
        }) {
            let x = pet.app.pet.x;
            let height =
                (pet.app.pet.y - pet.app.room.floor + floor - pet.app.height() * 0.73).max(1.0);
            self.flash = art::lightning(height as u32, scale).map(|art| TimedImage {
                x: ((x - 70.0) * scale as f32).round() as i32,
                art,
                left: 0.29,
            });
            let direction = if fastrand::bool() { 1.0 } else { -1.0 };
            pet.app.lightning_hit(direction);
        }
    }
}

pub fn at(center: Point, art: &OverlayImage, scale: f32) -> (i32, i32, &OverlayImage) {
    (
        (center.x * scale - art.width as f32 * 0.5).round() as i32,
        (center.y * scale - art.height as f32 * 0.5).round() as i32,
        art,
    )
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn bounded_x(x: f32, width: f32) -> f32 {
    let margin = STAR_RADIUS.min(width * 0.5);
    x.clamp(margin, width - margin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Settings, pack::Pack, startup_room};

    fn frame(elapsed: f32) -> Frame<'static> {
        Frame {
            width: 1000,
            floor: 500.0,
            scale: 1,
            elapsed,
            monitors: &[(0.0, 500.0), (500.0, 1000.0)],
            raining: false,
        }
    }

    #[test]
    fn a_star_moves_and_only_two_clicks_make_a_rainbow_on_its_monitor() {
        let mut world = World::test("bear-girl");
        let mut sky = Sky::default();
        sky.call_star();
        for _ in 0..60 {
            sky.tick(&mut [], &mut world, frame(0.04));
        }
        let start = sky.star.expect("landed");
        assert!(world.star().is_some());
        let now = Instant::now();
        assert!(sky.press(start, now));
        let end = Point::from_xy(615.0, start.y);
        sky.drag(
            &mut world,
            end,
            now + Duration::from_millis(90),
            Pointer::LeftUp,
            frame(0.0),
        );
        assert_eq!(world.star(), Some(0.615));
        assert!(sky.rainbow.is_none());
        for (click, millis) in [180, 360].into_iter().enumerate() {
            let t = now + Duration::from_millis(millis);
            assert!(sky.press(end, t));
            sky.drag(&mut world, end, t, Pointer::LeftUp, frame(0.0));
            assert_eq!(sky.rainbow.is_some(), click == 1);
        }
        assert!(world.star().is_none());
        let rainbow = sky.rainbow.as_ref().unwrap();
        assert_eq!(
            (rainbow.x, rainbow.art.width, rainbow.left),
            (500, 500, 5.0)
        );
        for _ in 0..130 {
            sky.tick(&mut [], &mut world, frame(0.04));
        }
        assert!(sky.rainbow.is_none());
    }

    #[test]
    fn lightning_only_strikes_when_raining_and_uses_the_same_id_for_a_second_strike() {
        let mut pets: Vec<_> = (0..2)
            .map(|id| Instance {
                id,
                app: crate::app::App::new(
                    startup_room(1000.0, 500.0),
                    Settings::default(),
                    Pack::builtin(),
                ),
            })
            .collect();
        let mut rain = Lightning {
            raining: true,
            next: Strike::Again(1, 0.01),
            flash: None,
        };
        rain.tick(&mut pets, false, 0.03, 500.0, 1);
        assert!(rain.flash.is_none());
        rain.raining = true;
        rain.next = Strike::Again(1, 0.01);
        rain.tick(&mut pets, true, 0.03, 500.0, 1);
        assert!(rain.flash.is_some());
        assert_eq!(pets[0].app.pet.activity, Activity::Idle);
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        assert!(pets[1].app.pet.vx.abs() >= 1400.0);
        assert!(matches!(rain.next, Strike::None));
    }
}
