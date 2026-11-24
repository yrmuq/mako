// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::ffi::CStr;

use serde::{Deserialize, Serialize};
use tiny_skia::Point;

use crate::game::Instance;
use crate::props;
use crate::season::{Date, OverlayImage};
use crate::sim::{Activity, Special};
use crate::world::{World, SNOWMAN_SECONDS};

const TREE_SECONDS: f32 = SNOWMAN_SECONDS * 2.0;
const MATURE: u8 = 5;
const DECORATED: u8 = 4;
const GIFTS: u8 = 0b111;
const GIFT_OFFSETS: [f32; 3] = [-42.0, 38.0, 66.0];

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Tree {
    pub position: f32,
    year: i32,
    calendar: bool,
    watered: f32,
    decorations: u8,
    gifts_started: bool,
    gifts: u8,
}

impl Tree {
    pub fn new(position: f32, year: i32, calendar: bool) -> Self {
        Self {
            position,
            year,
            calendar,
            watered: 0.0,
            decorations: 0,
            gifts_started: false,
            gifts: 0,
        }
    }

    pub fn valid(self) -> bool {
        (0.0..=1.0).contains(&self.position)
            && (1..=10000).contains(&self.year)
            && (0.0..=TREE_SECONDS).contains(&self.watered)
            && self.decorations <= DECORATED
            && self.gifts <= GIFTS
    }

    fn stage(self, date: Date, dates: Dates) -> u8 {
        if self.calendar {
            match date.month {
                11 => 0,
                12 => (1 + 4 * (date.day - 1) / (dates.mature - 1)).min(i32::from(MATURE)) as u8,
                _ => MATURE,
            }
        } else {
            (self.watered / TREE_SECONDS * f32::from(MATURE)).floor() as u8
        }
    }
}

#[derive(Clone, Copy)]
struct Dates {
    mature: i32,
    gifts: i32,
}

impl Dates {
    fn for_format(month_first: bool) -> Self {
        if month_first {
            Self {
                mature: 10,
                gifts: 25,
            }
        } else {
            Self {
                mature: 15,
                gifts: 31,
            }
        }
    }

    fn season(date: Date) -> Option<i32> {
        match (date.month, date.day) {
            (11, 20..=30) | (12, _) => Some(date.year + 1),
            (1 | 2, _) => Some(date.year),
            _ => None,
        }
    }

    fn can_plant(date: Date) -> bool {
        matches!((date.month, date.day), (11, 20..=30) | (12, _) | (1, 1))
    }

    fn holiday(self, date: Date) -> bool {
        date.month == 12 && date.day == self.gifts
    }

    fn gifts_visible(self, date: Date) -> bool {
        matches!((date.month, date.day), (1, 1..=13)) || date.month == 12 && date.day >= self.gifts
    }

    fn untrim(date: Date) -> bool {
        (date.month == 1 && date.day >= 14) || date.month == 2
    }

    fn decorations(self, date: Date) -> u8 {
        match (date.month, date.day) {
            (12, day) if day >= self.mature => {
                let span = self.gifts - 7 - self.mature;
                (1 + 3 * (day - self.mature) / span).min(i32::from(DECORATED)) as u8
            }
            (1, 1..=13) => DECORATED,
            _ => 0,
        }
    }
}

fn month_first(format: &[u8]) -> bool {
    let mut bytes = format.iter().copied();
    while bytes.any(|byte| byte == b'%') {
        let field = bytes.find(|byte| !matches!(byte, b'E' | b'O' | b'-' | b'_' | b'0'));
        match field {
            Some(b'm' | b'b' | b'B' | b'h' | b'D') => return true,
            Some(b'd' | b'e' | b'F' | b'Y' | b'y' | b'C' | b'G' | b'g') => return false,
            _ => {}
        }
    }
    false
}

fn local_dates() -> Dates {
    // A dedicated time locale keeps the process-wide locale and the UI language independent.
    let month_first = unsafe {
        let locale = libc::newlocale(libc::LC_TIME_MASK, c"".as_ptr(), std::ptr::null_mut());
        if locale.is_null() {
            false
        } else {
            let format = libc::nl_langinfo_l(libc::D_FMT, locale);
            let first = !format.is_null() && month_first(CStr::from_ptr(format).to_bytes());
            libc::freelocale(locale);
            first
        }
    };
    Dates::for_format(month_first)
}

#[derive(Clone, Copy)]
enum Job {
    Plant,
    Water,
    Decorate(Special),
    Untrim,
    Gift(usize),
    Candy,
    Dispose,
    Carry { right: bool },
}

impl Job {
    fn pose(self) -> Activity {
        match self {
            Self::Dispose => Activity::Approach,
            Self::Plant => Activity::Special(Special::Plant),
            Self::Water => Activity::Special(Special::Water),
            Self::Decorate(action) => Activity::Special(action),
            Self::Untrim => Activity::Special(Special::UntrimTree),
            Self::Gift(_) => Activity::Special(Special::OpenGift),
            Self::Candy => Activity::Special(Special::EatCandy),
            Self::Carry { .. } => Activity::Special(Special::CarryTree),
        }
    }
}

#[derive(Clone, Copy)]
struct Work {
    owner: u64,
    job: Job,
    spot: f32,
    walking: bool,
    remaining: f32,
}

struct Spark {
    at: Point,
    speed: Point,
    life: f32,
    kind: usize,
}

struct Flight {
    at: Point,
    velocity: Point,
    age: f32,
}

pub struct FrameInfo {
    pub date: Date,
    pub width: u32,
    pub floor: f32,
    pub scale: u32,
    pub elapsed: f32,
}

pub struct Festive {
    dates: Dates,
    job: Option<Work>,
    choose_in: f32,
    refill_in: f32,
    open_in: f32,
    gifts_visible: bool,
    flight: Option<Flight>,
    tree_at: Option<Point>,
    particles: Vec<Spark>,
    art: Art,
}

impl Festive {
    pub fn new() -> Self {
        Self {
            dates: local_dates(),
            job: None,
            choose_in: 0.0,
            refill_in: 0.0,
            open_in: 9.0,
            gifts_visible: false,
            flight: None,
            tree_at: None,
            particles: Vec::new(),
            art: Art::default(),
        }
    }

    pub fn tick(&mut self, pets: &mut [Instance], world: &mut World, info: FrameInfo) {
        let FrameInfo {
            date,
            width,
            floor,
            scale,
            elapsed,
        } = info;
        if width == 0 {
            return;
        }
        let dt = elapsed.clamp(0.0, 0.05);
        let holiday = self.dates.holiday(date);
        self.gifts_visible = self.dates.gifts_visible(date);
        let season = Dates::season(date);
        if let Some(tree) = world.tree() {
            if season.is_some_and(|year| year > tree.year) {
                *world.tree_mut() = None;
                world.changed();
            } else if holiday && tree.gifts_started && tree.gifts != GIFTS {
                self.refill_in -= dt;
                if self.refill_in <= 0.0 {
                    let slot = (!tree.gifts & GIFTS).trailing_zeros();
                    world.tree_mut().as_mut().unwrap().gifts |= 1 << slot;
                    world.changed();
                    self.refill_in = 11.0 + fastrand::f32() * 17.0;
                }
            }
        }
        self.open_in = (self.open_in - dt).max(0.0);
        self.tree_at = world
            .tree()
            .map(|tree| Point::from_xy(tree.position * width as f32, floor));
        self.advance_job(pets, world, &info, dt);
        self.choose_in -= dt;
        if self.job.is_none() && self.choose_in <= 0.0 {
            let urgent = holiday && world.tree().is_none_or(|tree| !tree.gifts_started);
            self.choose_in = if urgent {
                0.6
            } else {
                2.2 + fastrand::f32() * 1.6
            };
            self.choose(pets, world, date, width as f32);
        }
        if let Some(flight) = &mut self.flight {
            flight.age += dt;
            flight.velocity.y += 520.0 * dt;
            flight.at.x += flight.velocity.x * dt;
            flight.at.y += flight.velocity.y * dt;
            self.tree_at = Some(flight.at);
            if flight.age >= 1.0 {
                self.flight = None;
                self.tree_at = None;
            }
        }
        self.particles.retain_mut(|spark| {
            spark.life -= dt;
            spark.speed.y += 320.0 * dt;
            spark.at.x += spark.speed.x * dt;
            spark.at.y += spark.speed.y * dt;
            spark.life > 0.0 && spark.at.y <= floor + 4.0
        });
        if let Some(tree) = world.tree() {
            let stage = tree.stage(date, self.dates);
            let decorations = if stage == MATURE { tree.decorations } else { 0 };
            self.art.update(scale, stage, decorations);
        }
    }

    fn choose(&mut self, pets: &mut [Instance], world: &mut World, date: Date, width: f32) {
        let holiday = self.dates.holiday(date);
        let (job, spot) = if let Some(tree) = world.tree() {
            let stage = tree.stage(date, self.dates);
            let decorations = self.dates.decorations(date);
            let job = if Dates::season(date).is_none() && date.year >= tree.year {
                Job::Dispose
            } else if Dates::untrim(date) && (tree.decorations > 0 || tree.gifts > 0) {
                Job::Untrim
            } else if holiday && stage < MATURE {
                Job::Water
            } else if stage == MATURE && tree.decorations < decorations {
                Job::Decorate(if holiday || tree.decorations == DECORATED - 1 {
                    Special::StarTree
                } else if tree.decorations == 1 {
                    Special::TinselTree
                } else {
                    Special::Decorate
                })
            } else if holiday
                && stage == MATURE
                && tree.decorations == DECORATED
                && !tree.gifts_started
            {
                let tree = world.tree_mut().as_mut().unwrap();
                tree.gifts_started = true;
                tree.gifts = GIFTS;
                world.save_now();
                return;
            } else if self.gifts_visible && tree.gifts != 0 && self.open_in <= 0.0 {
                Job::Gift(tree.gifts.trailing_zeros() as usize)
            } else if (stage < MATURE || (tree.calendar && stage == MATURE && !Dates::untrim(date)))
                && Dates::season(date) == Some(tree.year)
                && fastrand::u8(0..100) < 12
            {
                Job::Water
            } else {
                return;
            };
            let x = match job {
                Job::Gift(index) => tree.position * width + GIFT_OFFSETS[index],
                _ => tree.position * width,
            };
            (job, x.clamp(0.0, width) / width)
        } else {
            if !Dates::can_plant(date) || self.flight.is_some() {
                return;
            }
            let Some(position) =
                crate::interactions::structure_spot(world.winter(), None, 0.35, 0.65).or_else(
                    || crate::interactions::structure_spot(world.winter(), None, 0.18, 0.82),
                )
            else {
                return;
            };
            (Job::Plant, position)
        };
        let Some(pet) = pets
            .iter_mut()
            .filter(|pet| pet.app.pet.activity.routine() && pet.app.can_be_struck())
            .min_by(|a, b| {
                (a.app.pet.x - spot * width)
                    .abs()
                    .total_cmp(&(b.app.pet.x - spot * width).abs())
            })
        else {
            return;
        };
        pet.app.pet.start(Activity::Approach);
        self.job = Some(Work {
            owner: pet.id,
            job,
            spot,
            walking: true,
            remaining: 0.0,
        });
    }

    fn advance_job(&mut self, pets: &mut [Instance], world: &mut World, info: &FrameInfo, dt: f32) {
        let Some(mut work) = self.job.take() else {
            return;
        };
        let expected = if work.walking {
            Activity::Approach
        } else {
            work.job.pose()
        };
        let Some(pet) = pets
            .iter_mut()
            .find(|pet| pet.id == work.owner && pet.app.pet.activity == expected)
        else {
            return;
        };
        let app = &mut pet.app;
        let date = info.date;
        let holiday = self.dates.holiday(date);
        let x = work.spot * info.width as f32;
        if work.walking {
            let side = if app.pet.x <= x { 1.0 } else { -1.0 };
            let offset = (app.height() * 0.34).clamp(15.0, 66.0);
            if !app.pet.approach(&app.room, x - side * offset, dt) {
                self.job = Some(work);
                return;
            }
            app.pet.face = side;
            if let Job::Dispose = work.job {
                work.job = Job::Carry {
                    right: x >= info.width as f32 * 0.5,
                };
            }
            work.walking = false;
            work.remaining = match work.job {
                Job::Water if !holiday => 3.0 + fastrand::f32() * 2.0,
                _ => 1.4,
            };
            app.pet.start(work.job.pose());
            self.job = Some(work);
            return;
        }
        if let Job::Carry { right } = work.job {
            let goal = if right { app.room.right } else { app.room.left };
            let direction = if right { 1.0 } else { -1.0 };
            let arrived = app.pet.approach_at(&app.room, goal, 108.0, dt);
            app.pet.face = direction;
            let hand = app.snowball_position();
            let at = Point::from_xy(hand.x + direction * 18.0, hand.y + 12.0);
            self.tree_at = Some(at);
            if arrived {
                self.flight = Some(Flight {
                    at,
                    velocity: Point::from_xy(direction * 440.0, -145.0),
                    age: 0.0,
                });
                *world.tree_mut() = None;
                world.save_now();
                app.pet.start(Activity::Idle);
            } else {
                self.job = Some(work);
            }
            return;
        }
        if matches!(work.job, Job::Plant) {
            work.remaining -= dt;
            if work.remaining > 0.0 {
                self.job = Some(work);
                return;
            }
            if let Some(year) = Dates::season(date).filter(|_| Dates::can_plant(date)) {
                if world.tree().is_none() {
                    let calendar =
                        date.month == 11 || date.month == 12 && date.day <= self.dates.mature;
                    *world.tree_mut() = Some(Tree::new(work.spot, year, calendar));
                    world.save_now();
                }
            }
            app.pet.start(Activity::Idle);
            return;
        }
        if world.tree().is_none() {
            app.pet.start(Activity::Idle);
            return;
        }
        let spent = info.elapsed.max(0.0).min(work.remaining);
        if matches!(work.job, Job::Water) {
            if !holiday {
                let tree = world.tree_mut().as_mut().unwrap();
                if !tree.calendar {
                    tree.watered = (tree.watered + spent).min(TREE_SECONDS);
                    world.changed();
                }
            }
            if (app.pet.time / 0.13).floor() > ((app.pet.time - dt) / 0.13).floor() {
                let hand = app.snowball_position();
                self.particles.push(Spark {
                    at: Point::from_xy(hand.x + app.pet.face * app.height() * 0.23, hand.y + 2.0),
                    speed: Point::from_xy(app.pet.face * 44.0, 54.0),
                    life: 0.55,
                    kind: props::FESTIVE_COLORS.len(),
                });
            }
        }
        work.remaining -= spent;
        if work.remaining > 0.0 {
            self.job = Some(work);
            return;
        }
        let tree = world.tree_mut().as_mut().unwrap();
        match work.job {
            Job::Water if holiday => tree.watered = TREE_SECONDS,
            Job::Decorate(_) => {
                tree.decorations = if holiday {
                    DECORATED
                } else {
                    (tree.decorations + 1).min(self.dates.decorations(date))
                };
                if holiday && !tree.gifts_started {
                    tree.gifts_started = true;
                    tree.gifts = GIFTS;
                    self.refill_in = 14.0;
                }
            }
            Job::Untrim => {
                tree.decorations = 0;
                tree.gifts = 0;
            }
            Job::Gift(slot) => {
                tree.gifts &= !(1 << slot);
                self.open_in = 13.0 + fastrand::f32() * 19.0;
                if self.refill_in <= 0.0 {
                    self.refill_in = 11.0 + fastrand::f32() * 17.0;
                }
                self.burst(Point::from_xy(x, info.floor - 22.0));
                if fastrand::u8(0..5) == 0 {
                    work.job = Job::Candy;
                    work.remaining = 2.24;
                    app.pet.start(work.job.pose());
                    self.job = Some(work);
                    world.save_now();
                    return;
                }
            }
            _ => {}
        }
        world.save_now();
        app.pet.start(Activity::Idle);
    }

    fn burst(&mut self, at: Point) {
        for n in 0..22 {
            self.particles.push(Spark {
                at,
                speed: Point::from_xy(
                    (fastrand::f32() - 0.5) * 185.0,
                    -78.0 - fastrand::f32() * 165.0,
                ),
                life: 0.55 + fastrand::f32() * 0.8,
                kind: n % props::FESTIVE_COLORS.len(),
            });
        }
    }

    pub fn images(
        &self,
        tree: Option<Tree>,
        width: u32,
        floor: f32,
    ) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let scale = self.art.scale as f32;
        let at = move |point: Point, image: &OverlayImage| {
            (
                (point.x * scale - image.width as f32 * 0.5).round() as i32,
                (point.y * scale - image.height as f32).round() as i32,
            )
        };
        let tree_img = self
            .tree_at
            .zip(self.art.tree.as_ref())
            .map(move |(point, (_, image))| {
                let (x, y) = at(point, image);
                (x, y, image)
            });
        let gifts = self
            .art
            .gifts
            .iter()
            .enumerate()
            .filter_map(move |(i, image)| {
                let tree = tree?;
                if !self.gifts_visible || tree.gifts & (1 << i) == 0 {
                    return None;
                }
                let (x, y) = at(
                    Point::from_xy(tree.position * width as f32 + GIFT_OFFSETS[i], floor),
                    image,
                );
                Some((x, y, image))
            });
        tree_img
            .into_iter()
            .chain(gifts)
            .chain(self.particles.iter().filter_map(move |spark| {
                let image = self.art.confetti.get(spark.kind)?;
                let (x, y) = at(spark.at, image);
                Some((x, y, image))
            }))
    }
}

#[derive(Default)]
struct Art {
    scale: u32,
    tree: Option<((u8, u8), OverlayImage)>,
    gifts: Vec<OverlayImage>,
    confetti: Vec<OverlayImage>,
}

impl Art {
    fn update(&mut self, scale: u32, stage: u8, decorations: u8) {
        if self.scale != scale {
            self.scale = scale;
            self.tree = None;
            self.gifts = (0..GIFT_OFFSETS.len())
                .map(|i| props::gift(i, scale).into())
                .collect();
            self.confetti = (0..props::FESTIVE_COLORS.len())
                .map(|i| props::confetti(i, scale).into())
                .collect();
            self.confetti.push(props::water_drop(scale).into());
        }
        if self
            .tree
            .as_ref()
            .is_none_or(|(key, _)| *key != (stage, decorations))
        {
            self.tree = Some((
                (stage, decorations),
                props::tree(stage, decorations, scale).into(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, config::Settings, pack::Pack, startup_room};

    #[test]
    fn the_tree_and_its_decorations_follow_the_local_calendar() {
        assert!(month_first(b"%m/%d/%Y"));
        assert!(!month_first(b"%Od.%Om.%Y"));
        assert!(!month_first(b"%Y-%m-%d"));
        for (dates, end, star, gift) in [
            (Dates::for_format(true), 10, 18, 25),
            (Dates::for_format(false), 15, 24, 31),
        ] {
            let tree = Tree {
                position: 0.5,
                year: 2028,
                calendar: true,
                watered: 0.0,
                decorations: 0,
                gifts_started: false,
                gifts: 0,
            };
            assert_eq!(tree.stage(Date::new(30, 11, 2027), dates), 0);
            assert_eq!(tree.stage(Date::new(1, 12, 2027), dates), 1);
            assert!(tree.stage(Date::new(end - 1, 12, 2027), dates) < MATURE);
            assert_eq!(tree.stage(Date::new(end, 12, 2027), dates), MATURE);
            assert_eq!(
                dates.decorations(Date::new(star - 1, 12, 2027)),
                DECORATED - 1
            );
            assert_eq!(dates.decorations(Date::new(star, 12, 2027)), DECORATED);
            assert!(dates.holiday(Date::new(gift, 12, 2027)));
        }
        assert!(Dates::can_plant(Date::new(1, 1, 2028)));
        assert!(!Dates::can_plant(Date::new(2, 1, 2028)));
        assert!(!Dates::untrim(Date::new(13, 1, 2028)));
        assert!(Dates::untrim(Date::new(14, 1, 2028)));
        assert_eq!(Dates::season(Date::new(29, 2, 2028)), Some(2028));
        assert_eq!(Dates::season(Date::new(1, 3, 2028)), None);
    }

    #[test]
    fn a_same_day_planting_gets_presents_after_one_watering_and_decoration() {
        let mut festive = Festive::new();
        festive.dates = Dates::for_format(true);
        festive.open_in = 100.0;
        let mut world = World::test("bear-girl");
        let mut pets = [Instance {
            id: 0,
            app: App::new(
                startup_room(1000.0, 650.0),
                Settings::default(),
                Pack::builtin(),
            ),
        }];
        let mut step = |festive: &mut Festive, world: &mut World, date| {
            pets[0].app.tick(0.05, true);
            festive.tick(
                &mut pets,
                world,
                FrameInfo {
                    date,
                    width: 1000,
                    floor: 650.0,
                    scale: 1,
                    elapsed: 0.05,
                },
            );
        };
        let holiday = Date::new(25, 12, 2027);
        let mut waterings = 0;
        let mut decorations = 0;
        for _ in 0..300 {
            let previous = world.tree();
            step(&mut festive, &mut world, holiday);
            if let Some((before, after)) = previous.zip(world.tree()) {
                waterings += u8::from(before.watered != after.watered);
                decorations += u8::from(before.decorations != after.decorations);
                if after.gifts == GIFTS {
                    break;
                }
            }
        }
        let tree = world.tree().unwrap();
        assert_eq!((waterings, decorations), (1, 1));
        assert!(!tree.calendar);
        assert_eq!(
            (tree.watered, tree.decorations, tree.gifts),
            (TREE_SECONDS, DECORATED, GIFTS)
        );

        world.tree_mut().as_mut().unwrap().gifts = 1;
        festive.refill_in = 0.0;
        step(&mut festive, &mut world, Date::new(26, 12, 2027));
        assert_eq!(world.tree().unwrap().gifts, 1);
        step(&mut festive, &mut world, holiday);
        assert_eq!(world.tree().unwrap().gifts, 3);

        for _ in 0..300 {
            step(&mut festive, &mut world, Date::new(14, 1, 2028));
        }
        assert_eq!(world.tree().unwrap().decorations, 0);
        step(&mut festive, &mut world, Date::new(29, 2, 2028));
        assert!(world.tree().is_some());

        for _ in 0..400 {
            step(&mut festive, &mut world, Date::new(1, 3, 2028));
            if festive.flight.is_some() {
                break;
            }
        }
        assert!(festive.flight.is_some());
        assert!(world.tree().is_none());
    }
}
