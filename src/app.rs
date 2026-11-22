// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use crate::config::{system_dark, LanguagePref, Settings, ThemePref};
use crate::draw::{self, Frame, Hit, Motion, RowId, Scene};
use crate::i18n::{Catalog, ID_NAME};
use crate::pack::{self, Choice, Pack, BUILTIN_ID};
use crate::sim::{Activity, Pet, Room, FEET_X, FEET_Y, SPRITE_H, SPRITE_W};

const MENU_W: f32 = draw::MENU_W;

pub struct App {
    pub pet: Pet,
    pub room: Room,
    settings: Settings,
    catalog: Catalog,
    pack: Pack,
    choices: Vec<Choice>,
    menu: bool,
    dragging: bool,
    grab_x: f32,
    grab_y: f32,
    committed_x: i32,
    committed_y: i32,
    committed_pet_x: f32,
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
        let pack = Pack::open(&settings.chibi);
        let choices = pack::choices();
        let mut app = Self {
            pet,
            room,
            settings,
            catalog,
            pack,
            choices,
            menu: false,
            dragging: false,
            grab_x: 0.0,
            grab_y: 0.0,
            committed_x: 0,
            committed_y: 0,
            committed_pet_x: 0.0,
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
        app.note_committed();
        app
    }

    pub fn set_room(&mut self, room: Room) {
        self.room = room;
        if !self.dragging && self.pet.activity != Activity::Fall {
            self.pet.y = self.room.floor;
            self.pet.x = self.pet.x.clamp(self.room.left, self.room.right);
        }
        self.sync_geometry();
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        if !self.dragging {
            self.pet.step(&self.room, dt);
        }
    }

    pub fn prepare(&mut self) {
        if !self.dragging {
            self.sync_geometry();
        }
    }

    /// Screen position to the frame drawn at the last committed origin.
    pub fn pointer_from_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (x - self.committed_x as f32, y - self.committed_y as f32)
    }

    pub fn pointer(&mut self, lx: f32, ly: f32, kind: Pointer) {
        match kind {
            Pointer::LeftDown => {
                if let Some((id, key)) = self.hit(lx, ly) {
                    self.activate(id, &key);
                    return;
                }
                if self.in_menu(lx, ly) {
                    return;
                }
                self.dragging = true;
                self.pet.grab();
                self.grab_x = lx;
                self.grab_y = ly;
                self.last_screen = None;
            }
            Pointer::Motion if self.dragging => {
                // Pointer events are measured against the last committed surface, not this drag.
                self.origin_x = (self.committed_x as f32 + lx - self.grab_x).round() as i32;
                self.origin_y = (self.committed_y as f32 + ly - self.grab_y).round() as i32;
                self.pet.x = self.origin_x as f32 + self.committed_pet_x + FEET_X;
                self.pet.y = self.origin_y as f32 + FEET_Y;
                if let Some((time, x, y)) = self.last_screen {
                    let dt = (self.clock - time).max(0.001);
                    self.pet.vx = (self.pet.x - x) / dt;
                    self.pet.vy = (self.pet.y - y) / dt;
                }
                self.last_screen = Some((self.clock, self.pet.x, self.pet.y));
            }
            Pointer::LeftUp if self.dragging => {
                self.dragging = false;
                self.menu = false;
                self.pet.release(&self.room, self.pet.vx, self.pet.vy);
                self.sync_geometry();
            }
            Pointer::RightDown => {
                if !self.dragging {
                    self.menu = !self.menu;
                    if self.menu {
                        self.choices = pack::choices();
                    }
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
            cycle: self.pet.phase,
            motion: match self.pet.activity {
                Activity::Walk => Motion::Walk,
                Activity::Fall => Motion::Fall,
                Activity::Drag => Motion::Drag,
                Activity::Idle => Motion::Idle,
            },
            tilt: self.pet.tilt(),
            facing_left: self.pet.face < 0.0,
            menu: self.menu,
            menu_left: self.menu_left(),
            dark: self.dark,
            rows: draw::rows(
                &self.catalog,
                &self.pet_title(),
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
                &self.choices,
                &self.pack.id,
            ),
        };
        let frame = draw::render(&scene, &self.pack, self.scale);
        self.hits = frame.hits.clone();
        frame
    }

    pub fn note_committed(&mut self) {
        self.committed_x = self.origin_x;
        self.committed_y = self.origin_y;
        self.committed_pet_x = self.layout_pet_x();
    }

    fn activate(&mut self, id: RowId, key: &str) {
        match id {
            RowId::SystemLang => self.settings.language = LanguagePref::System,
            RowId::English => self.settings.language = LanguagePref::En,
            RowId::Russian => self.settings.language = LanguagePref::Ru,
            RowId::SystemTheme => self.settings.theme = ThemePref::System,
            RowId::Light => self.settings.theme = ThemePref::Light,
            RowId::Dark => self.settings.theme = ThemePref::Dark,
            RowId::Chibi => {
                self.settings.chibi = key.to_string();
                self.pack = Pack::open(key);
            }
            RowId::Quit => {
                self.exit = true;
                return;
            }
            RowId::Name | RowId::ChibiLabel | RowId::Language | RowId::Theme => return,
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

    fn hit(&self, x: f32, y: f32) -> Option<(RowId, String)> {
        self.hits
            .iter()
            .find(|hit| x >= hit.x && y >= hit.y && x < hit.x + hit.w && y < hit.y + hit.h)
            .map(|hit| (hit.id, hit.key.clone()))
    }

    fn pet_title(&self) -> String {
        if self.pack.id == BUILTIN_ID {
            self.catalog.text(ID_NAME).to_string()
        } else {
            self.pack.name.clone()
        }
    }

    fn menu_left(&self) -> bool {
        let room_right = self.room.right + FEET_X;
        self.pet.x + FEET_X + MENU_W > room_right
    }

    fn in_menu(&self, x: f32, y: f32) -> bool {
        if !self.menu || y < 0.0 || y >= SPRITE_H {
            return false;
        }
        let menu_x = if self.menu_left() {
            0.0
        } else {
            self.layout_pet_x() + SPRITE_W
        };
        x >= menu_x && x < menu_x + MENU_W
    }

    fn layout_pet_x(&self) -> f32 {
        if self.menu && self.menu_left() {
            MENU_W
        } else {
            0.0
        }
    }

    fn sync_geometry(&mut self) {
        let (width, height) = draw::scene_size(self.menu);
        self.logical_w = width;
        self.logical_h = height;
        let pet_x = self.layout_pet_x();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_follows_the_latest_point_until_the_surface_commits() {
        let mut app = App::new(startup_room(800.0, 600.0));
        let origin_x = app.origin_x;
        let origin_y = app.origin_y;
        app.pointer(40.0, 80.0, Pointer::LeftDown);
        app.pointer(50.0, 90.0, Pointer::Motion);
        app.pointer(70.0, 100.0, Pointer::Motion);
        assert_eq!(app.origin_x, origin_x + 30);
        assert_eq!(app.origin_y, origin_y + 20);
        assert_eq!(app.committed_x, origin_x);
    }

    #[test]
    fn menu_padding_does_not_start_a_drag() {
        let mut app = App::new(startup_room(800.0, 600.0));
        app.pointer(20.0, 20.0, Pointer::RightDown);
        app.render();
        app.pointer(SPRITE_W + 12.0, 20.0, Pointer::LeftDown);
        assert_ne!(app.pet.activity, Activity::Drag);
        app.pointer(80.0, 120.0, Pointer::LeftDown);
        assert_eq!(app.pet.activity, Activity::Drag);
    }

    #[test]
    fn screen_pointer_uses_the_committed_window() {
        let app = App::new(startup_room(800.0, 600.0));
        let (x, y) =
            app.pointer_from_screen(app.origin_x as f32 + 12.0, app.origin_y as f32 + 40.0);
        assert_eq!((x, y), (12.0, 40.0));
    }

    #[test]
    fn menu_click_selects_the_row_under_the_pointer() {
        let mut app = App::new(startup_room(1200.0, 800.0));
        app.pointer(40.0, 40.0, Pointer::RightDown);
        let frame = app.render();
        let light = frame
            .hits
            .iter()
            .find(|hit| hit.id == RowId::Light)
            .expect("light row");
        app.pointer(
            light.x + light.w * 0.5,
            light.y + light.h * 0.5,
            Pointer::LeftDown,
        );
        assert!(!app.dark);
        assert!(!app.exit);
        let dark = frame
            .hits
            .iter()
            .find(|hit| hit.id == RowId::Dark)
            .expect("dark row");
        app.pointer(
            dark.x + dark.w * 0.5,
            dark.y + dark.h * 0.5,
            Pointer::LeftDown,
        );
        assert!(app.dark);
        let quit = frame
            .hits
            .iter()
            .find(|hit| hit.id == RowId::Quit)
            .expect("quit row");
        app.pointer(
            quit.x + quit.w * 0.5,
            quit.y + quit.h * 0.5,
            Pointer::LeftDown,
        );
        assert!(app.exit);
    }
}
