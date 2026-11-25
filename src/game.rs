// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use tiny_skia::{Point, Rect};

use crate::app::{App, Pointer};
use crate::config::{InputMode, Settings};
use crate::festive::{Festive, FrameInfo};
use crate::interactions::{Interactions, WinterArt};
use crate::pack::Pack;
use crate::ramps::Ramps;
use crate::season::{OverlayImage, Snow};
use crate::sim::Room;
use crate::sim::{Activity, Special};
use crate::sled::{SledFrame, SledScene};
use crate::snowman::{Fall, Grab, Motion as SnowmanMotion};
use crate::visitors::{Frame as VisitorFrame, Gifts, Kind, LostChibi, Visitors};
use crate::world::{World, MAX_SNOWMEN, MAX_SNOW_WALLS};

pub enum Command {
    Add(Pack),
    Remove(String),
    Uninstall(String),
    Appearance(Settings),
    Visit(Kind),
}

pub struct Instance {
    pub id: u64,
    pub app: App,
}

pub struct Startup {
    pub packs: Vec<Pack>,
    pub world: World,
    pub lost_chibi: Option<Sender<LostChibi>>,
}

#[cfg(test)]
impl Startup {
    pub fn test(pack: Pack) -> Self {
        let world = World::test(&pack.id);
        Self {
            packs: vec![pack],
            world,
            lost_chibi: None,
        }
    }
}

struct CollisionBody {
    bounds: Option<Rect>,
    velocity: Option<Point>,
    vulnerable: bool,
    snowball_owner: Option<u64>,
    on_ramp: bool,
}

struct OrdinaryScene {
    pets: Vec<Instance>,
    interactions: Interactions,
    snowmen: [SnowmanMotion; MAX_SNOWMEN],
    festive: Festive,
    sled: SledScene,
    ramps: Ramps,
    visitors: Visitors,
    gifts: Gifts,
}

pub struct Game {
    pub pets: Vec<Instance>,
    pub snow: Snow,
    world: World,
    interactions: Interactions,
    festive: Festive,
    sled: SledScene,
    ramps: Ramps,
    visitors: Visitors,
    gifts: Gifts,
    sled_monitors: Vec<(f32, f32)>,
    winter_art: WinterArt,
    width: u32,
    room: Room,
    settings: Settings,
    commands: Receiver<Command>,
    bottom: f32,
    scale: u32,
    next_id: u64,
    preview_generation: u64,
    lost_chibi: Option<Sender<LostChibi>>,
    held: Option<u64>,
    snowman_motion: [SnowmanMotion; MAX_SNOWMEN],
    ordinary: Option<OrdinaryScene>,
    contacts: HashSet<(u64, u64)>,
    pub exit: bool,
}

impl Game {
    pub fn new(
        room: Room,
        settings: Settings,
        startup: Startup,
        commands: Receiver<Command>,
    ) -> Self {
        let snow = Snow::new();
        let mut game = Self {
            pets: Vec::new(),
            snow,
            world: startup.world,
            interactions: Interactions::new(),
            festive: Festive::new(),
            sled: SledScene::default(),
            ramps: Ramps::default(),
            visitors: Visitors::default(),
            gifts: Gifts::default(),
            sled_monitors: Vec::new(),
            winter_art: WinterArt::default(),
            width: 0,
            room,
            settings,
            commands,
            bottom: 0.0,
            scale: 1,
            next_id: 0,
            preview_generation: 0,
            lost_chibi: startup.lost_chibi,
            held: None,
            snowman_motion: Default::default(),
            ordinary: None,
            contacts: HashSet::new(),
            exit: false,
        };
        game.world.update_season(game.snow.winter_year());
        for pack in startup.packs {
            game.add(pack);
        }
        game
    }

    fn add(&mut self, pack: Pack) {
        let mut app = App::new(self.room.clone(), self.settings.clone(), pack);
        app.set_bottom(self.bottom);
        app.set_room(self.room.clone());
        app.scale = self.scale;
        let n = self.next_id as f32;
        if self.next_id != 0 {
            let placement = (0.5 + n * 0.381_966).fract();
            app.pet.x = app.room.left + (app.room.right - app.room.left) * placement;
            app.prepare();
        }
        self.pets.push(Instance {
            id: self.next_id,
            app,
        });
        self.next_id += 1;
    }

    pub fn commands(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.commands.try_recv() {
                Ok(Command::Add(pack)) => {
                    self.world.add(pack.id.clone());
                    self.add(pack);
                }
                Ok(Command::Remove(id)) => {
                    if let Some(index) = self.pets.iter().rposition(|pet| pet.app.pack_id() == id) {
                        if self.held == Some(self.pets[index].id) {
                            self.held = None;
                        }
                        self.pets.remove(index);
                        self.world.remove(&id);
                    }
                }
                Ok(Command::Visit(guest)) => self.visitors.call(guest),
                Ok(Command::Appearance(settings)) => {
                    self.appearance(settings);
                }
                Ok(Command::Uninstall(id)) => {
                    if self
                        .pets
                        .iter()
                        .any(|pet| Some(pet.id) == self.held && pet.app.pack_id() == id)
                    {
                        self.held = None;
                    }
                    self.pets.retain(|pet| pet.app.pack_id() != id);
                    if let Some(scene) = &mut self.ordinary {
                        scene.pets.retain(|pet| pet.app.pack_id() != id);
                    }
                    self.world.uninstall(&id);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.world.checkpoint();
                    return changed;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.exit = true;
                    return changed;
                }
            }
            changed = true;
        }
    }

    fn appearance(&mut self, settings: Settings) {
        let was_preview = self.settings.preview_date.is_some();
        let preview = settings.preview_date.is_some();
        self.settings = settings;
        if was_preview != preview {
            self.cancel_grab(|_, _| false);
            self.contacts.clear();
            if preview {
                self.preview_generation += 1;
                self.world.start_preview();
                let pets = std::mem::take(&mut self.pets);
                for pack in pets.iter().map(|pet| pet.app.pack()) {
                    self.add(pack);
                }
                self.ordinary = Some(OrdinaryScene {
                    pets,
                    interactions: std::mem::replace(&mut self.interactions, Interactions::new()),
                    snowmen: std::mem::take(&mut self.snowman_motion),
                    festive: std::mem::replace(&mut self.festive, Festive::new()),
                    sled: std::mem::take(&mut self.sled),
                    ramps: std::mem::take(&mut self.ramps),
                    visitors: std::mem::take(&mut self.visitors),
                    gifts: std::mem::take(&mut self.gifts),
                });
            } else {
                self.world.stop_preview();
                let scene = self.ordinary.take().expect("parked ordinary scene");
                self.pets = scene.pets;
                self.interactions = scene.interactions;
                self.snowman_motion = scene.snowmen;
                self.festive = scene.festive;
                self.sled = scene.sled;
                self.ramps = scene.ramps;
                self.visitors = scene.visitors;
                self.gifts = scene.gifts;
                for pet in &mut self.pets {
                    pet.app.set_bottom(self.bottom);
                    pet.app.set_room(self.room.clone());
                    pet.app.scale = self.scale;
                }
            }
        }
        self.snow.set_date(self.settings.preview_date);
        for pet in &mut self.pets {
            pet.app.apply_settings(self.settings.clone());
        }
        self.tick_snow(0.0, self.width);
    }

    pub fn set_room(&mut self, room: Room, bottom: f32) {
        self.room = room;
        self.bottom = bottom;
        for pet in &mut self.pets {
            pet.app.set_bottom(bottom);
            pet.app.set_room(self.room.clone());
        }
    }

    pub fn set_scale(&mut self, scale: u32) {
        self.scale = scale;
        for pet in &mut self.pets {
            pet.app.scale = scale;
        }
    }

    pub fn set_sled_monitors(&mut self, monitors: Vec<(f32, f32)>) {
        self.sled_monitors = monitors;
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn input_mode(&self) -> InputMode {
        self.settings.input_mode
    }

    pub fn wants_frames(&self) -> bool {
        self.snow.active() || self.visitors.active()
    }

    pub fn cancel_grab(&mut self, input_at: impl Fn(f32, f32) -> bool) {
        if self
            .gifts
            .grabbed_at()
            .is_some_and(|point| !input_at(point.x, point.y))
        {
            self.gifts.release(&mut self.world, None);
        }
        if let Some((slot, point)) = self.interactions.grabbed_wall() {
            if !input_at(point.x, point.y) {
                self.interactions.release_wall(&mut self.world, slot);
            }
        }
        if self
            .festive
            .grabbed_at()
            .is_some_and(|point| !input_at(point.x, point.y))
        {
            self.festive.release(&mut self.world);
        }
        for slot in 0..MAX_SNOWMEN {
            if let SnowmanMotion::Held { grab, .. } = &self.snowman_motion[slot] {
                let cursor = grab.cursor();
                if !input_at(cursor.x, cursor.y) {
                    self.release_snowman(slot, None);
                }
            }
        }
        if let Some(pet) = self.pets.iter_mut().find(|pet| self.held == Some(pet.id)) {
            if pet
                .app
                .grab_position()
                .is_some_and(|point| !input_at(point.x, point.y))
            {
                pet.app.cancel_grab();
                self.held = None;
            }
        }
    }

    pub fn tick_snow(&mut self, dt: f32, width: u32) {
        if self.width > 0 && self.width != width {
            self.sled.clear(&mut self.pets);
        }
        self.width = width;
        let floor = (self.room.floor - self.bottom).max(0.0) as u32;
        self.snow.tick(dt, width, floor, self.scale, &self.settings);
        if self.world.update_season(self.snow.winter_year()) {
            self.snowman_motion = Default::default();
            self.ramps.clear(&mut self.pets);
            self.gifts.reset(&mut self.pets);
            self.interactions.clear_winter(&mut self.pets);
            self.sled.clear(&mut self.pets);
        }
        if self.snow.active() {
            self.winter_art.update(self.world.winter(), self.scale);
        }
    }

    pub fn overlay_images(&self) -> impl Iterator<Item = (i32, i32, &OverlayImage)> + Clone {
        let floor = (self.room.floor - self.bottom).max(0.0);
        self.snow
            .images()
            .chain(self.winter_art.images(
                self.world.winter(),
                self.width,
                floor,
                std::array::from_fn(|slot| self.snowman_motion[slot].lift()),
                std::array::from_fn(|slot| self.interactions.wall_lift(slot)),
            ))
            .chain(self.interactions.ball_images(&self.winter_art))
            .chain(self.interactions.fragment_images(&self.winter_art))
            .chain(self.sled.images())
            .chain(self.festive.images(self.world.tree()))
            .chain(self.gifts.images(&self.world, self.width, floor))
            .chain(self.visitors.images())
    }

    pub fn prop_input_bounds(&self) -> impl Iterator<Item = Rect> + '_ {
        (0..MAX_SNOWMEN)
            .filter_map(|slot| self.snowman_bounds(slot))
            .chain((0..MAX_SNOW_WALLS).flat_map(|slot| self.wall_ink(slot)))
            .chain(self.festive.input_rects())
            .chain(self.gifts.input_rects(
                &self.world,
                self.width,
                (self.room.floor - self.bottom).max(0.0),
            ))
    }

    fn snowman_bounds(&self, slot: usize) -> Option<Rect> {
        self.winter_art.snowman_bounds(
            slot,
            self.world.winter().snowman(slot)?,
            self.width,
            (self.room.floor - self.bottom).max(0.0) - self.snowman_motion[slot].lift(),
        )
    }

    fn wall_ink(&self, slot: usize) -> impl Iterator<Item = Rect> + '_ {
        self.world
            .winter()
            .walls
            .get(slot)
            .into_iter()
            .flat_map(move |wall| {
                self.winter_art.wall_ink(
                    slot,
                    *wall,
                    self.width,
                    (self.room.floor - self.bottom).max(0.0) - self.interactions.wall_lift(slot),
                )
            })
    }

    fn wall_bounds(&self, slot: usize) -> Option<Rect> {
        self.winter_art.wall_bounds(
            slot,
            *self.world.winter().walls.get(slot)?,
            self.width,
            (self.room.floor - self.bottom).max(0.0) - self.interactions.wall_lift(slot),
        )
    }

    fn release_snowman(&mut self, slot: usize, velocity: Option<Point>) {
        let lift = self.snowman_motion[slot].lift();
        if velocity.is_none() && lift > 0.0 {
            self.snowman_motion[slot] = SnowmanMotion::Falling(Fall::new(lift));
        } else {
            self.finish_snowman_drop(slot, velocity);
        }
    }

    fn finish_snowman_drop(&mut self, slot: usize, velocity: Option<Point>) {
        let lift = self.snowman_motion[slot].lift();
        self.snowman_motion[slot] = SnowmanMotion::Grounded;
        if let Some(velocity) = velocity {
            let height = self
                .snowman_bounds(slot)
                .map_or(8.0, |bounds| bounds.height());
            if let Some(man) = self.world.winter_mut().snowman_mut(slot).take() {
                let floor = (self.room.floor - self.bottom).max(0.0);
                let at = Point::from_xy(man.position * self.width as f32, floor - lift);
                self.interactions
                    .break_snowman(&mut self.pets, slot, at, velocity, height);
            }
        } else {
            self.interactions
                .release_snowman(&mut self.pets, slot, false);
        }
        self.world.save_now();
    }

    pub fn tick(&mut self, dt: f32) {
        for slot in 0..MAX_SNOWMEN {
            if let SnowmanMotion::Falling(fall) = &mut self.snowman_motion[slot] {
                if let Some(impact) = fall.step(dt) {
                    let velocity =
                        (impact >= Fall::BREAK_SPEED).then(|| Point::from_xy(0.0, impact));
                    self.finish_snowman_drop(slot, velocity);
                }
            }
        }
        let check_collisions = self.pets.len() > 1
            && self
                .pets
                .iter()
                .any(|pet| pet.app.strike_velocity().is_some());
        let before = check_collisions.then(|| {
            self.pets
                .iter()
                .map(|pet| CollisionBody {
                    bounds: pet.app.collision_bounds(),
                    velocity: pet.app.strike_velocity(),
                    vulnerable: pet.app.can_be_struck(),
                    snowball_owner: pet.app.snowball_owner,
                    on_ramp: pet.app.pet.activity == Activity::Special(Special::RampSlide),
                })
                .collect::<Vec<_>>()
        });
        for pet in &mut self.pets {
            pet.app.tick(dt, self.snow.active());
        }
        let floor = (self.room.floor - self.bottom).max(0.0);
        self.sled.tick(
            &mut self.pets,
            &mut self.world,
            SledFrame {
                width: self.width,
                floor,
                scale: self.scale,
                elapsed: dt,
                winter: self.snow.active(),
                monitors: &self.sled_monitors,
            },
        );
        if self.snow.active() && self.width > 0 {
            self.ramps.tick(
                &mut self.pets,
                &self.world.winter().walls,
                std::array::from_fn(|slot| self.interactions.wall_grounded(slot)),
                self.width as f32,
                dt,
            );
        }
        if let Some(before) = before {
            self.resolve_collisions(&before);
        } else {
            self.contacts.clear();
        }
        self.festive.tick(
            &mut self.pets,
            &mut self.world,
            FrameInfo {
                date: self.snow.effective_date(),
                width: self.width,
                floor,
                scale: self.scale,
                elapsed: dt,
            },
        );
        self.interactions.tick(
            &mut self.pets,
            &mut self.world,
            self.width,
            floor,
            dt,
            self.snow.active(),
        );
        let kidnapped = self.visitors.tick(
            &mut self.pets,
            &mut self.world,
            VisitorFrame {
                width: self.width,
                floor,
                scale: self.scale,
                elapsed: dt,
            },
            self.snow.active(),
        );
        if let Some(index) = kidnapped.and_then(|id| self.pets.iter().position(|pet| pet.id == id))
        {
            let pack = self.pets.remove(index).app.pack_id().to_owned();
            self.world.remove(&pack);
            if let Some(tx) = &self.lost_chibi {
                let preview = if self.settings.preview_date.is_some() {
                    self.preview_generation
                } else {
                    0
                };
                let _ = tx.send(LostChibi { pack, preview });
            }
        }
        self.gifts.tick(
            &mut self.pets,
            &mut self.world,
            self.width,
            floor,
            self.scale,
            dt,
        );
        if self.snow.active() && self.width != 0 {
            self.winter_art.update(self.world.winter(), self.scale);
        }
    }

    fn resolve_collisions(&mut self, before: &[CollisionBody]) {
        let after: Vec<_> = self
            .pets
            .iter()
            .map(|pet| pet.app.collision_bounds())
            .collect();
        let mut contacts = HashSet::new();
        let mut hits = Vec::new();
        for (left, left_body) in before.iter().enumerate() {
            let Some(left_start) = left_body.bounds else {
                continue;
            };
            for right in (left + 1)..before.len() {
                let right_body = &before[right];
                let (source, target, velocity) = match (left_body.velocity, right_body.velocity) {
                    (Some(a), Some(b)) if b.x.hypot(b.y) > a.x.hypot(a.y) => (right, left, b),
                    (Some(a), _) => (left, right, a),
                    (None, Some(b)) => (right, left, b),
                    (None, None) => continue,
                };
                let (Some(right_start), Some(left_end), Some(right_end)) =
                    (right_body.bounds, after[left], after[right])
                else {
                    continue;
                };
                let pair = (self.pets[left].id, self.pets[right].id);
                if left_end.intersect(&right_end).is_some() {
                    contacts.insert(pair);
                }
                if before[target].vulnerable
                    && before[source].snowball_owner != Some(self.pets[target].id)
                    && !self.contacts.contains(&pair)
                {
                    if let Some(time) = swept_contact(left_start, left_end, right_start, right_end)
                    {
                        hits.push((time, source, target, velocity));
                    }
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut remaining = vec![1.0_f32; before.len()];
        for (_, source, target, velocity) in hits {
            if self.pets[source].app.body_hits_left == Some(0) {
                continue;
            }
            let power = remaining[source];
            let speed = velocity.x.hypot(velocity.y) * power;
            let vx = if velocity.x.abs() < 100.0 {
                let delta = self.pets[target].app.pet.x - self.pets[source].app.pet.x;
                let side = if delta.abs() < 0.01 {
                    if target < source {
                        -1.0
                    } else {
                        1.0
                    }
                } else {
                    delta.signum()
                };
                side * (speed * 0.24).clamp(150.0, 480.0)
            } else {
                (velocity.x * 0.54 * power).clamp(-1800.0, 1800.0)
            };
            let vy = -(170.0 + speed * 0.26).min(840.0);
            if self.pets[target]
                .app
                .knock_back(vx, vy, before[source].snowball_owner)
            {
                if let Some(limit) = &mut self.pets[source].app.body_hits_left {
                    *limit = limit.saturating_sub(1);
                }
                if before[source].on_ramp {
                    self.pets[target].app.body_hits_left = Some(0);
                }
                self.pets[source].app.pet.vx *= 0.84;
                remaining[source] *= 0.84;
            }
        }
        self.contacts = contacts;
    }

    pub fn pointer(&mut self, screen_x: f32, screen_y: f32, kind: Pointer) {
        let now = Instant::now();
        let point = Point::from_xy(screen_x, screen_y);
        let floor = (self.room.floor - self.bottom).max(0.0);
        if self.gifts.grabbed_at().is_some() {
            self.gifts
                .drag(&mut self.world, point, now, self.width, floor, kind);
            return;
        }
        if self.festive.grabbed_at().is_some() {
            self.festive
                .drag(&mut self.world, point, now, self.width, floor, kind);
            return;
        }
        if let Some((slot, _)) = self.interactions.grabbed_wall() {
            if self.width > 0 && matches!(kind, Pointer::Motion | Pointer::LeftUp) {
                let height = self.wall_bounds(slot).map_or(0.0, |bounds| bounds.height());
                if let Some(wall) = self.world.winter_mut().walls.get_mut(slot) {
                    self.interactions.drag_wall(
                        wall,
                        slot,
                        point,
                        now,
                        (self.width as f32, floor, height),
                    );
                    self.world.changed();
                }
                if matches!(kind, Pointer::LeftUp) {
                    self.interactions.release_wall(&mut self.world, slot);
                }
            }
            return;
        }
        if let Some(slot) = self
            .snowman_motion
            .iter()
            .position(|motion| matches!(motion, SnowmanMotion::Held { .. }))
        {
            let height = self
                .snowman_bounds(slot)
                .map_or(0.0, |bounds| bounds.height());
            let SnowmanMotion::Held { grab, lift } = &mut self.snowman_motion[slot] else {
                unreachable!("checked held motion")
            };
            if matches!(kind, Pointer::Motion | Pointer::LeftUp) {
                let base = grab.move_to(point, now);
                let width = self.width as f32;
                *lift = (floor - base.y).clamp(0.0, (floor - height).max(0.0));
                if let Some(man) = self.world.winter_mut().snowman_mut(slot).as_mut() {
                    let margin = (39.0 / width).min(0.5);
                    man.position = (base.x / width).clamp(margin, 1.0 - margin);
                    self.world.changed();
                }
                if matches!(kind, Pointer::LeftUp) {
                    let velocity = grab.throw(now);
                    self.release_snowman(slot, velocity);
                }
            }
            return;
        }
        let index = match kind {
            Pointer::LeftDown if self.held.is_none() => self
                .pets
                .iter()
                .rposition(|pet| pet.app.hit_body(screen_x, screen_y)),
            _ => self.pets.iter().position(|pet| self.held == Some(pet.id)),
        };
        if let Some(index) = index {
            let pet = &mut self.pets[index];
            let (x, y) = pet.app.pointer_from_screen(screen_x, screen_y);
            match kind {
                Pointer::LeftDown => self.held = Some(pet.id),
                Pointer::LeftUp => self.held = None,
                Pointer::Motion => {}
            }
            pet.app.pointer(x, y, kind);
        } else if self.held.is_none() && matches!(kind, Pointer::LeftDown) && self.width > 0 {
            if self.gifts.click(
                &mut self.pets,
                &mut self.world,
                point,
                now,
                self.width,
                floor,
            ) {
                return;
            }
            if self.festive.start_drag(&mut self.pets, point, now) {
                return;
            }
            let hit = |bounds: Rect| {
                (bounds.left()..bounds.right()).contains(&screen_x)
                    && (bounds.top()..bounds.bottom()).contains(&screen_y)
            };
            if let Some(slot) = (0..MAX_SNOW_WALLS)
                .rev()
                .find(|&slot| self.wall_ink(slot).any(hit))
            {
                let wall = self.world.winter().walls[slot];
                let lift = self.interactions.wall_lift(slot);
                let base = Point::from_xy(wall.position * self.width as f32, floor - lift);
                self.interactions
                    .grab_wall(&mut self.pets, slot, Grab::new(point, base, now));
                return;
            }
            if let Some(slot) = (0..MAX_SNOWMEN)
                .rev()
                .find(|&slot| self.snowman_bounds(slot).is_some_and(hit))
            {
                let man = self.world.winter().snowman(slot).expect("drawn snowman");
                let lift = self.snowman_motion[slot].lift();
                let base = Point::from_xy(man.position * self.width as f32, floor - lift);
                self.snowman_motion[slot] = SnowmanMotion::Held {
                    grab: Grab::new(point, base, now),
                    lift,
                };
                self.interactions.snowman_held[slot] = true;
            }
        }
    }
}

// Track when all four overlapping-edge constraints hold during the same tick.
pub(crate) fn swept_contact(a: Rect, next_a: Rect, b: Rect, next_b: Rect) -> Option<f32> {
    let mut enter = 0.0_f32;
    let mut exit = 1.0_f32;
    for (start, end) in [
        (a.right() - b.left(), next_a.right() - next_b.left()),
        (b.right() - a.left(), next_b.right() - next_a.left()),
        (a.bottom() - b.top(), next_a.bottom() - next_b.top()),
        (b.bottom() - a.top(), next_b.bottom() - next_a.top()),
    ] {
        if start < 0.0 {
            if end < 0.0 {
                return None;
            }
            enter = enter.max(-start / (end - start));
        } else if end < 0.0 {
            exit = exit.min(start / (start - end));
        }
    }
    (enter <= exit).then_some(enter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_visitors_remove_one_instance_from_the_test_scene_and_report_its_pack() {
        use crate::season::Date;

        let (tx, rx) = std::sync::mpsc::channel();
        let (report, lost) = std::sync::mpsc::channel();
        let pack = Pack::builtin();
        let mut startup = Startup::test(pack.clone());
        startup.lost_chibi = Some(report);
        let mut game = Game::new(
            crate::startup_room(960.0, 640.0),
            Settings::default(),
            startup,
            rx,
        );
        game.tick_snow(0.0, 960);
        tx.send(Command::Add(pack.clone())).unwrap();
        game.commands();
        for (generation, guest) in [(1, Kind::Ufo), (2, Kind::Santa)] {
            tx.send(Command::Appearance(Settings {
                preview_date: Some(Date::new(15, 6, 2042)),
                ..Settings::default()
            }))
            .unwrap();
            tx.send(Command::Visit(guest)).unwrap();
            game.commands();
            assert!(game.wants_frames());
            let id = game.pets[0].id;
            for _ in 0..160 {
                for (pet, x) in game.pets.iter_mut().zip([340.0, 690.0]) {
                    if pet.app.pet.activity.routine() {
                        pet.app.pet.start(Activity::Idle);
                        pet.app.pet.x = x;
                    }
                }
                game.tick(0.04);
                if game.pets.len() == 1 {
                    break;
                }
            }
            assert_eq!(game.pets.len(), 1);
            assert_ne!(game.pets[0].id, id);
            assert_eq!(game.world.chibis().len(), 1);
            let event = lost.try_recv().unwrap();
            assert_eq!(event.pack, pack.id);
            assert_eq!(event.preview, generation);
            assert!(lost.try_recv().is_err());

            tx.send(Command::Appearance(Settings::default())).unwrap();
            game.commands();
            assert_eq!(game.world.chibis().len(), 2);
            assert_eq!(game.pets.len(), 2);
        }
    }

    #[test]
    fn changing_the_dev_date_parks_and_restores_the_ordinary_pets() {
        use crate::season::Date;
        use crate::world::Snowman;

        let (tx, rx) = std::sync::mpsc::channel();
        let settings = Settings::default();
        let mut game = Game::new(
            crate::startup_room(1200.0, 800.0),
            settings.clone(),
            Startup::test(Pack::builtin()),
            rx,
        );
        game.tick_snow(0.0, 1200);
        game.pets[0].app.pet.x = 365.0;
        game.world.winter_mut().snowman = Some(Snowman {
            position: 0.46,
            seconds: 212.0,
        });
        tx.send(Command::Appearance(Settings {
            preview_date: Some(Date::new(15, 1, 2041)),
            ..settings.clone()
        }))
        .unwrap();
        game.commands();
        assert!(game.snow.active());
        assert_ne!(game.pets[0].id, 0);
        assert!(game.world.winter().snowman.is_none());
        tx.send(Command::Remove(crate::pack::BUILTIN_ID.into()))
            .unwrap();
        game.commands();
        assert!(game.pets.is_empty());
        tx.send(Command::Appearance(settings)).unwrap();
        game.commands();
        assert_eq!(game.pets[0].id, 0);
        assert_eq!(game.pets[0].app.pet.x, 365.0);
        assert_eq!(game.world.winter().snowman.unwrap().seconds, 212.0);
    }

    #[test]
    fn fast_throws_knock_down_multiple_chibis_and_miss_bodies_outside_the_path() {
        use crate::sim::Activity;

        let (_tx, rx) = std::sync::mpsc::channel();
        let settings = Settings {
            scale: "0.2".into(),
            ..Settings::default()
        };
        let mut game = Game::new(
            crate::startup_room(1400.0, 800.0),
            settings,
            Startup::test(Pack::builtin()),
            rx,
        );
        for _ in 0..3 {
            game.add(Pack::builtin());
        }
        for (pet, x) in game.pets.iter_mut().zip([200.0, 255.0, 320.0, 275.0]) {
            pet.app.pet.x = x;
        }
        let thrower = &mut game.pets[0].app;
        thrower.pet.y -= 15.0;
        thrower.knock_back(3600.0, -80.0, None);
        let above = &mut game.pets[3].app;
        above.pet.y -= 210.0;
        above.pet.release(&above.room, 0.0, 0.0);

        game.tick(0.05);

        for target in &game.pets[1..3] {
            assert_eq!(target.app.pet.activity, Activity::Fall);
            assert!(target.app.pet.vx > 0.0 && target.app.pet.vy < 0.0);
            assert!(target.app.pet.can_strike());
        }
        assert_eq!(game.pets[3].app.pet.vx, 0.0);
        assert!(game.pets[0].app.pet.vx < 3600.0);
    }

    #[test]
    fn a_knocked_chibi_can_hit_another_in_the_next_tick() {
        use crate::sim::Activity;

        let (_tx, rx) = std::sync::mpsc::channel();
        let settings = Settings {
            scale: "0.2".into(),
            ..Settings::default()
        };
        let mut game = Game::new(
            crate::startup_room(1400.0, 800.0),
            settings,
            Startup::test(Pack::builtin()),
            rx,
        );
        game.add(Pack::builtin());
        game.add(Pack::builtin());
        for (pet, x) in game.pets.iter_mut().zip([200.0, 250.0, 295.0]) {
            pet.app.pet.x = x;
        }
        game.pets[0].app.pet.y -= 12.0;
        game.pets[0].app.knock_back(1200.0, -80.0, None);
        game.tick(0.04);
        assert_eq!(game.pets[1].app.pet.activity, Activity::Fall);
        assert_eq!(game.pets[2].app.pet.activity, Activity::Idle);

        // Keep the original thrower out of the next contact.
        game.pets[0].app.pet.grab();
        game.tick(0.05);
        assert_eq!(game.pets[2].app.pet.activity, Activity::Fall);
        assert!(game.pets[2].app.pet.vx > 0.0);
    }

    #[test]
    fn snowball_knockback_spares_the_thrower_and_topples_only_one_bystander() {
        use crate::sim::Activity;

        let (_tx, rx) = std::sync::mpsc::channel();
        let mut game = Game::new(
            crate::startup_room(1400.0, 800.0),
            Settings {
                scale: "0.2".into(),
                ..Settings::default()
            },
            Startup::test(Pack::builtin()),
            rx,
        );
        game.add(Pack::builtin());
        game.add(Pack::builtin());
        game.add(Pack::builtin());
        let shooter = game.pets[0].id;
        for (pet, x) in game.pets.iter_mut().zip([250.0, 200.0, 315.0, 393.0]) {
            pet.app.pet.x = x;
        }
        let victim = &mut game.pets[1].app;
        victim.pet.y -= 15.0;
        let body = victim.collision_bounds().unwrap();
        let impact = Point::from_xy(body.left(), (body.top() + body.bottom()) * 0.5);
        assert!(victim.hit_with_snowball(impact, 3600.0, -80.0, shooter));
        game.tick(0.05);
        assert!(game.pets[0].app.pet.activity.routine());
        assert_eq!(game.pets[2].app.pet.activity, Activity::Fall);
        assert_eq!(game.pets[2].app.snowball_owner, Some(shooter));
        assert_eq!(game.pets[2].app.body_hits_left, Some(0));
        assert_eq!(game.pets[3].app.pet.activity, Activity::Idle);
        game.tick(0.05);
        assert_eq!(game.pets[3].app.pet.activity, Activity::Idle);
    }

    #[test]
    fn moving_and_breaking_one_snowman_spares_the_other() {
        use crate::season::Date;
        use crate::world::Snowman;

        let (_tx, rx) = std::sync::mpsc::channel();
        let mut game = Game::new(
            crate::startup_room(1200.0, 800.0),
            Settings::default(),
            Startup::test(Pack::builtin()),
            rx,
        );
        game.snow.set_date(Some(Date::new(15, 1, 2041)));
        game.tick_snow(0.0, 1200);
        for (slot, (position, seconds)) in [(0.27, 218.0), (0.74, 498.0)].into_iter().enumerate() {
            *game.world.winter_mut().snowman_mut(slot) = Some(Snowman { position, seconds });
        }
        game.tick_snow(0.0, 1200);
        let bounds = game.snowman_bounds(1).unwrap();
        let x = (bounds.left() + bounds.right()) * 0.5;
        let y = bounds.bottom() - 3.0;
        game.pointer(x, y, Pointer::LeftDown);
        game.pointer(x + 65.0, y - 27.0, Pointer::Motion);
        assert_eq!(game.snowman_motion[0].lift(), 0.0);
        assert_eq!(game.snowman_motion[1].lift(), 27.0);
        game.cancel_grab(|_, _| false);
        for _ in 0..16 {
            game.tick(0.02);
        }
        assert!(matches!(game.snowman_motion[1], SnowmanMotion::Grounded));
        let moved = game.world.winter().snowman(1).unwrap();
        assert!((moved.position - (0.74 + 65.0 / 1200.0)).abs() < 1.0e-4);
        assert_eq!(moved.seconds, 498.0);
        game.finish_snowman_drop(1, Some(Point::from_xy(900.0, 0.0)));
        assert!(game.world.winter().snowman(1).is_none());
        let kept = game.world.winter().snowman(0).unwrap();
        assert_eq!((kept.position, kept.seconds), (0.27, 218.0));
    }

    #[test]
    fn a_wall_can_be_lifted_and_keeps_its_construction_progress() {
        use crate::season::Date;
        use crate::world::SnowWall;

        let (_tx, rx) = std::sync::mpsc::channel();
        let mut game = Game::new(
            crate::startup_room(1200.0, 800.0),
            Settings::default(),
            Startup::test(Pack::builtin()),
            rx,
        );
        game.snow.set_date(Some(Date::new(15, 1, 2041)));
        game.tick_snow(0.0, 1200);
        game.world.winter_mut().walls.push(SnowWall {
            slope_right: true,
            position: 0.26,
            seconds: 347.0,
        });
        game.tick_snow(0.0, 1200);
        let bounds = game.wall_bounds(0).unwrap();
        let x = (bounds.left() + bounds.right()) * 0.5;
        let y = bounds.bottom() - 4.0;
        game.pointer(x, y, Pointer::LeftDown);
        game.pointer(x + 145.0, y - 38.0, Pointer::Motion);
        assert_eq!(game.interactions.wall_lift(0), 38.0);
        game.pointer(x + 145.0, y - 38.0, Pointer::LeftUp);
        for _ in 0..20 {
            game.tick(0.02);
        }
        let wall = game.world.winter().walls[0];
        assert!((wall.position - (0.26 + 145.0 / 1200.0)).abs() < 1.0e-4);
        assert_eq!(wall.seconds, 347.0);
        assert_eq!(game.interactions.wall_lift(0), 0.0);
    }

    #[test]
    fn grab_follows_the_drawn_order_and_stays_with_one_chibi() {
        let (_tx, rx) = std::sync::mpsc::channel();
        let mut game = Game::new(
            crate::startup_room(1400.0, 800.0),
            Settings::default(),
            Startup::test(Pack::builtin()),
            rx,
        );
        game.add(Pack::builtin());

        let paint = |game: &mut Game, older_x, newer_x| {
            let mut point = (0.0, 0.0);
            for (pet, x) in game.pets.iter_mut().zip([older_x, newer_x]) {
                let app = &mut pet.app;
                app.pet.x = x;
                app.prepare();
                let frame = app.render();
                if pet.id == 0 {
                    let (index, _) = frame
                        .bgra
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                        .find(|(_, pixel)| pixel[3] == 255)
                        .expect("visible chibi");
                    let width = frame.width as usize;
                    point = (
                        app.origin_x as f32 + (index % width) as f32 + 0.5,
                        app.origin_y as f32 + (index / width) as f32 + 0.5,
                    );
                }
                app.note_committed();
            }
            point
        };

        let (x, y) = paint(&mut game, 600.0, 600.0);
        assert!(game.pets.iter().all(|pet| pet.app.hit_body(x, y)));
        game.pointer(x, y, Pointer::LeftDown);
        assert_eq!(game.held, Some(1));
        game.pointer(x, y, Pointer::LeftUp);

        let (x, y) = paint(&mut game, 900.0, 250.0);
        assert!(game.pets[0].app.hit_body(x, y));
        assert!(!game.pets[1].app.hit_body(x, y));
        game.pointer(x, y, Pointer::LeftDown);
        game.pointer(x - 650.0, y, Pointer::Motion);
        assert_eq!(game.held, Some(0));
        game.pointer(x - 650.0, y, Pointer::LeftUp);
        assert_eq!(game.held, None);
    }

    #[test]
    fn removing_a_chibi_only_removes_one_instance() {
        let (tx, rx) = std::sync::mpsc::channel();
        let first = Pack::builtin();
        let id = first.id.clone();
        let mut game = Game::new(
            crate::startup_room(1000.0, 700.0),
            Settings::default(),
            Startup::test(first),
            rx,
        );
        tx.send(Command::Add(Pack::builtin())).unwrap();
        assert!(game.commands());
        assert_ne!(game.pets[0].app.pet.x, game.pets[1].app.pet.x);
        tx.send(Command::Remove(id)).unwrap();
        assert!(game.commands());
        assert_eq!(game.pets.len(), 1);
        assert_eq!(game.pets[0].id, 0);
    }
}
