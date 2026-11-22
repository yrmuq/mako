// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Screen-edge movement. Coordinates grow right and down. `y` is the feet.

pub const SPRITE_W: f32 = 200.0;
pub const SPRITE_H: f32 = 270.0;
pub const FEET_X: f32 = 100.0;
pub const FEET_Y: f32 = 258.0;
const IDLE_FOR: f32 = 1.6;
const WALK_FOR: f32 = 2.8;
const WALK_SPEED: f32 = 78.0;
const GRAVITY: f32 = 2400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Walk,
    Fall,
    Drag,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub left: f32,
    pub right: f32,
    pub floor: f32,
}

#[derive(Clone, Debug)]
pub struct Pet {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub face: f32,
    pub activity: Activity,
    pub time: f32,
    pub phase: f32,
}

impl Pet {
    pub fn on_floor(room: &Room) -> Self {
        Self {
            x: ((room.left + room.right) * 0.5).clamp(room.left, room.right),
            y: room.floor,
            vx: 0.0,
            vy: 0.0,
            face: 1.0,
            activity: Activity::Idle,
            time: 0.0,
            phase: 0.0,
        }
    }

    pub fn step(&mut self, room: &Room, dt: f32) {
        let dt = dt.clamp(0.0, 0.05);
        self.phase += dt;
        if self.activity != Activity::Drag {
            self.time += dt;
        }
        match self.activity {
            Activity::Drag => {}
            Activity::Fall => self.fall(room, dt),
            Activity::Walk => self.walk(room, dt),
            Activity::Idle => {
                self.vx = 0.0;
                self.vy = 0.0;
                self.y = room.floor;
                self.x = self.x.clamp(room.left, room.right);
                if self.time >= IDLE_FOR {
                    self.start(Activity::Walk);
                }
            }
        }
    }

    pub fn grab(&mut self) {
        self.activity = Activity::Drag;
        self.vx = 0.0;
        self.vy = 0.0;
        self.time = 0.0;
    }

    /// Release a drag. A downward or sideways throw falls; a rest lands or idles.
    pub fn release(&mut self, room: &Room, vx: f32, vy: f32) {
        self.vx = vx;
        self.vy = vy;
        let moving = vx.abs() > 40.0 || vy.abs() > 40.0 || self.y < room.floor - 2.0;
        if moving {
            self.start(Activity::Fall);
            if self.vx.abs() > 1.0 {
                self.face = if self.vx >= 0.0 { 1.0 } else { -1.0 };
            }
        } else {
            self.y = room.floor;
            self.x = self.x.clamp(room.left, room.right);
            self.start(Activity::Idle);
        }
    }

    pub fn tilt(&self) -> f32 {
        match self.activity {
            Activity::Fall => (self.vx / 800.0).clamp(-0.35, 0.35),
            Activity::Drag => (self.vx / 1200.0).clamp(-0.25, 0.25),
            _ => 0.0,
        }
    }

    fn start(&mut self, activity: Activity) {
        self.activity = activity;
        self.time = 0.0;
    }

    fn fall(&mut self, room: &Room, dt: f32) {
        self.vy += GRAVITY * dt;
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        if self.x < room.left {
            self.x = room.left;
            self.vx = self.vx.abs();
            self.face = 1.0;
        } else if self.x > room.right {
            self.x = room.right;
            self.vx = -self.vx.abs();
            self.face = -1.0;
        }
        if self.y >= room.floor {
            self.y = room.floor;
            self.vx = 0.0;
            self.vy = 0.0;
            self.start(Activity::Idle);
        }
    }

    fn walk(&mut self, room: &Room, dt: f32) {
        self.vx = self.face.signum() * WALK_SPEED;
        self.x += self.vx * dt;
        self.y = room.floor;
        if self.x <= room.left {
            self.x = room.left;
            self.face = 1.0;
        } else if self.x >= room.right {
            self.x = room.right;
            self.face = -1.0;
        }
        if self.time >= WALK_FOR {
            self.start(Activity::Idle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room() -> Room {
        Room {
            left: 100.0,
            right: 500.0,
            floor: 400.0,
        }
    }

    #[test]
    fn fall_lands_on_the_floor() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        pet.y = 100.0;
        pet.become_for_test(Activity::Fall);
        for _ in 0..180 {
            pet.step(&room, 1.0 / 60.0);
        }
        assert_ne!(pet.activity, Activity::Fall);
        assert!((pet.y - room.floor).abs() < 0.1);
    }

    #[test]
    fn walk_turns_around_at_the_right_edge() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        pet.x = room.right - 1.0;
        pet.face = 1.0;
        pet.become_for_test(Activity::Walk);
        pet.step(&room, 1.0 / 30.0);
        assert_eq!(pet.face, -1.0);
        assert!((pet.x - room.right).abs() < 0.1);
    }

    impl Pet {
        fn become_for_test(&mut self, activity: Activity) {
            self.start(activity);
        }
    }
}
