// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use crate::config::{system_dark, LanguagePref, Settings, ThemePref};
use crate::draw::{self, Frame, Hit, RowId, Scene};
use crate::i18n::Catalog;
use crate::sim::{Pet, Room, FEET_X, FEET_Y, SPRITE_H, SPRITE_W};

const MENU_W: f32 = draw::MENU_W;

pub struct App {
    pub pet: Pet,
    pub room: Room,
    settings: Settings,
    catalog: Catalog,
    menu: bool,
    dragging: bool,
    grab_x: f32,
    grab_y: f32,
    committed_x: i32,
    committed_y: i32,
    last_screen: Option<(f32, f32, f32)>,
    pub dark: bool,
    pub exit: bool,
    pub scale: u32,
    hits: Vec<Hit>,
    pub logical_w: u32,
    pub logical_h: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    clock: f32,
}

impl App {
    pub fn new(room: Room) -> Self {
        let settings = Settings::load();
        let catalog = Catalog::load(&settings.code());
        let pet = Pet::on_floor(&room);
        let dark = match settings.theme {
            ThemePref::System => system_dark(),
            ThemePref::Light => false,
            ThemePref::Dark => true,
        };
        let mut app = Self {
            pet,
            room,
            settings,
            catalog,
            menu: false,
            dragging: false,
            grab_x: 0.0,
            grab_y: 0.0,
            committed_x: 0,
            committed_y: 0,
            last_screen: None,
            dark,
            exit: false,
            scale: 1,
            hits: Vec::new(),
            logical_w: SPRITE_W as u32,
            logical_h: SPRITE_H as u32,
            origin_x: 0,
            origin_y: 0,
            clock: 0.0,
        };
        app.sync_geometry();
        app.committed_x = app.origin_x;
        app.committed_y = app.origin_y;
        app
    }

    pub fn set_room(&mut self, room: Room) {
        self.room = room;
        if !self.dragging && self.pet.activity != crate::sim::Activity::Fall {
            self.pet.y = self.room.floor;
            self.pet.x = self.pet.x.clamp(self.room.left, self.room.right);
        }
        self.sync_geometry();
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        if !self.dragging {
            self.pet.step(&self.room, dt);
            self.sync_geometry();
        }
    }

    pub fn pointer(&mut self, lx: f32, ly: f32, kind: Pointer) {
        match kind {
            Pointer::LeftDown => {
                if let Some(hit) = self.hit(lx, ly) {
                    self.activate(hit);
                    return;
                }
                self.menu = false;
                self.dragging = true;
                self.pet.grab();
                self.grab_x = lx;
                self.grab_y = ly;
                self.last_screen = None;
            }
            Pointer::Motion if self.dragging => {
                let screen_x = self.committed_x as f32 + lx;
                let screen_y = self.committed_y as f32 + ly;
                self.origin_x = (screen_x - self.grab_x).round() as i32;
                self.origin_y = (screen_y - self.grab_y).round() as i32;
                self.pet_from_origin();
                let screen_feet_x = self.pet.x;
                let screen_feet_y = self.pet.y;
                if let Some((time, x, y)) = self.last_screen {
                    let dt = (self.clock - time).max(0.001);
                    self.pet.vx = (screen_feet_x - x) / dt;
                    self.pet.vy = (screen_feet_y - y) / dt;
                }
                self.last_screen = Some((self.clock, screen_feet_x, screen_feet_y));
                self.sync_geometry();
            }
            Pointer::LeftUp if self.dragging => {
                self.dragging = false;
                self.pet.release(&self.room, self.pet.vx, self.pet.vy);
                self.sync_geometry();
            }
            Pointer::RightDown => {
                if !self.dragging {
                    self.menu = !self.menu;
                    self.sync_geometry();
                }
            }
            Pointer::Motion | Pointer::LeftUp => {}
        }
    }

    pub fn key_escape(&mut self) {
        if self.menu {
            self.menu = false;
            self.sync_geometry();
        }
    }

    pub fn render(&mut self) -> Frame {
        let scene = Scene {
            swing: self.pet.swing(),
            tilt: self.pet.tilt(),
            facing_left: self.pet.face < 0.0,
            menu: self.menu,
            menu_left: self.menu_left(),
            dark: self.dark,
            rows: draw::rows(
                &self.catalog,
                match self.settings.language {
                    LanguagePref::System => "system",
                    LanguagePref::En => "en",
                    LanguagePref::Ru => "ru",
                },
                match self.settings.theme {
                    ThemePref::System => "system",
                    ThemePref::Light => "light",
                    ThemePref::Dark => "dark",
                },
            ),
        };
        let frame = draw::render(&scene, self.scale);
        self.hits = frame.hits.clone();
        frame
    }

    pub fn note_committed(&mut self) {
        self.committed_x = self.origin_x;
        self.committed_y = self.origin_y;
    }

    fn activate(&mut self, id: RowId) {
        match id {
            RowId::SystemLang => self.settings.language = LanguagePref::System,
            RowId::English => self.settings.language = LanguagePref::En,
            RowId::Russian => self.settings.language = LanguagePref::Ru,
            RowId::SystemTheme => self.settings.theme = ThemePref::System,
            RowId::Light => self.settings.theme = ThemePref::Light,
            RowId::Dark => self.settings.theme = ThemePref::Dark,
            RowId::Quit => {
                self.exit = true;
                return;
            }
            RowId::Name | RowId::Language | RowId::Theme => return,
        }
        self.dark = match self.settings.theme {
            ThemePref::System => system_dark(),
            ThemePref::Light => false,
            ThemePref::Dark => true,
        };
        self.catalog = Catalog::load(&self.settings.code());
        if let Err(error) = self.settings.save() {
            eprintln!("{error}");
        }
        self.sync_geometry();
    }

    fn hit(&self, x: f32, y: f32) -> Option<RowId> {
        self.hits
            .iter()
            .find(|hit| x >= hit.x && y >= hit.y && x < hit.x + hit.w && y < hit.y + hit.h)
            .map(|hit| hit.id)
    }

    fn menu_left(&self) -> bool {
        let room_right = self.room.right + FEET_X;
        self.pet.x + FEET_X + MENU_W > room_right
    }

    fn pet_from_origin(&mut self) {
        let pet_x = if self.menu && self.menu_left() {
            MENU_W
        } else {
            0.0
        };
        self.pet.x = self.origin_x as f32 + pet_x + FEET_X;
        self.pet.y = self.origin_y as f32 + FEET_Y;
    }

    fn sync_geometry(&mut self) {
        let (width, height) = draw::scene_size(self.menu);
        self.logical_w = width;
        self.logical_h = height;
        let pet_x = if self.menu && self.menu_left() {
            MENU_W
        } else {
            0.0
        };
        self.origin_x = (self.pet.x - pet_x - FEET_X).round() as i32;
        self.origin_y = (self.pet.y - FEET_Y).round() as i32;
    }
}

pub enum Pointer {
    Motion,
    LeftDown,
    LeftUp,
    RightDown,
}

pub fn startup_room(width: f32, height: f32) -> Room {
    let margin = 16.0;
    Room {
        left: margin + FEET_X,
        right: (width - margin - FEET_X).max(margin + FEET_X + 1.0),
        floor: (height - 12.0).max(FEET_Y),
    }
}
