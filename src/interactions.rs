// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use tiny_skia::{Point, Rect};

use crate::festive::Tree;
use crate::game::{swept_contact, Instance};
use crate::props::{self, SculptStage};
use crate::ramps::{self, Cover};
use crate::season::OverlayImage;
use crate::sim::{Activity, Special};
use crate::snowman::{Fall, Fragment, Grab, Motion};
use crate::world::{
    Pile, SnowWall, Snowman, WinterObjects, World, MAX_SNOWMEN, MAX_SNOW_WALLS, SNOWMAN_SECONDS,
};

pub const RELEASE_AT: f32 = 0.44;
const PACK_FOR: f32 = 0.87;
const THROW_FOR: f32 = 0.76;
const BALL_GRAVITY: f32 = 750.0;
const BALL_RADIUS: f32 = 11.0;
const BALL_KNOCKBACK: f32 = 0.68 * 2.0;
const HIT_WINDOW: f64 = 30.0;
const CANNON_DURATION: f32 = 2.5;
const COVER_MAX_DISTANCE: f32 = 300.0;
const COVER_AFTER_BLOCK: f64 = 2.0;

#[derive(Clone, Copy)]
enum Task {
    ApproachPile {
        position: f32,
        target: u64,
    },
    Pack {
        target: u64,
    },
    Aim {
        target: u64,
        remaining: f32,
    },
    Throw {
        target: u64,
        released: bool,
    },
    ApproachSnowman {
        slot: usize,
    },
    Sculpt {
        slot: usize,
        remaining: f32,
    },
    ApproachWall {
        wall: usize,
    },
    BuildWall {
        wall: usize,
        remaining: f32,
    },
    Cannon {
        remaining: f32,
        next_shot: f32,
        cursor: usize,
    },
}

impl Task {
    fn activity(self) -> Activity {
        match self {
            Self::ApproachPile { .. }
            | Self::ApproachSnowman { .. }
            | Self::ApproachWall { .. }
            | Self::Aim { .. } => Activity::Approach,
            Self::Pack { .. } => Activity::PackSnow,
            Self::Throw { .. } => Activity::ThrowSnow,
            Self::Sculpt { .. } | Self::BuildWall { .. } => Activity::Sculpt,
            Self::Cannon { .. } => Activity::Special(Special::Cannon),
        }
    }

    fn snowman_slot(self) -> Option<usize> {
        match self {
            Self::ApproachSnowman { slot } | Self::Sculpt { slot, .. } => Some(slot),
            _ => None,
        }
    }

    fn wall_slot(self) -> Option<usize> {
        match self {
            Self::ApproachWall { wall } | Self::BuildWall { wall, .. } => Some(wall),
            _ => None,
        }
    }
}

struct Conversation {
    pair: [u64; 2],
    elapsed: f32,
    duration: f32,
}

struct Snowball {
    owner: u64,
    position: Point,
    vx: f32,
    vy: f32,
    counter: Option<u64>,
}

#[derive(Clone, Copy)]
enum Guard {
    Run {
        wall: usize,
        side: f32,
        hidden: bool,
    },
    Leave {
        wall: usize,
        side: f32,
    },
    Parry,
}

struct Defence {
    attacker: u64,
    guard: Guard,
    hold_until: f64,
}

pub struct Interactions {
    tasks: HashMap<u64, Task>,
    retaliations: HashMap<u64, u64>,
    counters: HashMap<u64, u64>,
    defences: HashMap<u64, Defence>,
    snow_hits: HashMap<u64, VecDeque<f64>>,
    owes_cannon: HashSet<u64>,
    time: f64,
    conversations: Vec<Conversation>,
    balls: Vec<Snowball>,
    fragments: Vec<Fragment>,
    pub snowman_held: [bool; MAX_SNOWMEN],
    snowman_cooldown: [f32; MAX_SNOWMEN],
    wall_motion: [Motion; MAX_SNOW_WALLS],
    choose_in: f32,
    pile_in: f32,
}

impl Interactions {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            retaliations: HashMap::new(),
            counters: HashMap::new(),
            defences: HashMap::new(),
            snow_hits: HashMap::new(),
            owes_cannon: HashSet::new(),
            time: 0.0,
            conversations: Vec::new(),
            balls: Vec::new(),
            fragments: Vec::new(),
            snowman_held: [false; MAX_SNOWMEN],
            snowman_cooldown: [0.0; MAX_SNOWMEN],
            wall_motion: Default::default(),
            choose_in: 2.0,
            pile_in: 3.0,
        }
    }

    pub fn clear_winter(&mut self, pets: &mut [Instance]) {
        for pet in pets {
            if self
                .tasks
                .get(&pet.id)
                .is_some_and(|task| task.activity() == pet.app.pet.activity)
                || self.defences.contains_key(&pet.id) && defending(pet.app.pet.activity)
            {
                pet.app.pet.start(Activity::Idle);
            }
            pet.app.cover = None;
        }
        self.tasks.clear();
        self.retaliations.clear();
        self.counters.clear();
        self.defences.clear();
        self.snow_hits.clear();
        self.owes_cannon.clear();
        self.time = 0.0;
        self.balls.clear();
        self.fragments.clear();
        self.snowman_held.fill(false);
        self.snowman_cooldown.fill(0.0);
        self.wall_motion = Default::default();
        self.pile_in = 3.0;
    }

    pub fn tick(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        width: u32,
        floor: f32,
        elapsed: f32,
        winter: bool,
    ) {
        let dt = elapsed.clamp(0.0, 0.05);
        self.time += f64::from(elapsed.max(0.0));
        self.talk(pets, elapsed);
        for cooldown in &mut self.snowman_cooldown {
            *cooldown = (*cooldown - elapsed).max(0.0);
        }
        self.fragments
            .retain_mut(|fragment| fragment.tick(dt, floor));
        if winter && width > 0 {
            self.drop_pile(world, dt);
            self.start_cannons(pets);
            self.retaliate(pets, world.winter(), width as f32);
            self.tasks
                .retain(|id, _| pets.iter().any(|pet| pet.id == *id));
            for index in 0..pets.len() {
                let id = pets[index].id;
                if let Some(task) = self.tasks.remove(&id) {
                    if pets[index].app.pet.activity == task.activity() {
                        if task
                            .snowman_slot()
                            .is_some_and(|slot| self.snowman_held[slot])
                            || task
                                .wall_slot()
                                .is_some_and(|slot| !self.wall_grounded(slot))
                        {
                            self.tasks.insert(id, task);
                        } else if let Some(next) =
                            self.advance(task, index, pets, world, width as f32, dt, elapsed)
                        {
                            self.tasks.insert(id, next);
                        } else if !pets[index].app.pet.activity.routine() {
                            pets[index].app.pet.start(Activity::Idle);
                        }
                    }
                }
            }
            self.counters.retain(|id, _| self.tasks.contains_key(id));
            self.defend(pets, world.winter(), width as f32, floor, dt);
            self.fly(pets, &world.winter().walls, width as f32, floor, dt);
        }
        self.choose_in -= dt;
        if self.choose_in <= 0.0 {
            self.choose_in = 2.2 + fastrand::f32() * 1.6;
            self.choose(pets, world, width as f32, winter, fastrand::u8(0..100));
        }
    }

    fn retaliate(&mut self, pets: &mut [Instance], objects: &WinterObjects, width: f32) {
        self.retaliations.retain(|victim, thrower| {
            pets.iter().any(|pet| pet.id == *victim) && pets.iter().any(|pet| pet.id == *thrower)
        });
        for pet in pets {
            let Some(&target) = self.retaliations.get(&pet.id) else {
                continue;
            };
            if !pet.app.pet.activity.routine() || !pet.app.can_be_struck() {
                continue;
            }
            let from = pet.app.pet.x / width;
            if let Some(position) = nearest_pile(objects, from) {
                let task = Task::ApproachPile { position, target };
                pet.app.pet.start(task.activity());
                self.tasks.insert(pet.id, task);
                self.retaliations.remove(&pet.id);
                self.counters.insert(pet.id, target);
            }
        }
    }

    fn start_cannons(&mut self, pets: &mut [Instance]) {
        self.owes_cannon
            .retain(|id| pets.iter().any(|pet| pet.id == *id));
        self.snow_hits.retain(|id, hits| {
            hits.retain(|time| self.time - time <= HIT_WINDOW);
            !hits.is_empty() && pets.iter().any(|pet| pet.id == *id)
        });
        if pets.len() < 2 {
            return;
        }
        for pet in pets {
            if !pet.app.pet.activity.routine() || !self.owes_cannon.remove(&pet.id) {
                continue;
            }
            self.retaliations.remove(&pet.id);
            let task = Task::Cannon {
                remaining: CANNON_DURATION,
                next_shot: 0.18,
                cursor: 0,
            };
            pet.app.pet.start(task.activity());
            self.tasks.insert(pet.id, task);
        }
    }

    fn defend(
        &mut self,
        pets: &mut [Instance],
        objects: &WinterObjects,
        width: f32,
        floor: f32,
        dt: f32,
    ) {
        if self.defences.is_empty()
            && self.counters.is_empty()
            && !self.balls.iter().any(|ball| ball.counter.is_some())
        {
            return;
        }
        let mut threats: Vec<_> = self
            .balls
            .iter()
            .filter_map(|ball| Some((ball.owner, ball.counter?)))
            .chain(
                self.counters
                    .iter()
                    .map(|(&attacker, &target)| (attacker, target)),
            )
            .collect();
        threats.sort_unstable();
        threats.dedup();
        let grounded = std::array::from_fn(|slot| self.wall_grounded(slot));
        for index in 0..pets.len() {
            let id = pets[index].id;
            let threat = threats.iter().find_map(|&(attacker, target)| {
                (target == id)
                    .then(|| {
                        pets.iter()
                            .find(|pet| pet.id == attacker)
                            .map(|pet| (attacker, pet.app.pet.x))
                    })
                    .flatten()
            });
            let app = &mut pets[index].app;
            app.cover = None;
            if let Some((attacker, from)) =
                threat.filter(|_| app.can_be_struck() && !self.counters.contains_key(&id))
            {
                let defence = self.defences.entry(id).or_insert_with(|| Defence {
                    attacker,
                    guard: cover_for(app, &objects.walls, &grounded, width, from),
                    hold_until: 0.0,
                });
                let reconsider = match defence.guard {
                    Guard::Run { wall, side, hidden } => {
                        !grounded[wall]
                            || (!hidden
                                && (objects.walls[wall].position * width + side * 23.0 - app.pet.x)
                                    .abs()
                                    > COVER_MAX_DISTANCE)
                    }
                    Guard::Leave { .. } => true,
                    Guard::Parry => matches!(
                        cover_for(app, &objects.walls, &grounded, width, from),
                        Guard::Run { .. }
                    ),
                };
                if defence.attacker != attacker || reconsider {
                    defence.attacker = attacker;
                    defence.guard = cover_for(app, &objects.walls, &grounded, width, from);
                }
                match &mut defence.guard {
                    Guard::Run { wall, side, hidden } => {
                        let x = objects.walls[*wall].position * width;
                        let goal = x + *side * 23.0;
                        if (app.pet.x - goal.clamp(app.room.left, app.room.right)).abs() > 1.0 {
                            *hidden = false;
                        }
                        if !*hidden {
                            if app.pet.activity != Activity::Special(Special::RunCover) {
                                self.tasks.remove(&id);
                                app.pet.start(Activity::Special(Special::RunCover));
                            }
                            *hidden = app.pet.approach_at(&app.room, goal, 258.0, dt);
                            if *hidden {
                                app.pet.face = -*side;
                                app.pet.start(Activity::Special(Special::Hide));
                            }
                        }
                        app.cover = wall_bounds(objects.walls[*wall], width, floor);
                    }
                    Guard::Leave { .. } => unreachable!("active defence replaced exit"),
                    Guard::Parry => {
                        if matches!(
                            app.pet.activity,
                            Activity::Special(
                                Special::Hide | Special::RunCover | Special::LeaveCover
                            )
                        ) {
                            app.pet.start(Activity::Idle);
                        }
                        let nearby = self.balls.iter().any(|ball| {
                            ball.counter == Some(id)
                                && ball.owner == attacker
                                && (ball.position.x - app.pet.x).abs() < 160.0
                                && (ball.position.y - (app.pet.y - app.height() * 0.48)).abs()
                                    < app.height()
                        });
                        if nearby && app.pet.activity != Activity::Special(Special::Parry) {
                            self.tasks.remove(&id);
                            app.pet.face = if from < app.pet.x { -1.0 } else { 1.0 };
                            app.pet.start(Activity::Special(Special::Parry));
                        }
                    }
                }
            } else if let Some(defence) = self.defences.get_mut(&id) {
                match defence.guard {
                    Guard::Run {
                        wall, hidden: true, ..
                    } if self.time < defence.hold_until
                        && grounded[wall]
                        && app.pet.activity == Activity::Special(Special::Hide) =>
                    {
                        app.cover = wall_bounds(objects.walls[wall], width, floor);
                        continue;
                    }
                    Guard::Run { wall, side, .. } | Guard::Leave { wall, side }
                        if defending(app.pet.activity) && grounded[wall] =>
                    {
                        let object = objects.walls[wall];
                        defence.guard = Guard::Leave { wall, side };
                        app.cover = wall_bounds(object, width, floor);
                        if app.pet.activity != Activity::Special(Special::LeaveCover) {
                            app.pet.start(Activity::Special(Special::LeaveCover));
                        }
                        let depth = if (side > 0.0) == object.slope_right {
                            ramps::RUN
                        } else {
                            0.0
                        };
                        let exit = object.position * width + side * (ramps::HALF + depth + 38.0);
                        if !app.pet.approach_at(&app.room, exit, 155.0, dt) {
                            continue;
                        }
                        app.cover = None;
                    }
                    _ => {}
                }
                self.defences.remove(&id);
                if defending(app.pet.activity) {
                    app.pet.start(Activity::Idle);
                }
            }
        }
        self.defences
            .retain(|id, _| pets.iter().any(|pet| pet.id == *id));
    }

    fn talk(&mut self, pets: &mut [Instance], elapsed: f32) {
        self.conversations.retain_mut(|chat| {
            let active = |activity| matches!(activity, Activity::Talk | Activity::Listen);
            chat.elapsed += elapsed;
            let both = chat.pair.iter().all(|id| {
                pets.iter()
                    .any(|pet| pet.id == *id && active(pet.app.pet.activity))
            });
            let speaking = (chat.elapsed / 1.1).floor() as usize % 2;
            let ongoing = both && chat.elapsed < chat.duration;
            for pet in pets.iter_mut().filter(|pet| chat.pair.contains(&pet.id)) {
                if active(pet.app.pet.activity) {
                    let pose = if !ongoing {
                        Activity::Idle
                    } else if pet.id == chat.pair[speaking] {
                        Activity::Talk
                    } else {
                        Activity::Listen
                    };
                    if pet.app.pet.activity != pose {
                        pet.app.pet.start(pose);
                    }
                }
            }
            ongoing
        });
    }

    fn choose(
        &mut self,
        pets: &mut [Instance],
        world: &mut World,
        width: f32,
        winter: bool,
        roll: u8,
    ) {
        let resting: Vec<_> = pets
            .iter()
            .enumerate()
            .filter_map(|(i, pet)| {
                (pet.app.pet.activity.routine() && pet.app.can_be_struck()).then_some(i)
            })
            .collect();
        if resting.is_empty() {
            return;
        }
        match roll {
            0..14 => {
                for &i in &resting {
                    for &j in resting.iter().filter(|&&j| j > i) {
                        let distance = (pets[i].app.pet.x - pets[j].app.pet.x).abs();
                        let size = (pets[i].app.height() + pets[j].app.height()) * 0.5;
                        if ((size * 0.34).max(16.0)..=(size * 1.47).clamp(45.0, 190.0))
                            .contains(&distance)
                        {
                            self.start_talk(pets, [i, j]);
                            return;
                        }
                    }
                }
            }
            14..38 if winter && width > 0.0 => {
                let index = resting[fastrand::usize(..resting.len())];
                self.start_building(&mut pets[index], world);
            }
            38..57 if winter && pets.len() >= 2 && width > 0.0 => {
                let i = resting[fastrand::usize(..resting.len())];
                let from = pets[i].app.pet.x / width;
                let Some(position) = nearest_pile(world.winter(), from) else {
                    return;
                };
                let from = position * width;
                let target = pets
                    .iter()
                    .enumerate()
                    .filter_map(|(j, target)| {
                        let gap = (target.app.pet.x - from).abs();
                        let climbing = target.app.pet.activity == Activity::ClimbWall;
                        let limit = if climbing { 1100.0 } else { 600.0 };
                        (j != i
                            && (target.app.pet.activity.routine() || climbing)
                            && target.app.can_be_struck()
                            && (40.0..=limit).contains(&gap))
                        .then_some((j, if climbing { 4 } else { 1 }))
                    })
                    .collect::<Vec<_>>();
                let total = target.iter().map(|(_, weight)| weight).sum::<u32>();
                let selected = (total > 0).then(|| fastrand::u32(0..total));
                if let Some(j) = selected.and_then(|mut roll| {
                    target.into_iter().find_map(|(index, weight)| {
                        if roll < weight {
                            Some(index)
                        } else {
                            roll -= weight;
                            None
                        }
                    })
                }) {
                    let task = Task::ApproachPile {
                        position,
                        target: pets[j].id,
                    };
                    pets[i].app.pet.start(task.activity());
                    self.tasks.insert(pets[i].id, task);
                }
            }
            57..81 if winter && width > 0.0 => {
                let index = resting[fastrand::usize(..resting.len())];
                self.start_wall(&mut pets[index], world);
            }
            _ => {}
        }
    }

    fn start_talk(&mut self, pets: &mut [Instance], [i, j]: [usize; 2]) {
        let direction = (pets[j].app.pet.x - pets[i].app.pet.x).signum();
        pets[i].app.pet.face = direction;
        pets[j].app.pet.face = -direction;
        pets[i].app.pet.start(Activity::Talk);
        pets[j].app.pet.start(Activity::Listen);
        self.conversations.push(Conversation {
            pair: [pets[i].id, pets[j].id],
            elapsed: 0.0,
            duration: 3.5 + fastrand::f32() * 1.8,
        });
    }

    fn start_building(&mut self, pet: &mut Instance, world: &mut World) {
        let ready = |slot: usize| {
            !self.snowman_held[slot]
                && self.snowman_cooldown[slot] <= 0.0
                && !self
                    .tasks
                    .values()
                    .any(|task| task.snowman_slot() == Some(slot))
        };
        let slot = if let Some((slot, _)) = world
            .winter()
            .snowmen()
            .find(|&(slot, man)| man.seconds < SNOWMAN_SECONDS && ready(slot))
        {
            slot
        } else {
            let Some(slot) = (0..MAX_SNOWMEN)
                .find(|&slot| world.winter().snowman(slot).is_none() && ready(slot))
            else {
                return;
            };
            let Some(position) = structure_spot(world.winter(), world.tree(), 0.30, 0.70)
                .or_else(|| structure_spot(world.winter(), world.tree(), 0.19, 0.81))
            else {
                return;
            };
            *world.winter_mut().snowman_mut(slot) = Some(Snowman {
                position,
                seconds: 0.0,
            });
            world.save_now();
            slot
        };
        pet.app.pet.start(Activity::Approach);
        self.tasks.insert(pet.id, Task::ApproachSnowman { slot });
    }

    fn start_wall(&mut self, pet: &mut Instance, world: &mut World) {
        if self.tasks.values().any(|task| task.wall_slot().is_some()) {
            return;
        }
        let walls = &world.winter().walls;
        let index = if let Some(i) = walls.iter().position(|wall| !wall.finished()) {
            i
        } else {
            if !walls.is_empty() {
                return;
            }
            let Some(position) = structure_spot(world.winter(), world.tree(), 0.19, 0.81) else {
                return;
            };
            world.winter_mut().walls.push(SnowWall {
                slope_right: fastrand::bool(),
                position,
                seconds: 0.0,
            });
            world.save_now();
            0
        };
        if !self.wall_grounded(index) {
            return;
        }
        let task = Task::ApproachWall { wall: index };
        pet.app.pet.start(task.activity());
        self.tasks.insert(pet.id, task);
    }

    pub fn wall_impact(&mut self, slot: usize, dt: f32) -> Option<f32> {
        let Motion::Falling(fall) = &mut self.wall_motion[slot] else {
            return None;
        };
        let speed = fall.step(dt)?;
        self.wall_motion[slot] = Motion::Grounded;
        (speed >= Fall::BREAK_SPEED).then_some(speed)
    }

    pub fn remove_wall(
        &mut self,
        pets: &mut [Instance],
        slot: usize,
        at: Point,
        speed: f32,
        height: f32,
    ) {
        self.wall_motion[slot..].rotate_left(1);
        self.wall_motion[MAX_SNOW_WALLS - 1] = Motion::Grounded;
        self.tasks.retain(|&id, task| {
            let activity = task.activity();
            let wall = match task {
                Task::ApproachWall { wall } | Task::BuildWall { wall, .. } => wall,
                _ => return true,
            };
            if *wall == slot {
                if let Some(pet) = pets
                    .iter_mut()
                    .find(|pet| pet.id == id && pet.app.pet.activity == activity)
                {
                    pet.app.pet.start(Activity::Idle);
                }
                false
            } else {
                *wall -= usize::from(*wall > slot);
                true
            }
        });
        self.defences.retain(|&id, defence| {
            let wall = match &mut defence.guard {
                Guard::Run { wall, .. } | Guard::Leave { wall, .. } => wall,
                Guard::Parry => return true,
            };
            if *wall == slot {
                if let Some(pet) = pets.iter_mut().find(|pet| pet.id == id) {
                    pet.app.cover = None;
                    if defending(pet.app.pet.activity) {
                        pet.app.pet.start(Activity::Idle);
                    }
                }
                false
            } else {
                *wall -= usize::from(*wall > slot);
                true
            }
        });
        Fragment::scatter(
            &mut self.fragments,
            at,
            Point::from_xy(-210.0, speed),
            height,
        );
        Fragment::scatter(
            &mut self.fragments,
            at,
            Point::from_xy(210.0, speed),
            height,
        );
    }

    pub fn wall_grounded(&self, slot: usize) -> bool {
        matches!(self.wall_motion[slot], Motion::Grounded)
    }

    pub fn wall_lift(&self, slot: usize) -> f32 {
        self.wall_motion[slot].lift()
    }

    pub fn grabbed_wall(&self) -> Option<(usize, Point)> {
        self.wall_motion
            .iter()
            .enumerate()
            .find_map(|(slot, motion)| {
                if let Motion::Held { grab, .. } = motion {
                    Some((slot, grab.cursor()))
                } else {
                    None
                }
            })
    }

    pub fn grab_wall(&mut self, pets: &mut [Instance], slot: usize, grab: Grab) {
        let lift = self.wall_lift(slot);
        self.wall_motion[slot] = Motion::Held { grab, lift };
        for pet in pets {
            let Some(task) = self.tasks.get_mut(&pet.id) else {
                continue;
            };
            if task.wall_slot() == Some(slot) && pet.app.pet.activity == task.activity() {
                *task = Task::ApproachWall { wall: slot };
                pet.app.pet.start(task.activity());
            }
        }
    }

    pub fn drag_wall(
        &mut self,
        wall: &mut SnowWall,
        slot: usize,
        point: Point,
        now: Instant,
        room: (f32, f32, f32),
    ) {
        let (width, floor, height) = room;
        if let Motion::Held { grab, lift } = &mut self.wall_motion[slot] {
            let base = grab.move_to(point, now);
            let ladder_span = ramps::HALF + 15.0;
            let slope_span = ramps::HALF + ramps::RUN;
            let (left, right) = if !wall.finished() {
                (ramps::HALF, ramps::HALF)
            } else if wall.slope_right {
                (ladder_span, slope_span)
            } else {
                (slope_span, ladder_span)
            };
            wall.position =
                (base.x / width).clamp((left / width).min(0.5), 1.0 - (right / width).min(0.5));
            *lift = (floor - base.y).clamp(0.0, (floor - height).max(0.0));
        }
    }

    pub fn release_wall(&mut self, world: &mut World, slot: usize) {
        let lift = self.wall_lift(slot);
        self.wall_motion[slot] = if lift > 0.0 {
            Motion::Falling(Fall::new(lift))
        } else {
            Motion::Grounded
        };
        world.save_now();
    }

    pub fn release_snowman(&mut self, pets: &mut [Instance], slot: usize, broken: bool) {
        self.snowman_held[slot] = false;
        for pet in pets {
            if let Some(task) = self
                .tasks
                .get_mut(&pet.id)
                .filter(|task| task.snowman_slot() == Some(slot))
            {
                if pet.app.pet.activity == task.activity() {
                    if broken {
                        pet.app.pet.start(Activity::Idle);
                    } else {
                        *task = Task::ApproachSnowman { slot };
                        pet.app.pet.start(task.activity());
                    }
                }
            }
        }
        if broken {
            self.tasks
                .retain(|_, task| task.snowman_slot() != Some(slot));
        }
    }

    pub fn break_snowman(
        &mut self,
        pets: &mut [Instance],
        slot: usize,
        at: Point,
        velocity: Point,
        height: f32,
    ) {
        self.release_snowman(pets, slot, true);
        self.snowman_cooldown[slot] = 7.0;
        Fragment::scatter(&mut self.fragments, at, velocity, height);
        for pet in pets {
            pet.app.mourn();
        }
    }

    fn drop_pile(&mut self, world: &mut World, dt: f32) {
        self.pile_in -= dt;
        if self.pile_in > 0.0 {
            return;
        }
        self.pile_in = if world.winter().piles.is_empty() {
            7.0
        } else {
            17.0 + 17.0 * fastrand::f32()
        };
        if world.winter().piles.len() < 5 {
            if let Some(position) = free_spot(world.winter(), world.tree(), 0.17, 0.83) {
                world.winter_mut().piles.push(Pile {
                    position,
                    servings: fastrand::u8(2..=5),
                });
                world.save_now();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn advance(
        &mut self,
        task: Task,
        index: usize,
        pets: &mut [Instance],
        world: &mut World,
        width: f32,
        dt: f32,
        elapsed: f32,
    ) -> Option<Task> {
        let id = pets[index].id;
        let target_state = match task {
            Task::Pack { target } | Task::Aim { target, .. } | Task::Throw { target, .. } => {
                let other = &pets.iter().find(|pet| pet.id == target)?.app;
                Some((
                    other.pet.x,
                    other.can_be_struck(),
                    other.pet.activity == Activity::ClimbWall,
                ))
            }
            _ => None,
        };
        let app = &mut pets[index].app;
        let next = match task {
            Task::ApproachPile { position, target } => {
                let pile = world
                    .winter()
                    .piles
                    .iter()
                    .position(|pile| pile.position == position)?;
                let x = position * width;
                let offset = (app.height() * 0.18).clamp(8.0, 36.0);
                let side = if app.pet.x <= x { 1.0 } else { -1.0 };
                let goal = x - side * offset;
                if !app.pet.approach(&app.room, goal, dt) {
                    return Some(task);
                }
                app.pet.face = side;
                let piles = &mut world.winter_mut().piles;
                piles[pile].servings -= 1;
                if piles[pile].servings == 0 {
                    piles.remove(pile);
                }
                world.save_now();
                Task::Pack { target }
            }
            Task::Pack { target: id } => {
                if app.pet.time < PACK_FOR {
                    return Some(task);
                }
                let (target_x, vulnerable, climbing) = target_state?;
                let gap = target_x - app.pet.x;
                if vulnerable && (38.0..=throw_range(climbing)).contains(&gap.abs()) {
                    app.pet.face = gap.signum();
                    Task::Throw {
                        target: id,
                        released: false,
                    }
                } else {
                    Task::Aim {
                        target: id,
                        remaining: 30.0,
                    }
                }
            }
            Task::Aim {
                target: id,
                remaining,
            } => {
                let (target_x, vulnerable, climbing) = target_state?;
                let gap = target_x - app.pet.x;
                if remaining <= 0.0 {
                    return None;
                }
                if vulnerable && (38.0..=throw_range(climbing)).contains(&gap.abs()) {
                    app.pet.face = gap.signum();
                    Task::Throw {
                        target: id,
                        released: false,
                    }
                } else {
                    let side = if target_x - 110.0 < app.room.left {
                        1.0
                    } else if target_x + 110.0 > app.room.right || gap > 0.0 {
                        -1.0
                    } else {
                        1.0
                    };
                    app.pet.approach(&app.room, target_x + side * 110.0, dt);
                    return Some(Task::Aim {
                        target: id,
                        remaining: remaining - elapsed,
                    });
                }
            }
            Task::Throw { target, released } => {
                if app.pet.time >= THROW_FOR {
                    return None;
                }
                if released || app.pet.time < RELEASE_AT {
                    return Some(task);
                }
                let gap = target_state?.0 - app.pet.x;
                if gap.abs() > 0.01 {
                    app.pet.face = gap.signum();
                }
                let origin = app.snowball_position();
                let target_app = &pets.iter().find(|pet| pet.id == target)?.app;
                let counter = self.counters.remove(&id);
                if let Some(mut ball) = Snowball::towards(id, origin, target_app, false) {
                    if counter == Some(target) {
                        ball.counter = counter;
                    }
                    self.balls.push(ball);
                }
                return Some(Task::Throw {
                    target,
                    released: true,
                });
            }
            Task::ApproachSnowman { slot } => {
                let man = world.winter().snowman(slot)?;
                let x = man.position * width;
                let offset = (app.height() * 0.29).clamp(14.0, 54.0);
                let side = if app.pet.x <= x { 1.0 } else { -1.0 };
                if !app.pet.approach(&app.room, x - side * offset, dt) {
                    return Some(task);
                }
                app.pet.face = side;
                app.sculpt_stage = SculptStage::for_step(snowman_step(man.seconds));
                Task::Sculpt {
                    slot,
                    remaining: 3.0 + 2.0 * fastrand::f32(),
                }
            }
            Task::Sculpt { slot, remaining } => {
                let man = world.winter_mut().snowman_mut(slot).as_mut()?;
                let spent = elapsed.min(remaining).min(SNOWMAN_SECONDS - man.seconds);
                man.seconds += spent;
                app.sculpt_stage = SculptStage::for_step(snowman_step(man.seconds));
                let finished = man.seconds >= SNOWMAN_SECONDS || spent >= remaining;
                if spent > 0.0 {
                    if finished {
                        world.save_now();
                    } else {
                        world.changed();
                    }
                }
                return (!finished).then_some(Task::Sculpt {
                    slot,
                    remaining: remaining - spent,
                });
            }
            Task::ApproachWall { wall } => {
                let x = world.winter().walls.get(wall)?.position * width;
                let side = if app.pet.x <= x { 1.0 } else { -1.0 };
                let offset = (app.height() * 0.36).clamp(39.0, 65.0);
                if !app.pet.approach(&app.room, x - side * offset, dt) {
                    return Some(task);
                }
                app.pet.face = side;
                app.sculpt_stage = wall_pose(world.winter().walls[wall].seconds);
                Task::BuildWall {
                    wall,
                    remaining: 3.0 + fastrand::f32() * 2.0,
                }
            }
            Task::BuildWall { wall, remaining } => {
                let wall_progress = world.winter_mut().walls.get_mut(wall)?;
                let spent = elapsed
                    .min(remaining)
                    .min(SNOWMAN_SECONDS - wall_progress.seconds);
                wall_progress.seconds += spent;
                app.sculpt_stage = wall_pose(wall_progress.seconds);
                let finished = wall_progress.finished() || spent >= remaining;
                if spent > 0.0 {
                    if finished {
                        world.save_now();
                    } else {
                        world.changed();
                    }
                }
                return (!finished).then_some(Task::BuildWall {
                    wall,
                    remaining: remaining - spent,
                });
            }
            Task::Cannon {
                remaining,
                next_shot,
                cursor,
            } => {
                return self.cannon(index, pets, remaining - dt, next_shot - dt, cursor);
            }
        };
        pets[index].app.pet.start(next.activity());
        Some(next)
    }

    fn cannon(
        &mut self,
        shooter: usize,
        pets: &mut [Instance],
        remaining: f32,
        mut next_shot: f32,
        mut cursor: usize,
    ) -> Option<Task> {
        if remaining <= 0.0 {
            return None;
        }
        if next_shot <= 0.0 {
            let target = (0..pets.len())
                .map(|offset| (cursor + offset) % pets.len())
                .find(|&index| index != shooter);
            let side = target.map_or(pets[shooter].app.pet.face, |index| {
                if pets[index].app.pet.x < pets[shooter].app.pet.x {
                    -1.0
                } else {
                    1.0
                }
            });
            let source = &mut pets[shooter];
            source.app.pet.face = side;
            let mut origin = source.app.snowball_position();
            origin.x += side * source.app.height() * 0.29;
            let owner = source.id;
            let ball = target
                .and_then(|index| {
                    cursor = (index + 1) % pets.len();
                    Snowball::towards(owner, origin, &pets[index].app, true)
                })
                .unwrap_or(Snowball {
                    owner,
                    position: origin,
                    vx: side * 940.0,
                    vy: -100.0,
                    counter: None,
                });
            self.balls.push(ball);
            next_shot += 0.24;
        }
        Some(Task::Cannon {
            remaining,
            next_shot,
            cursor,
        })
    }

    fn fly(&mut self, pets: &mut [Instance], walls: &[SnowWall], width: f32, floor: f32, dt: f32) {
        if self.balls.is_empty() {
            return;
        }
        let bounds: Vec<_> = pets
            .iter()
            .map(|pet| {
                pet.app
                    .can_be_struck()
                    .then(|| pet.app.collision_bounds())
                    .flatten()
            })
            .collect();
        let wall_bounds: Vec<_> = walls
            .iter()
            .enumerate()
            .filter(|(slot, _)| self.wall_grounded(*slot))
            .filter_map(|(slot, &wall)| Some((slot, wall_bounds(wall, width, floor)?)))
            .flat_map(|(slot, cover)| cover.solids().map(|rect| (slot, rect)))
            .collect();
        let retaliations = &mut self.retaliations;
        let snow_hits = &mut self.snow_hits;
        let owes_cannon = &mut self.owes_cannon;
        let defences = &mut self.defences;
        let now = self.time;
        self.balls.retain_mut(|ball| {
            let before = ball.position;
            let from = ball.bounds();
            ball.position.x += ball.vx * dt;
            ball.position.y += (ball.vy + BALL_GRAVITY * dt * 0.5) * dt;
            ball.vy += BALL_GRAVITY * dt;
            let to = ball.bounds();
            let target = bounds
                .iter()
                .enumerate()
                .filter_map(|(i, body)| {
                    if pets[i].id == ball.owner || !pets[i].app.can_be_struck() {
                        return None;
                    }
                    if ball.counter == Some(pets[i].id)
                        && defences.get(&pets[i].id).is_some_and(|defence| {
                            defence.attacker == ball.owner
                                && matches!(defence.guard, Guard::Run { hidden: true, .. })
                        })
                    {
                        return None;
                    }
                    let body = (*body)?;
                    swept_contact(from, to, body, body).map(|time| (i, time))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let wall = wall_bounds
                .iter()
                .filter_map(|&(slot, rect)| Some((slot, swept_contact(from, to, rect, rect)?)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((slot, _)) = wall.filter(|&(_, time)| target.is_none_or(|(_, hit)| time <= hit)) {
                if let Some(defence) = ball.counter.and_then(|id| defences.get_mut(&id)) {
                    if defence.attacker == ball.owner
                        && matches!(defence.guard, Guard::Run { wall, hidden: true, .. } if wall == slot)
                    {
                        defence.hold_until = now + COVER_AFTER_BLOCK;
                    }
                }
                return false;
            }
            if let Some((index, time)) = target {
                let id = pets[index].id;
                let parry = ball.counter == Some(id)
                    && pets[index].app.pet.activity == Activity::Special(Special::Parry)
                    && fastrand::bool();
                if parry {
                    let at = Point::from_xy(
                        before.x + (ball.position.x - before.x) * time,
                        before.y + (ball.position.y - before.y) * time,
                    );
                    if let Some(returned) = pets
                        .iter()
                        .find(|pet| pet.id == ball.owner)
                        .and_then(|owner| Snowball::towards(id, at, &owner.app, false))
                    {
                        *ball = returned;
                    } else {
                        ball.owner = id;
                        ball.position = at;
                        ball.vx = -ball.vx;
                        ball.vy = -ball.vy.abs().max(90.0);
                        ball.counter = None;
                    }
                    return true;
                }
                let at = Point::from_xy(
                    before.x + (ball.position.x - before.x) * time,
                    before.y + (ball.position.y - before.y) * time,
                );
                if pets[index].app.hit_with_snowball(
                    at,
                    (ball.vx * BALL_KNOCKBACK).clamp(-1450.0, 1450.0),
                    -310.0,
                    ball.owner,
                ) {
                    let hits = snow_hits.entry(id).or_default();
                    hits.retain(|time| now - time <= HIT_WINDOW);
                    hits.push_back(now);
                    if hits.len() >= 3 {
                        hits.clear();
                        owes_cannon.insert(id);
                    } else if fastrand::bool() {
                        retaliations.insert(id, ball.owner);
                    }
                }
                return false;
            }
            ball.position.y < floor && ball.position.x > -10.0 && ball.position.x < width + 10.0
        });
    }

    pub fn ball_images<'a>(
        &'a self,
        art: &'a WinterArt,
    ) -> impl Iterator<Item = (i32, i32, &'a OverlayImage)> + Clone {
        self.balls.iter().filter_map(|ball| {
            let sprite = art.ball.as_ref()?;
            let s = art.scale as f32;
            Some((
                (ball.position.x * s - sprite.width as f32 * 0.5).round() as i32,
                (ball.position.y * s - sprite.height as f32 * 0.5).round() as i32,
                sprite,
            ))
        })
    }

    pub fn fragment_images<'a>(
        &'a self,
        art: &'a WinterArt,
    ) -> impl Iterator<Item = (i32, i32, &'a OverlayImage)> + Clone {
        self.fragments
            .iter()
            .filter_map(|fragment| fragment.image(&art.fragments, art.scale as f32))
    }
}

impl Snowball {
    fn towards(owner: u64, from: Point, app: &crate::app::App, rapid: bool) -> Option<Self> {
        let bounds = app.collision_bounds()?;
        let climbing = app.pet.activity == Activity::ClimbWall;
        let dx = (bounds.left() + bounds.right()) * 0.5 - from.x;
        let mut flight = dx.abs()
            / if rapid {
                940.0
            } else if climbing {
                880.0
            } else {
                630.0
            };
        if rapid || climbing {
            flight = flight.clamp(if climbing { 0.43 } else { 0.17 }, 1.5);
        } else if !(0.01..=1.12).contains(&flight) {
            return None;
        }
        let lead = if climbing { -72.0 * flight } else { 0.0 };
        let target_y = ((bounds.top() + bounds.bottom()) * 0.5 + lead).max(BALL_RADIUS);
        Some(Self {
            owner,
            position: from,
            vx: dx / flight,
            vy: (target_y - from.y) / flight - 0.5 * BALL_GRAVITY * flight,
            counter: None,
        })
    }

    fn bounds(&self) -> Rect {
        Rect::from_xywh(
            self.position.x - BALL_RADIUS,
            self.position.y - BALL_RADIUS,
            BALL_RADIUS * 2.0,
            BALL_RADIUS * 2.0,
        )
        .expect("snowball bounds")
    }
}

fn defending(activity: Activity) -> bool {
    matches!(
        activity,
        Activity::Special(Special::RunCover | Special::Hide | Special::LeaveCover | Special::Parry)
    )
}

fn cover_for(
    app: &crate::app::App,
    walls: &[SnowWall],
    grounded: &[bool; MAX_SNOW_WALLS],
    width: f32,
    attacker_x: f32,
) -> Guard {
    walls
        .iter()
        .enumerate()
        .filter_map(|(wall, &object)| {
            let x = object.position * width;
            let side = if attacker_x <= x { 1.0 } else { -1.0 };
            let goal = x + side * 23.0;
            let distance = (app.pet.x - goal).abs();
            (grounded[wall]
                && object.finished()
                && (app.room.left..=app.room.right).contains(&goal)
                && distance <= COVER_MAX_DISTANCE)
                .then_some((wall, side, distance))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map_or(Guard::Parry, |(wall, side, _)| Guard::Run {
            wall,
            side,
            hidden: false,
        })
}

fn wall_bounds(wall: SnowWall, width: f32, floor: f32) -> Option<Cover> {
    wall.finished()
        .then(|| Cover::new(wall.position * width, floor, wall.slope_right))
}

fn wall_pose(seconds: f32) -> SculptStage {
    match snowman_step(seconds) {
        0..=6 => SculptStage::Base,
        7..=15 => SculptStage::Middle,
        _ => SculptStage::Top,
    }
}

fn throw_range(climbing: bool) -> f32 {
    if climbing {
        1120.0
    } else {
        610.0
    }
}

fn nearest_pile(objects: &WinterObjects, from: f32) -> Option<f32> {
    objects
        .piles
        .iter()
        .map(|pile| pile.position)
        .min_by(|a, b| (a - from).abs().total_cmp(&(b - from).abs()))
}

fn room_for_structure(objects: &WinterObjects, tree: Option<Tree>, x: f32) -> bool {
    objects
        .snowmen()
        .all(|(_, man)| (man.position - x).abs() > 0.085)
        && tree.is_none_or(|tree| (tree.position - x).abs() > 0.094)
        && objects
            .walls
            .iter()
            .all(|wall| (wall.position - x).abs() > 0.10)
}

fn free_spot(objects: &WinterObjects, tree: Option<Tree>, from: f32, to: f32) -> Option<f32> {
    (0..12)
        .map(|_| from + fastrand::f32() * (to - from))
        .find(|x| {
            room_for_structure(objects, tree, *x)
                && objects
                    .piles
                    .iter()
                    .all(|pile| (pile.position - x).abs() > 0.065)
        })
}

pub fn structure_spot(
    objects: &WinterObjects,
    tree: Option<Tree>,
    from: f32,
    to: f32,
) -> Option<f32> {
    free_spot(objects, tree, from, to).or_else(|| {
        (0..=20)
            .map(|step| from + (to - from) * step as f32 / 20.0)
            .filter(|x| room_for_structure(objects, tree, *x))
            .max_by(|a, b| {
                let clearance = |x: f32| {
                    objects
                        .piles
                        .iter()
                        .map(|pile| (pile.position - x).abs())
                        .fold(1.0, f32::min)
                };
                clearance(*a).total_cmp(&clearance(*b))
            })
    })
}

#[derive(Default)]
pub struct WinterArt {
    scale: u32,
    piles: Vec<OverlayImage>,
    ball: Option<OverlayImage>,
    men: [Option<(u8, OverlayImage)>; MAX_SNOWMEN],
    fragments: Vec<OverlayImage>,
    walls: Vec<((u8, bool), OverlayImage)>,
}

fn snowman_step(seconds: f32) -> u8 {
    (seconds * f32::from(props::SNOWMAN_STEPS) / SNOWMAN_SECONDS) as u8
}

impl WinterArt {
    pub fn update(&mut self, objects: &WinterObjects, scale: u32) {
        if self.scale != scale {
            self.scale = scale;
            self.piles = (1..=5)
                .map(|size| props::pile(size, scale).into())
                .collect();
            self.ball = Some(props::ball_image(scale).into());
            self.fragments = (0..3)
                .map(|kind| props::snow_chunk(kind, scale).into())
                .collect();
            self.men = Default::default();
            self.walls.clear();
        }
        self.walls.truncate(objects.walls.len());
        for (index, wall) in objects.walls.iter().enumerate() {
            let key = (snowman_step(wall.seconds), wall.slope_right);
            if let Some((old, image)) = self.walls.get_mut(index) {
                if *old != key {
                    *old = key;
                    *image = props::snow_wall(key.0, key.1, scale).into();
                }
            } else {
                self.walls
                    .push((key, props::snow_wall(key.0, key.1, scale).into()));
            }
        }
        for (slot, cached) in self.men.iter_mut().enumerate() {
            if let Some(man) = objects.snowman(slot) {
                let step = snowman_step(man.seconds);
                if cached.as_ref().is_none_or(|(old, _)| *old != step) {
                    *cached = Some((step, props::snowman(step, scale).into()));
                }
            } else {
                *cached = None;
            }
        }
    }

    pub fn snowman_bounds(
        &self,
        slot: usize,
        man: Snowman,
        width: u32,
        floor: f32,
    ) -> Option<Rect> {
        let (_, image) = self.men[slot].as_ref()?;
        self.prop_bounds(image, man.position * width as f32, floor)
    }

    pub fn wall_bounds(&self, slot: usize, wall: SnowWall, width: u32, floor: f32) -> Option<Rect> {
        let (_, image) = self.walls.get(slot)?;
        self.prop_bounds(image, wall.position * width as f32, floor)
    }

    pub fn wall_ink(
        &self,
        slot: usize,
        wall: SnowWall,
        width: u32,
        floor: f32,
    ) -> impl Iterator<Item = Rect> + '_ {
        let scale = self.scale.max(1);
        self.walls
            .get(slot)
            .into_iter()
            .flat_map(move |(_, image)| {
                let scale_f = scale as f32;
                let left = wall.position * width as f32 - image.width as f32 / (2.0 * scale_f);
                let top = floor - image.height as f32 / scale_f;
                image.ink.iter().filter_map(move |&(x, y, width)| {
                    if y as u32 % scale != scale / 2 {
                        return None;
                    }
                    Rect::from_xywh(
                        left + f32::from(x) / scale_f,
                        top + (y as u32 / scale) as f32,
                        f32::from(width) / scale_f,
                        1.0,
                    )
                })
            })
    }

    fn prop_bounds(&self, image: &OverlayImage, centre: f32, floor: f32) -> Option<Rect> {
        let scale = self.scale as f32;
        let top = f32::from(image.ink.first()?.1) / scale;
        let left = image.ink.iter().map(|line| line.0).min()?;
        let right = image
            .ink
            .iter()
            .map(|line| i32::from(line.0) + i32::from(line.2))
            .max()?;
        Rect::from_xywh(
            centre - image.width as f32 / (2.0 * scale) + f32::from(left) / scale,
            floor - image.height as f32 / scale + top,
            (right - i32::from(left)) as f32 / scale,
            image.height as f32 / scale - top,
        )
    }

    pub fn images<'a>(
        &'a self,
        objects: &'a WinterObjects,
        width: u32,
        floor: f32,
        snowman_lift: [f32; MAX_SNOWMEN],
        wall_lift: [f32; MAX_SNOW_WALLS],
    ) -> impl Iterator<Item = (i32, i32, &'a OverlayImage)> + Clone {
        let s = self.scale as f32;
        let at = move |position: f32, image: &'a OverlayImage| {
            let centre = position * width as f32 * s;
            (
                (centre - image.width as f32 * 0.5).round() as i32,
                (floor * s).round() as i32 - image.height as i32,
                image,
            )
        };
        objects
            .piles
            .iter()
            .filter_map(move |pile| {
                self.piles
                    .get(usize::from(pile.servings) - 1)
                    .map(|image| at(pile.position, image))
            })
            .chain(objects.snowmen().filter_map(move |(slot, man)| {
                self.men[slot].as_ref().map(|(_, image)| {
                    let (x, y, image) = at(man.position, image);
                    (x, y - (snowman_lift[slot] * s).round() as i32, image)
                })
            }))
            .chain(objects.walls.iter().zip(&self.walls).enumerate().map(
                move |(slot, (wall, (_, image)))| {
                    let (x, y, image) = at(wall.position, image);
                    (x, y - (wall_lift[slot] * s).round() as i32, image)
                },
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, config::Settings, pack::Pack, startup_room};

    fn pet(id: u64, x: f32) -> Instance {
        let mut app = App::new(
            startup_room(1000.0, 650.0),
            Settings::default(),
            Pack::builtin(),
        );
        app.pet.x = x;
        Instance { id, app }
    }

    #[test]
    fn an_answer_waits_for_recovery_and_targets_the_original_thrower() {
        let mut world = World::test("bear-girl");
        world.update_season(Some(2027));
        world.winter_mut().piles.push(Pile {
            position: 0.38,
            servings: 2,
        });
        let mut pets = [pet(0, 300.0), pet(1, 421.0), pet(2, 534.0)];
        let mut interactions = Interactions::new();
        interactions.retaliations.insert(1, 0);
        for activity in [Activity::Fall, Activity::Down, Activity::Rise] {
            pets[1].app.pet.start(activity);
            interactions.retaliate(&mut pets, world.winter(), 1000.0);
            assert!(interactions.tasks.is_empty());
        }
        pets[1].app.pet.start(Activity::Idle);
        interactions.retaliate(&mut pets, world.winter(), 1000.0);
        assert_eq!(pets[1].app.pet.activity, Activity::Approach);
        assert!(matches!(
            interactions.tasks[&1],
            Task::ApproachPile { target: 0, .. }
        ));
        assert!(interactions.retaliations.is_empty());
    }

    #[test]
    fn a_snowball_hits_the_first_other_chibi_and_knocks_it_down() {
        let mut pets = [pet(0, 300.0), pet(1, 421.0), pet(2, 534.0)];
        let target = pets[1].app.collision_bounds().unwrap();
        let mut interactions = Interactions::new();
        interactions.balls.push(Snowball {
            owner: 0,
            position: Point::from_xy(315.0, (target.top() + target.bottom()) * 0.5),
            vx: 710.0,
            vy: -70.0,
            counter: None,
        });
        for _ in 0..12 {
            interactions.fly(&mut pets, &[], 1000.0, 650.0, 1.0 / 60.0);
        }
        assert!(interactions.balls.is_empty());
        assert_eq!(pets[0].app.pet.activity, Activity::Idle);
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        assert!((pets[1].app.pet.vx - 710.0 * 1.36).abs() < 0.01);
        assert_eq!(pets[1].app.pet.vy, -310.0);
        assert_eq!(pets[2].app.pet.activity, Activity::Idle);
    }

    #[test]
    fn a_counter_is_parried_without_cover_or_blocked_by_a_finished_wall() {
        let mut objects = WinterObjects::default();
        objects.walls.push(SnowWall {
            slope_right: true,
            position: 0.5,
            seconds: 450.0,
        });
        let mut pets = [pet(0, 300.0), pet(1, 509.0)];
        let mut interactions = Interactions::new();
        let y = 591.0;
        interactions.balls.push(Snowball {
            owner: 0,
            position: Point::from_xy(398.0, y),
            vx: 640.0,
            vy: -30.0,
            counter: Some(1),
        });
        let seed = (0..100)
            .find(|&seed| fastrand::Rng::with_seed(seed).bool())
            .unwrap();
        fastrand::seed(seed);
        for _ in 0..10 {
            interactions.defend(&mut pets, &objects, 1000.0, 650.0, 0.05);
            interactions.fly(&mut pets, &objects.walls, 1000.0, 650.0, 0.05);
            if interactions
                .balls
                .first()
                .is_some_and(|ball| ball.owner == 1)
            {
                break;
            }
        }
        assert_eq!(pets[1].app.pet.activity, Activity::Special(Special::Parry));
        assert_eq!(interactions.balls[0].owner, 1);
        assert!(interactions.balls[0].vx < 0.0);

        interactions.balls.clear();
        interactions.counters.insert(0, 1);
        objects.walls[0].seconds = SNOWMAN_SECONDS;
        interactions.defend(&mut pets, &objects, 1000.0, 650.0, 0.01);
        assert_eq!(
            pets[1].app.pet.activity,
            Activity::Special(Special::RunCover)
        );
        assert!(pets[1].app.cover.is_some());
        for _ in 0..10 {
            interactions.defend(&mut pets, &objects, 1000.0, 650.0, 0.05);
        }
        assert_eq!(pets[1].app.pet.activity, Activity::Special(Special::Hide));
        let cover = pets[1].app.cover.unwrap();
        assert!(cover.contains(475.0, 600.0));
        assert!(!cover.contains(500.0, 600.0));
        interactions.balls.push(Snowball {
            owner: 0,
            position: Point::from_xy(400.0, y),
            vx: 640.0,
            vy: -30.0,
            counter: Some(1),
        });
        for _ in 0..10 {
            interactions.fly(&mut pets, &objects.walls, 1000.0, 650.0, 0.05);
        }
        assert!(interactions.balls.is_empty());
        assert_eq!(pets[1].app.pet.activity, Activity::Special(Special::Hide));
        interactions.counters.clear();
        interactions.balls.clear();
        interactions.time = COVER_AFTER_BLOCK - 0.01;
        interactions.defend(&mut pets, &objects, 1000.0, 650.0, 0.05);
        assert_eq!(pets[1].app.pet.activity, Activity::Special(Special::Hide));
        interactions.time = COVER_AFTER_BLOCK;
        interactions.defend(&mut pets, &objects, 1000.0, 650.0, 0.05);
        assert_eq!(
            pets[1].app.pet.activity,
            Activity::Special(Special::LeaveCover)
        );
        assert!(pets[1].app.cover.is_some());
        for _ in 0..30 {
            interactions.defend(&mut pets, &objects, 1000.0, 650.0, 0.05);
        }
        assert_eq!(pets[1].app.pet.activity, Activity::Idle);
        assert!(pets[1].app.cover.is_none());
    }

    #[test]
    fn a_far_wall_is_not_cover_and_an_existing_wall_prevents_another_build() {
        let mut world = World::test(crate::pack::BUILTIN_ID);
        world.winter_mut().walls.push(SnowWall {
            position: 0.5,
            seconds: SNOWMAN_SECONDS,
            slope_right: true,
        });
        let mut defender = pet(1, 850.0);
        let mut interactions = Interactions::new();
        assert!(matches!(
            cover_for(
                &defender.app,
                &world.winter().walls,
                &[true; MAX_SNOW_WALLS],
                1000.0,
                220.0
            ),
            Guard::Parry
        ));
        interactions.start_wall(&mut defender, &mut world);
        assert_eq!(world.winter().walls.len(), 1);
        assert!(interactions.tasks.is_empty());
    }

    #[test]
    fn three_recent_hits_arm_a_cannon_after_recovery() {
        let mut pets = [pet(0, 300.0), pet(1, 421.0), pet(2, 510.0)];
        let mut interactions = Interactions::new();
        for time in [1.0, 32.0, 38.0, 62.0] {
            interactions.time = time;
            pets[1] = pet(1, 421.0);
            interactions.balls.push(Snowball {
                owner: 0,
                position: Point::from_xy(352.0, 590.0),
                vx: 710.0,
                vy: -70.0,
                counter: None,
            });
            for _ in 0..12 {
                interactions.fly(&mut pets, &[], 1000.0, 650.0, 0.02);
            }
            assert!(interactions.balls.is_empty());
            assert_eq!(pets[1].app.pet.activity, Activity::Fall);
            assert_eq!(interactions.owes_cannon.contains(&1), time == 62.0);
        }
        interactions.start_cannons(&mut pets);
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        pets[1] = pet(1, 421.0);
        interactions.start_cannons(&mut pets);
        assert_eq!(pets[1].app.pet.activity, Activity::Special(Special::Cannon));
        pets[0].app.pet.start(Activity::Down);
        pets[2].app.pet.start(Activity::Down);
        interactions.cannon(1, &mut pets, 4.0, 0.0, 0);
        interactions.cannon(1, &mut pets, 4.0, 0.0, 2);
        assert_eq!(interactions.balls.len(), 2);
        assert!(interactions.balls[0].vx < 0.0 && interactions.balls[1].vx > 0.0);
        for _ in 0..24 {
            interactions.fly(&mut pets, &[], 1000.0, 650.0, 0.02);
        }
        assert_eq!(pets[0].app.pet.activity, Activity::Down);
        assert_eq!(pets[1].app.pet.activity, Activity::Special(Special::Cannon));
        assert_eq!(pets[2].app.pet.activity, Activity::Down);

        interactions.balls.clear();
        let mut task = interactions.tasks.get(&1).copied();
        while let Some(Task::Cannon {
            remaining,
            next_shot,
            cursor,
        }) = task
        {
            task = interactions.cannon(1, &mut pets, remaining - 0.01, next_shot - 0.01, cursor);
        }
        assert_eq!(interactions.balls.len(), 10);
    }

    #[test]
    fn a_throw_can_reach_a_chibi_climbing_far_above_the_floor() {
        let mut pets = [pet(0, 660.0), pet(1, 400.0)];
        pets[1].app.pet.face = -1.0;
        pets[1].app.pet.x = pets[1].app.room.left;
        pets[1].app.pet.y = 345.0;
        pets[1].app.pet.start(Activity::ClimbWall);
        assert!(pets[1].app.can_be_struck());
        let mut interactions = Interactions::new();
        interactions
            .balls
            .push(Snowball::towards(0, Point::from_xy(640.0, 590.0), &pets[1].app, false).unwrap());
        for _ in 0..90 {
            if pets[1].app.pet.activity == Activity::Fall {
                break;
            }
            pets[1].app.tick(1.0 / 60.0, true);
            interactions.fly(&mut pets, &[], 1000.0, 650.0, 1.0 / 60.0);
        }
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        assert!(pets[1].app.pet.y < 345.0);
    }

    #[test]
    fn a_chibi_collects_snow_and_throws_it_at_another_chibi() {
        let mut world = World::test("bear-girl");
        world.update_season(Some(2027));
        world.winter_mut().piles.push(Pile {
            position: 0.33,
            servings: 1,
        });
        let mut pets = [pet(0, 275.0), pet(1, 477.0)];
        let mut interactions = Interactions::new();
        interactions.choose_in = 100.0;
        interactions.pile_in = 100.0;
        pets[0].app.pet.start(Activity::Approach);
        pets[1].app.pet.start(Activity::Listen);
        interactions.tasks.insert(
            0,
            Task::ApproachPile {
                position: 0.33,
                target: 1,
            },
        );
        let mut packed = false;
        let mut threw = false;
        let dt = 1.0 / 60.0;
        for _ in 0..180 {
            for pet in &mut pets {
                pet.app.tick(dt, true);
            }
            interactions.tick(&mut pets, &mut world, 1000, 650.0, dt, true);
            packed |= pets[0].app.pet.activity == Activity::PackSnow;
            threw |= !interactions.balls.is_empty();
            if pets[1].app.pet.activity == Activity::Fall {
                break;
            }
        }
        assert!(world.winter().piles.is_empty());
        assert!(packed && threw);
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
    }

    #[test]
    fn the_same_snowman_counts_only_while_someone_is_sculpting() {
        let mut world = World::test("bear-girl");
        world.update_season(Some(2027));
        world.winter_mut().snowman = Some(Snowman {
            position: 0.50,
            seconds: 449.0,
        });
        let mut pets = [pet(0, 464.0), pet(1, 534.0)];
        let mut interactions = Interactions::new();
        interactions.choose_in = 100.0;
        for index in 0..pets.len() {
            let id = pets[index].id;
            interactions.start_building(&mut pets[index], &mut world);
            let original = world.winter().snowman.unwrap().seconds;
            assert!(matches!(
                interactions.tasks[&id],
                Task::ApproachSnowman { slot: 0 }
            ));
            for _ in 0..180 {
                interactions.tick(&mut pets, &mut world, 1000, 650.0, 0.02, true);
                if matches!(interactions.tasks.get(&id), Some(Task::Sculpt { .. })) {
                    break;
                }
            }
            assert_eq!(world.winter().snowman.unwrap().seconds, original);
            let Task::Sculpt { slot: 0, remaining } = interactions.tasks[&id] else {
                panic!("the builder never reached the snowman")
            };
            assert!((3.0..=5.0).contains(&remaining));
            assert_eq!(
                pets[index].app.sculpt_stage,
                if index == 0 {
                    SculptStage::Middle
                } else {
                    SculptStage::Top
                }
            );
            interactions.tick(&mut pets, &mut world, 1000, 650.0, 1.25, true);
            assert_eq!(world.winter().snowman.unwrap().seconds, original + 1.25);
            assert_eq!(pets[index].app.sculpt_stage, SculptStage::Top);
            pets[index].app.pet.start(Activity::Fall);
            interactions.tick(&mut pets, &mut world, 1000, 650.0, 0.5, true);
            assert_eq!(world.winter().snowman.unwrap().seconds, original + 1.25);
        }
        assert_eq!(world.winter().snowman.unwrap().seconds, 451.5);
    }

    #[test]
    fn two_chibis_can_work_on_different_snowmen() {
        let mut world = World::test("bear-girl");
        world.update_season(Some(2027));
        *world.winter_mut().snowman_mut(0) = Some(Snowman {
            position: 0.5,
            seconds: 200.0,
        });
        let mut pets = [pet(0, 480.0), pet(1, 420.0)];
        let mut interactions = Interactions::new();
        interactions.start_building(&mut pets[0], &mut world);
        interactions.start_building(&mut pets[1], &mut world);
        assert!(matches!(
            interactions.tasks[&0],
            Task::ApproachSnowman { slot: 0 }
        ));
        assert!(matches!(
            interactions.tasks[&1],
            Task::ApproachSnowman { slot: 1 }
        ));
        let second = world.winter().snowman(1).unwrap();
        assert!((second.position - 0.5).abs() > 0.085);
        assert_eq!(second.seconds, 0.0);
        interactions.release_snowman(&mut pets, 0, true);
        assert!(!interactions.tasks.contains_key(&0));
        assert!(matches!(
            interactions.tasks[&1],
            Task::ApproachSnowman { slot: 1 }
        ));
    }

    #[test]
    fn nearby_chibis_take_turns_and_stop_if_one_is_grabbed() {
        let mut pets = [pet(0, 490.0), pet(1, 550.0)];
        let mut interactions = Interactions::new();
        interactions.choose(&mut pets, &mut World::test("bear-girl"), 1000.0, false, 10);
        assert_eq!(pets[0].app.pet.activity, Activity::Talk);
        assert_eq!(pets[1].app.pet.activity, Activity::Listen);
        assert_eq!((pets[0].app.pet.face, pets[1].app.pet.face), (1.0, -1.0));
        interactions.talk(&mut pets, 1.15);
        assert_eq!(pets[0].app.pet.activity, Activity::Listen);
        assert_eq!(pets[1].app.pet.activity, Activity::Talk);
        pets[1].app.pet.grab();
        interactions.talk(&mut pets, 0.1);
        assert_eq!(pets[0].app.pet.activity, Activity::Idle);
        assert_eq!(pets[1].app.pet.activity, Activity::Drag);
    }
}
