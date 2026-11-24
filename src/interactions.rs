// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;

use tiny_skia::{Point, Rect};

use crate::game::{swept_contact, Instance};
use crate::props::{self, SculptStage};
use crate::season::OverlayImage;
use crate::sim::Activity;
use crate::snowman::Fragment;
use crate::world::{Pile, Snowman, WinterObjects, World, SNOWMAN_SECONDS};

pub const RELEASE_AT: f32 = 0.44;
const PACK_FOR: f32 = 0.87;
const THROW_FOR: f32 = 0.76;
const BALL_GRAVITY: f32 = 750.0;
const BALL_RADIUS: f32 = 5.5;
const BALL_KNOCKBACK: f32 = 0.68 * 2.0;

#[derive(Clone, Copy)]
enum Task {
    ApproachPile { position: f32, target: u64 },
    Pack { target: u64 },
    Aim { target: u64, remaining: f32 },
    Throw { target: u64, released: bool },
    ApproachSnowman,
    Sculpt { remaining: f32 },
}

impl Task {
    fn activity(self) -> Activity {
        match self {
            Self::ApproachPile { .. } | Self::ApproachSnowman | Self::Aim { .. } => {
                Activity::Approach
            }
            Self::Pack { .. } => Activity::PackSnow,
            Self::Throw { .. } => Activity::ThrowSnow,
            Self::Sculpt { .. } => Activity::Sculpt,
        }
    }

    fn building(self) -> bool {
        matches!(self, Self::ApproachSnowman | Self::Sculpt { .. })
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
}

pub struct Interactions {
    tasks: HashMap<u64, Task>,
    retaliations: HashMap<u64, u64>,
    conversations: Vec<Conversation>,
    balls: Vec<Snowball>,
    fragments: Vec<Fragment>,
    pub snowman_held: bool,
    snowman_cooldown: f32,
    choose_in: f32,
    pile_in: f32,
}

impl Interactions {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            retaliations: HashMap::new(),
            conversations: Vec::new(),
            balls: Vec::new(),
            fragments: Vec::new(),
            snowman_held: false,
            snowman_cooldown: 0.0,
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
            {
                pet.app.pet.start(Activity::Idle);
            }
        }
        self.tasks.clear();
        self.retaliations.clear();
        self.balls.clear();
        self.fragments.clear();
        self.snowman_held = false;
        self.snowman_cooldown = 0.0;
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
        self.talk(pets, elapsed);
        self.snowman_cooldown = (self.snowman_cooldown - elapsed).max(0.0);
        self.fragments
            .retain_mut(|fragment| fragment.tick(dt, floor));
        if winter && width > 0 {
            self.drop_pile(world, dt);
            self.retaliate(pets, world.winter(), width as f32);
            self.tasks
                .retain(|id, _| pets.iter().any(|pet| pet.id == *id));
            for index in 0..pets.len() {
                let id = pets[index].id;
                if let Some(task) = self.tasks.remove(&id) {
                    if pets[index].app.pet.activity == task.activity() {
                        if self.snowman_held && task.building() {
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
            self.fly(pets, width as f32, floor, dt);
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
            }
        }
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
            14..38
                if winter && width > 0.0 && !self.snowman_held && self.snowman_cooldown == 0.0 =>
            {
                if !self.tasks.values().any(|task| task.building())
                    && world
                        .winter()
                        .snowman
                        .is_none_or(|man| man.seconds < SNOWMAN_SECONDS)
                {
                    let index = resting[fastrand::usize(..resting.len())];
                    self.start_building(&mut pets[index], world);
                }
            }
            38..57 if winter && resting.len() >= 2 && width > 0.0 => {
                let i = resting[fastrand::usize(..resting.len())];
                let from = pets[i].app.pet.x / width;
                let Some(position) = nearest_pile(world.winter(), from) else {
                    return;
                };
                let from = position * width;
                let target = resting
                    .iter()
                    .copied()
                    .filter(|&j| {
                        let gap = (pets[j].app.pet.x - from).abs();
                        j != i && (40.0..=600.0).contains(&gap)
                    })
                    .min_by(|&j, &k| {
                        (pets[j].app.pet.x - from)
                            .abs()
                            .total_cmp(&(pets[k].app.pet.x - from).abs())
                    });
                if let Some(j) = target {
                    let task = Task::ApproachPile {
                        position,
                        target: pets[j].id,
                    };
                    pets[i].app.pet.start(task.activity());
                    self.tasks.insert(pets[i].id, task);
                }
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
        if world.winter().snowman.is_none() {
            let position = free_spot(world.winter(), 0.30, 0.70).unwrap_or(0.5);
            world.winter_mut().snowman = Some(Snowman {
                position,
                seconds: 0.0,
            });
            world.save_now();
        }
        pet.app.pet.start(Activity::Approach);
        self.tasks.insert(pet.id, Task::ApproachSnowman);
    }

    pub fn release_snowman(&mut self, pets: &mut [Instance], broken: bool) {
        self.snowman_held = false;
        for pet in pets {
            if let Some(task) = self.tasks.get_mut(&pet.id).filter(|task| task.building()) {
                if pet.app.pet.activity == task.activity() {
                    if broken {
                        pet.app.pet.start(Activity::Idle);
                    } else {
                        *task = Task::ApproachSnowman;
                        pet.app.pet.start(task.activity());
                    }
                }
            }
        }
        if broken {
            self.tasks.retain(|_, task| !task.building());
        }
    }

    pub fn break_snowman(
        &mut self,
        pets: &mut [Instance],
        at: Point,
        velocity: Point,
        height: f32,
    ) {
        self.release_snowman(pets, true);
        self.snowman_cooldown = 7.0;
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
            if let Some(position) = free_spot(world.winter(), 0.17, 0.83) {
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
        let target = match task {
            Task::Pack { target } | Task::Aim { target, .. } => {
                let other = &pets.iter().find(|pet| pet.id == target)?.app;
                Some((other.pet.x, other.can_be_struck()))
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
                let (target_x, vulnerable) = target?;
                let gap = target_x - app.pet.x;
                if vulnerable && (38.0..=610.0).contains(&gap.abs()) {
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
                let (target_x, vulnerable) = target?;
                let gap = target_x - app.pet.x;
                if remaining <= 0.0 {
                    return None;
                }
                if vulnerable && (38.0..=610.0).contains(&gap.abs()) {
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
                let origin = app.snowball_position();
                let direction = app.pet.face;
                let target_app = &pets.iter().find(|pet| pet.id == target)?.app;
                if let Some(bounds) = target_app.collision_bounds() {
                    let velocity = direction * 630.0;
                    let dx = (bounds.left() + bounds.right()) * 0.5 - origin.x;
                    let flight = dx / velocity;
                    if (0.01..=1.12).contains(&flight) {
                        let dy = (bounds.top() + bounds.bottom()) * 0.5 - origin.y;
                        self.balls.push(Snowball {
                            owner: id,
                            position: origin,
                            vx: velocity,
                            vy: (dy - 0.5 * BALL_GRAVITY * flight * flight) / flight,
                        });
                    }
                }
                return Some(Task::Throw {
                    target,
                    released: true,
                });
            }
            Task::ApproachSnowman => {
                let man = world.winter().snowman?;
                let x = man.position * width;
                let offset = (app.height() * 0.29).clamp(14.0, 54.0);
                let side = if app.pet.x <= x { 1.0 } else { -1.0 };
                if !app.pet.approach(&app.room, x - side * offset, dt) {
                    return Some(task);
                }
                app.pet.face = side;
                app.sculpt_stage = SculptStage::for_step(snowman_step(man.seconds));
                Task::Sculpt {
                    remaining: 3.0 + 2.0 * fastrand::f32(),
                }
            }
            Task::Sculpt { remaining } => {
                let man = world.winter_mut().snowman.as_mut()?;
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
                    remaining: remaining - spent,
                });
            }
        };
        pets[index].app.pet.start(next.activity());
        Some(next)
    }

    fn fly(&mut self, pets: &mut [Instance], width: f32, floor: f32, dt: f32) {
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
        let retaliations = &mut self.retaliations;
        self.balls.retain_mut(|ball| {
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
                    let body = (*body)?;
                    swept_contact(from, to, body, body).map(|time| (i, time))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((index, _)) = target {
                if pets[index].app.knock_back(ball.vx * BALL_KNOCKBACK, -310.0) && fastrand::bool()
                {
                    retaliations.insert(pets[index].id, ball.owner);
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

fn nearest_pile(objects: &WinterObjects, from: f32) -> Option<f32> {
    objects
        .piles
        .iter()
        .map(|pile| pile.position)
        .min_by(|a, b| (a - from).abs().total_cmp(&(b - from).abs()))
}

fn free_spot(objects: &WinterObjects, from: f32, to: f32) -> Option<f32> {
    (0..12)
        .map(|_| from + fastrand::f32() * (to - from))
        .find(|x| {
            objects
                .snowman
                .is_none_or(|man| (man.position - x).abs() > 0.085)
                && objects
                    .piles
                    .iter()
                    .all(|pile| (pile.position - x).abs() > 0.065)
        })
}

#[derive(Default)]
pub struct WinterArt {
    scale: u32,
    piles: Vec<OverlayImage>,
    ball: Option<OverlayImage>,
    man: Option<(u8, OverlayImage)>,
    fragments: Vec<OverlayImage>,
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
            self.man = None;
        }
        if let Some(man) = objects.snowman {
            let step = snowman_step(man.seconds);
            if self.man.as_ref().is_none_or(|(old, _)| *old != step) {
                self.man = Some((step, props::snowman(step, scale).into()));
            }
        } else {
            self.man = None;
        }
    }

    pub fn snowman_bounds(&self, man: Snowman, width: u32, floor: f32) -> Option<Rect> {
        let (_, image) = self.man.as_ref()?;
        let scale = self.scale as f32;
        let centre = man.position * width as f32;
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
        snowman_lift: f32,
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
            .chain(objects.snowman.into_iter().filter_map(move |man| {
                self.man.as_ref().map(|(_, image)| {
                    let (x, y, image) = at(man.position, image);
                    (x, y - (snowman_lift * s).round() as i32, image)
                })
            }))
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
        });
        for _ in 0..12 {
            interactions.fly(&mut pets, 1000.0, 650.0, 1.0 / 60.0);
        }
        assert!(interactions.balls.is_empty());
        assert_eq!(pets[0].app.pet.activity, Activity::Idle);
        assert_eq!(pets[1].app.pet.activity, Activity::Fall);
        assert!((pets[1].app.pet.vx - 710.0 * 1.36).abs() < 0.01);
        assert_eq!(pets[1].app.pet.vy, -310.0);
        assert_eq!(pets[2].app.pet.activity, Activity::Idle);
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
            assert!(matches!(interactions.tasks[&id], Task::ApproachSnowman));
            for _ in 0..180 {
                interactions.tick(&mut pets, &mut world, 1000, 650.0, 0.02, true);
                if matches!(interactions.tasks.get(&id), Some(Task::Sculpt { .. })) {
                    break;
                }
            }
            assert_eq!(world.winter().snowman.unwrap().seconds, original);
            let Task::Sculpt { remaining } = interactions.tasks[&id] else {
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
