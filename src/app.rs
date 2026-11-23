// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use tiny_skia::{Point, Rect};

use crate::config::Settings;
use crate::draw::{self, Frame, Motion, Part, Scene};
use crate::pack::{BonePoint, Pack};
use crate::rag::Ragdoll;
use crate::sim::{Activity, Pet, Room, FEET_X, FEET_Y, GRAVITY, SPRITE_H, SPRITE_W};

struct Grip {
    point: BonePoint,
    cursor: Point,
}

pub struct App {
    pub pet: Pet,
    pub room: Room,
    settings: Settings,
    pack: Pack,
    rag: Ragdoll,
    grip: Option<Grip>,
    committed_x: i32,
    committed_y: i32,
    parts: Vec<Part>,
    committed_parts: Vec<Part>,
    trail: Vec<(f32, f32, f32)>,
    prev_vx: f32,
    prev_vy: f32,
    screen: (f32, f32),
    bottom: f32,
    pub scale: u32,
    pub logical_w: u32,
    pub logical_h: u32,
    pub origin_x: i32,
    pub origin_y: i32,
    clock: f32,
    blink_at: f32,
    blink_length: f32,
}

impl App {
    pub fn new(room: Room, settings: Settings, pack: Pack) -> Self {
        let pet = Pet::on_floor(&room, pack.animation_length("walk"));
        let blink_length = pack.animation_length("blink").unwrap_or(0.0);
        let rag = Ragdoll::new(&pack);
        let screen = screen_of(&room);
        let mut app = Self {
            pet,
            room,
            settings,
            pack,
            rag,
            grip: None,
            committed_x: 0,
            committed_y: 0,
            parts: Vec::new(),
            committed_parts: Vec::new(),
            trail: Vec::new(),
            prev_vx: 0.0,
            prev_vy: 0.0,
            screen,
            bottom: 0.0,
            scale: 1,
            logical_w: SPRITE_W as u32,
            logical_h: SPRITE_H as u32,
            origin_x: 0,
            origin_y: 0,
            clock: 0.0,
            blink_at: next_blink(0.0),
            blink_length,
        };
        app.fit_room();
        app.pet.y = app.room.floor;
        app.pet.x = app.pet.x.clamp(app.room.left, app.room.right);
        app.sync_geometry();
        app.note_committed();
        app
    }

    pub fn set_bottom(&mut self, bottom: f32) {
        self.bottom = bottom.max(0.0);
    }

    pub fn set_room(&mut self, room: Room) {
        self.screen = screen_of(&room);
        self.fit_room();
        if self.grip.is_none()
            && self.pet.activity != Activity::Fall
            && self.pet.activity != Activity::Down
        {
            self.pet.y = self.room.floor;
            self.pet.x = self.pet.x.clamp(self.room.left, self.room.right);
        }
        self.sync_geometry();
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        if self.clock >= self.blink_at + self.blink_length {
            self.blink_at = next_blink(self.clock);
        }
        let dt = dt.clamp(0.0, 0.05);
        self.sync_rag();
        if let Some(grip) = &self.grip {
            self.pet.phase += dt;
            if self
                .trail
                .last()
                .is_some_and(|sample| self.clock - sample.0 > 0.05)
            {
                self.pet.vx = 0.0;
                self.pet.vy = 0.0;
            }
            let ax = ((self.pet.vx - self.prev_vx) / dt.max(0.001)).clamp(-8000.0, 8000.0);
            let ay = ((self.pet.vy - self.prev_vy) / dt.max(0.001)).clamp(-8000.0, 8000.0);
            self.prev_vx = self.pet.vx;
            self.prev_vy = self.pet.vy;
            let pull = Point::from_xy(-ax * self.pet.face, GRAVITY - ay);
            self.rag.suspend(dt, pull, &self.pack, grip.point);
        } else {
            let vx = self.pet.vx;
            let vy = self.pet.vy;
            self.pet.step(&self.room, dt);
            if self.pet.activity == Activity::Fall {
                let ax = ((self.pet.vx - vx) / dt.max(0.001)).clamp(-14000.0, 14000.0);
                let ay = ((self.pet.vy - vy) / dt.max(0.001)).clamp(-14000.0, 14000.0);
                self.rag.step(dt, ax, ay, self.settings.pet_scale());
            } else if self.pet.activity == Activity::Down {
                let ax = ((self.pet.vx - vx) / dt.max(0.001)).clamp(-9000.0, 9000.0);
                // The landing frame still feels the floor. After that, gravity must not
                // stand her up while she skids.
                let ay = if vy > 80.0 {
                    ((self.pet.vy - vy) / dt.max(0.001)).clamp(-14000.0, 14000.0)
                } else {
                    crate::sim::GRAVITY
                };
                self.rag.plant(vx * self.pet.face);
                self.rag.step(dt, ax, ay, self.settings.pet_scale());
            } else if self.rag.is_planted() {
                self.rag.clear_plant();
                self.rag.relax(dt);
            } else if self.rag.live {
                self.rag.relax(dt);
            }
        }
    }

    pub fn prepare(&mut self) {
        if self.grip.is_none() {
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
                if self.grip.is_some() {
                    return;
                }
                let Some(point) = draw::pick(&self.committed_parts, lx, ly) else {
                    return;
                };
                self.sync_rag();
                self.rag.clear_plant();
                if !self.rag.live {
                    let degrees = self.pack.local_degrees(self.motion_name(), self.pet.phase);
                    self.rag.seed(&degrees);
                }
                self.rag.live = true;
                let cursor =
                    Point::from_xy(self.committed_x as f32 + lx, self.committed_y as f32 + ly);
                self.grip = Some(Grip { point, cursor });
                self.pet.grab();
                self.trail.clear();
                self.trail.push((self.clock, cursor.x, cursor.y));
                self.prev_vx = 0.0;
                self.prev_vy = 0.0;
            }
            Pointer::Motion if self.grip.is_some() => self.move_grip(lx, ly),
            Pointer::LeftUp if self.grip.is_some() => {
                self.move_grip(lx, ly);
                let grip = self.grip.take().expect("active grip");
                let (vx, vy) = self.flick();
                self.rag.release(
                    &self.pack,
                    grip.point,
                    Point::from_xy(vx * self.pet.face, vy),
                    self.settings.scales(),
                );
                self.trail.clear();
                self.pet.release(&self.room, vx, vy);
                self.sync_geometry();
            }
            Pointer::Motion | Pointer::LeftUp => {}
        }
    }

    fn move_grip(&mut self, lx: f32, ly: f32) {
        let grip = self.grip.as_mut().expect("active grip");
        let cursor = Point::from_xy(self.committed_x as f32 + lx, self.committed_y as f32 + ly);
        if grip.cursor == cursor {
            return;
        }
        self.pet.x += cursor.x - grip.cursor.x;
        self.pet.y += cursor.y - grip.cursor.y;
        grip.cursor = cursor;
        self.trail.push((self.clock, cursor.x, cursor.y));
        self.trail.retain(|(time, _, _)| self.clock - *time <= 0.12);
        if let Some(&(time, x, y)) = self.trail.first() {
            let step = (self.clock - time).max(0.001);
            self.pet.vx = (cursor.x - x) / step;
            self.pet.vy = (cursor.y - y) / step;
        }
    }

    fn scene(&self) -> Scene {
        Scene {
            cycle: self.pet.phase,
            blink: (self.clock >= self.blink_at).then_some(self.clock - self.blink_at),
            motion: match self.pet.activity {
                Activity::Walk => Motion::Walk,
                Activity::Fall => Motion::Fall,
                Activity::Drag => Motion::Drag,
                Activity::Down => Motion::Down,
                Activity::Sit => Motion::Sit,
                Activity::Idle => Motion::Idle,
            },
            tilt: if self.rag.live {
                self.rag.turn()
            } else {
                self.pet.tilt()
            },
            facing_left: self.pet.face < 0.0,
            scales: self.settings.scales(),
            pose: if self.rag.live {
                self.rag.degrees()
            } else {
                Vec::new()
            },
            anchor: self.grip.as_ref().map(|grip| grip.point),
        }
    }

    pub fn collision_bounds(&self) -> Option<Rect> {
        if self.grip.is_some() {
            return None;
        }
        let origin = Point::from_xy(self.pet.x - self.feet_x(), self.pet.y - self.feet_y());
        draw::collision_bounds(&self.scene(), &self.pack, origin, self.screen.0)
    }

    pub fn knock_back(&mut self, vx: f32, vy: f32) {
        if !self.rag.live {
            let degrees = self.pack.local_degrees(self.motion_name(), self.pet.phase);
            self.rag.seed(&degrees);
        }
        self.rag
            .impact((vx * self.pet.face / 85.0).clamp(-12.0, 12.0));
        self.pet.knock_back(vx, vy);
    }

    pub fn render(&mut self) -> Frame {
        let mut frame = draw::render(&self.scene(), &self.pack, self.scale);
        self.parts = std::mem::take(&mut frame.parts);
        self.logical_w = frame.width / self.scale.max(1);
        self.logical_h = frame.height / self.scale.max(1);
        let feet_x = self.feet_x() - frame.shift.0 as f32;
        let feet_y = self.feet_y() - frame.shift.1 as f32;
        if let (Some(grip), Some(anchor)) = (&self.grip, frame.anchor) {
            let support = frame.support.unwrap_or(feet_y);
            let floor_origin = (self.room.floor - support).floor() as i32;
            self.origin_x = (grip.cursor.x - anchor.x).round() as i32;
            self.origin_y = ((grip.cursor.y - anchor.y).round() as i32).min(floor_origin);
            self.pet.x = self.origin_x as f32 + feet_x;
            self.pet.y = self.origin_y as f32 + support;
        } else {
            let rightmost_origin = (self.screen.0 as i32 - self.logical_w as i32).max(0);
            self.origin_x = ((self.pet.x - feet_x).round() as i32).clamp(0, rightmost_origin);
            self.origin_y = (self.pet.y - feet_y).round() as i32;
        }
        frame
    }

    pub fn note_committed(&mut self) {
        self.committed_x = self.origin_x;
        self.committed_y = self.origin_y;
        self.committed_parts = std::mem::take(&mut self.parts);
    }

    pub fn pack_id(&self) -> &str {
        &self.pack.id
    }

    pub fn apply_settings(&mut self, settings: Settings) {
        self.settings = settings;
        self.fit_room();
        self.sync_geometry();
    }

    pub fn hit_body(&self, screen_x: f32, screen_y: f32) -> bool {
        let (x, y) = self.pointer_from_screen(screen_x, screen_y);
        draw::pick(&self.committed_parts, x, y).is_some()
    }

    fn feet_x(&self) -> f32 {
        FEET_X * self.settings.scales().0
    }
    fn feet_y(&self) -> f32 {
        draw::scene_size(self.settings.scales()).1 as f32
    }

    fn motion_name(&self) -> &'static str {
        match self.pet.activity {
            Activity::Walk => "walk",
            Activity::Fall => "fall",
            Activity::Drag => "drag",
            Activity::Down => "idle",
            Activity::Sit => "sit",
            Activity::Idle => "idle",
        }
    }

    fn flick(&self) -> (f32, f32) {
        let Some(&(t1, x1, y1)) = self.trail.last() else {
            return (0.0, 0.0);
        };
        // The hand already stopped. Holding her and letting go is not a throw.
        if self.clock - t1 > 0.08 {
            return (0.0, 0.0);
        }
        let (t0, x0, y0) = self
            .trail
            .iter()
            .rev()
            .find(|(time, _, _)| t1 - *time >= 0.045)
            .copied()
            .unwrap_or(self.trail[0]);
        let dt = (t1 - t0).max(0.016);
        let vx = (x1 - x0) / dt;
        let vy = (y1 - y0) / dt;
        if vx.hypot(vy) < 240.0 {
            return (0.0, 0.0);
        }
        (
            (vx * 2.0).clamp(-4800.0, 4800.0),
            (vy * 2.0).clamp(-4800.0, 4800.0),
        )
    }

    fn sync_rag(&mut self) {
        if self.rag.id != self.pack.id {
            self.rag = Ragdoll::new(&self.pack);
        }
    }

    fn fit_room(&mut self) {
        let (width, height) = self.screen;
        let inset = self.feet_x().min((width - 34.0).max(0.0) / 2.0);
        self.room = Room {
            left: 16.0 + inset,
            right: (width - 16.0 - inset).max(16.0 + inset + 1.0),
            floor: (height - self.bottom).max(1.0),
        };
    }

    fn sync_geometry(&mut self) {
        let (width, height) = draw::scene_size(self.settings.scales());
        self.logical_w = width;
        self.logical_h = height;
        self.origin_x = (self.pet.x - self.feet_x()).round() as i32;
        self.origin_y = (self.pet.y - self.feet_y()).round() as i32;
    }
}

fn next_blink(now: f32) -> f32 {
    now + 2.4 + 2.4 * fastrand::f32()
}

pub enum Pointer {
    Motion,
    LeftDown,
    LeftUp,
}

pub fn startup_room(width: f32, height: f32) -> Room {
    let margin = 16.0;
    Room {
        left: margin + FEET_X,
        right: (width - margin - FEET_X).max(margin + FEET_X + 1.0),
        floor: height.max(FEET_Y),
    }
}

fn screen_of(room: &Room) -> (f32, f32) {
    (room.right + 16.0 + FEET_X, room.floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app(room: Room) -> App {
        App::new(room, Settings::default(), Pack::builtin())
    }

    fn visible_part(app: &mut App, name: &str, bottom: bool) -> (f32, f32) {
        app.render();
        app.note_committed();
        for row in 0..app.logical_h {
            let y = if bottom { app.logical_h - 1 - row } else { row };
            for x in 0..app.logical_w {
                let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
                if draw::pick(&app.committed_parts, x, y)
                    .is_some_and(|hit| app.pack.bones[hit.bone].name == name)
                {
                    return (x, y);
                }
            }
        }
        panic!("no visible part named {name}");
    }

    #[test]
    fn drag_follows_the_latest_point_until_the_surface_commits() {
        let mut app = test_app(startup_room(800.0, 600.0));
        let (x, y) = visible_part(&mut app, "torso", false);
        let origin_x = app.origin_x;
        let origin_y = app.origin_y;
        app.pointer(x, y, Pointer::LeftDown);
        app.pointer(x + 10.0, y - 10.0, Pointer::Motion);
        app.pointer(x + 30.0, y - 20.0, Pointer::Motion);
        let anchor = app.render().anchor.expect("drag anchor");
        assert!((app.origin_x as f32 + anchor.x - (origin_x as f32 + x + 30.0)).abs() <= 0.51);
        assert!((app.origin_y as f32 + anchor.y - (origin_y as f32 + y - 20.0)).abs() <= 0.51);
        assert_eq!(app.committed_x, origin_x);
    }

    #[test]
    fn screen_pointer_uses_the_committed_window() {
        let app = test_app(startup_room(800.0, 600.0));
        let (x, y) =
            app.pointer_from_screen(app.origin_x as f32 + 12.0, app.origin_y as f32 + 40.0);
        assert_eq!((x, y), (12.0, 40.0));
    }

    #[test]
    fn a_small_size_shrinks_the_frame() {
        let mut app = test_app(startup_room(800.0, 600.0));
        app.settings.scale = "0.2".to_string();
        app.fit_room();
        app.pet.y = app.room.floor;
        app.sync_geometry();
        assert_eq!(app.logical_w, (SPRITE_W * 0.2).round() as u32);
        assert_eq!(app.logical_h, (SPRITE_H * 0.2).round() as u32);
    }

    #[test]
    fn a_flick_throws_along_the_drag() {
        let mut app = test_app(startup_room(1200.0, 800.0));
        let (x, y) = visible_part(&mut app, "torso", false);
        let start_x = app.pet.x;
        app.pointer(x, y, Pointer::LeftDown);
        for step in 1..=5 {
            app.clock = step as f32 * 0.02;
            app.pointer(
                x + step as f32 * 18.0,
                y - step as f32 * 14.0,
                Pointer::Motion,
            );
        }
        app.pointer(x + 90.0, y - 70.0, Pointer::LeftUp);
        assert_eq!(app.pet.activity, Activity::Fall);
        assert!(app.pet.vx > 200.0, "{}", app.pet.vx);
        let mut far = app.pet.x;
        let mut high = app.pet.y;
        for _ in 0..40 {
            app.tick(1.0 / 60.0);
            far = far.max(app.pet.x);
            high = high.min(app.pet.y);
        }
        assert!(far > start_x + 40.0, "{far}");
        assert!(high < app.room.floor - 5.0, "{high}");
        assert!(app.rag.live);
    }

    #[test]
    fn letting_go_after_a_pause_does_not_throw() {
        let mut app = test_app(startup_room(1200.0, 800.0));
        let (x, y) = visible_part(&mut app, "torso", false);
        app.pointer(x, y, Pointer::LeftDown);
        app.clock = 0.02;
        app.pointer(x + 60.0, y - 40.0, Pointer::Motion);
        app.clock = 0.05;
        app.pointer(x + 120.0, y - 60.0, Pointer::Motion);
        app.clock = 0.40;
        app.pointer(x + 120.0, y - 60.0, Pointer::LeftUp);
        assert_eq!(app.pet.activity, Activity::Fall);
        assert!(app.pet.vx.abs() < 1.0, "{}", app.pet.vx);
        assert!(app.pet.vy.abs() < 1.0, "{}", app.pet.vy);
    }

    #[test]
    fn a_downward_mouse_flick_bounces_with_the_frame_inside_the_screen_edge() {
        let mut app = test_app(startup_room(1200.0, 800.0));
        app.set_bottom(48.0);
        app.set_room(startup_room(1200.0, 800.0));
        app.pet.x = app.room.right;
        let (x, y) = visible_part(&mut app, "torso", false);
        let (cursor_x, cursor_y) = (app.origin_x as f32 + x, app.origin_y as f32 + y);
        app.pointer(x, y, Pointer::LeftDown);
        app.clock = 0.10;
        app.pointer(x, y - 130.0, Pointer::Motion);
        app.clock = 0.125;
        app.pointer(x, y - 82.0, Pointer::Motion);
        app.clock = 0.15;
        app.pointer(x, y + 26.0, Pointer::Motion);
        let held = app.render();
        let lowest = app.origin_y as f32 + held.support.expect("held pose contact");
        assert!((0.0..1.01).contains(&(app.room.floor - lowest)));
        app.note_committed();
        let (x, y) = app.pointer_from_screen(cursor_x, cursor_y + 26.0);
        app.pointer(x, y, Pointer::LeftUp);
        assert_eq!(app.pet.activity, Activity::Fall);
        assert!(app.pet.vy > 0.0);

        let mut bounced = false;
        let mut rested = false;
        for _ in 0..360 {
            app.tick(1.0 / 60.0);
            let frame = app.render();
            assert!(app.origin_x >= 0 && app.origin_x + frame.width as i32 <= 1200);
            assert!(app.origin_y + frame.height as i32 <= app.room.floor as i32);
            rested |= app.pet.activity == Activity::Down;
            bounced |= app.pet.vy < 0.0 && app.pet.y < app.room.floor - 10.0;
            if app.pet.activity == Activity::Idle {
                break;
            }
        }
        assert!(bounced);
        assert!(rested);
        assert_eq!(app.pet.activity, Activity::Idle);
    }

    #[test]
    fn the_floor_is_the_top_edge_of_the_bottom_panel() {
        let mut app = test_app(startup_room(800.0, 600.0));
        assert_eq!(app.room.floor, 600.0);
        app.set_bottom(48.0);
        for scale in ["0.2", "0.4", "0.6", "0.8", "1"] {
            app.settings.scale = scale.to_string();
            app.set_room(startup_room(800.0, 600.0));
            assert_eq!(app.room.floor, 552.0);
            assert_eq!(app.origin_y + app.logical_h as i32, 552);
        }
    }
    #[test]
    fn a_visible_limb_stays_at_the_cursor_and_the_body_hangs_below_it() {
        for (part, bottom) in [
            ("head", false),
            ("left-hand", true),
            ("right-hand", true),
            ("left-leg", true),
            ("right-leg", true),
        ] {
            for face in [-1.0, 1.0] {
                let mut app = test_app(startup_room(1200.0, 800.0));
                app.pet.face = face;
                let (x, y) = visible_part(&mut app, part, bottom);
                app.pointer(x, y, Pointer::LeftDown);
                let (x, y) = app.pointer_from_screen(550.0, 215.0);
                app.pointer(x, y, Pointer::Motion);
                for tick in 0..150 {
                    app.tick(1.0 / 60.0);
                    if tick % 20 == 0 {
                        app.render();
                        app.note_committed();
                    }
                }
                let anchor = app.render().anchor.expect("drag anchor");
                assert!((app.origin_x as f32 + anchor.x - 550.0).abs() <= 0.51);
                assert!((app.origin_y as f32 + anchor.y - 215.0).abs() <= 0.51);
                let grip = app.grip.as_ref().unwrap();
                let pose = app.pack.pose_with("idle", 0.0, Some(&app.rag.degrees()));
                let pin = grip.point.in_pose(&pose);
                let (sin, cos) = app.rag.turn().sin_cos();
                let below = |name: &str| {
                    let index = app
                        .pack
                        .bones
                        .iter()
                        .position(|bone| bone.name == name)
                        .unwrap();
                    let joint = pose.placed[index];
                    (joint.x - pin.x) * sin + (joint.y - pin.y) * cos
                };
                assert!(below("torso") > 75.0, "{part}: {}", below("torso"));
                if part.ends_with("leg") {
                    assert!(
                        below("head") > below("torso"),
                        "head must hang below the torso"
                    );
                } else if part.ends_with("hand") {
                    assert!(
                        below(part) > 110.0,
                        "shoulder must hang below the grabbed hand"
                    );
                }
            }
        }
    }
}
