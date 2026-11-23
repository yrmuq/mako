// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;
use std::sync::mpsc::Receiver;

use tiny_skia::{Point, Rect};

use crate::app::{App, Pointer};
use crate::config::Settings;
use crate::pack::Pack;
use crate::sim::Room;

pub enum Command {
    Add(Pack),
    Remove(String),
    Appearance(Settings),
}

pub struct Instance {
    pub id: u64,
    pub app: App,
}

pub struct Game {
    pub pets: Vec<Instance>,
    room: Room,
    settings: Settings,
    commands: Receiver<Command>,
    bottom: f32,
    scale: u32,
    next_id: u64,
    held: Option<u64>,
    contacts: HashSet<(u64, u64)>,
    pub exit: bool,
}

impl Game {
    pub fn new(room: Room, settings: Settings, first: Pack, commands: Receiver<Command>) -> Self {
        let mut game = Self {
            pets: Vec::new(),
            room,
            settings,
            commands,
            bottom: 0.0,
            scale: 1,
            next_id: 0,
            held: None,
            contacts: HashSet::new(),
            exit: false,
        };
        game.add(first);
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
                Ok(Command::Add(pack)) => self.add(pack),
                Ok(Command::Remove(id)) => {
                    if let Some(index) = self.pets.iter().rposition(|pet| pet.app.pack_id() == id) {
                        if self.held == Some(self.pets[index].id) {
                            self.held = None;
                        }
                        self.pets.remove(index);
                    }
                }
                Ok(Command::Appearance(settings)) => {
                    self.settings = settings;
                    for pet in &mut self.pets {
                        pet.app.apply_settings(self.settings.clone());
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return changed,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.exit = true;
                    return changed;
                }
            }
            changed = true;
        }
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

    pub fn scale(&self) -> u32 {
        self.scale
    }

    pub fn tick(&mut self, dt: f32) {
        let check_collisions =
            self.pets.len() > 1 && self.pets.iter().any(|pet| pet.app.pet.can_strike());
        let before = check_collisions.then(|| {
            self.pets
                .iter()
                .map(|pet| {
                    let body = &pet.app.pet;
                    let velocity = body
                        .can_strike()
                        .then_some(Point::from_xy(body.vx, body.vy));
                    (pet.app.collision_bounds(), velocity)
                })
                .collect::<Vec<_>>()
        });
        for pet in &mut self.pets {
            pet.app.tick(dt);
        }
        if let Some(before) = before {
            self.resolve_collisions(&before);
        } else {
            self.contacts.clear();
        }
    }

    fn resolve_collisions(&mut self, before: &[(Option<Rect>, Option<Point>)]) {
        let after: Vec<_> = self
            .pets
            .iter()
            .map(|pet| pet.app.collision_bounds())
            .collect();
        let mut contacts = HashSet::new();
        let mut hits = Vec::new();
        for (left, &(left_start, left_velocity)) in before.iter().enumerate() {
            let Some(left_start) = left_start else {
                continue;
            };
            for right in (left + 1)..before.len() {
                let (source, target, velocity) = match (left_velocity, before[right].1) {
                    (Some(a), Some(b)) if b.x.hypot(b.y) > a.x.hypot(a.y) => (right, left, b),
                    (Some(a), _) => (left, right, a),
                    (None, Some(b)) => (right, left, b),
                    (None, None) => continue,
                };
                let (Some(right_start), Some(left_end), Some(right_end)) =
                    (before[right].0, after[left], after[right])
                else {
                    continue;
                };
                let pair = (self.pets[left].id, self.pets[right].id);
                if left_end.intersect(&right_end).is_some() {
                    contacts.insert(pair);
                }
                if !self.contacts.contains(&pair) {
                    if let Some(time) = swept_contact(left_start, left_end, right_start, right_end)
                    {
                        hits.push((time, source, target, velocity));
                    }
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut struck = HashSet::new();
        let mut remaining = vec![1.0_f32; before.len()];
        for (_, source, target, velocity) in hits {
            if struck.insert(target) {
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
                self.pets[target].app.knock_back(vx, vy);
                self.pets[source].app.pet.vx *= 0.84;
                remaining[source] *= 0.84;
            }
        }
        self.contacts = contacts;
    }

    pub fn pointer(&mut self, screen_x: f32, screen_y: f32, kind: Pointer) {
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
        }
    }
}

// Track when all four overlapping-edge constraints hold during the same tick.
fn swept_contact(a: Rect, next_a: Rect, b: Rect, next_b: Rect) -> Option<f32> {
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
            Pack::builtin(),
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
        thrower.knock_back(3600.0, -80.0);
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
            Pack::builtin(),
            rx,
        );
        game.add(Pack::builtin());
        game.add(Pack::builtin());
        for (pet, x) in game.pets.iter_mut().zip([200.0, 250.0, 295.0]) {
            pet.app.pet.x = x;
        }
        game.pets[0].app.pet.y -= 12.0;
        game.pets[0].app.knock_back(1200.0, -80.0);
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
    fn grab_follows_the_drawn_order_and_stays_with_one_chibi() {
        let (_tx, rx) = std::sync::mpsc::channel();
        let mut game = Game::new(
            crate::startup_room(1400.0, 800.0),
            Settings::default(),
            Pack::builtin(),
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
            first,
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
