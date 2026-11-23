// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::mpsc::Receiver;

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
        for pet in &mut self.pets {
            pet.app.tick(dt);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
