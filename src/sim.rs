// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Screen-edge movement. Coordinates grow right and down. `y` is the feet.

pub const SPRITE_W: f32 = 288.0;
pub const SPRITE_H: f32 = 270.0;
pub const FEET_X: f32 = SPRITE_W * 0.5;
pub const FEET_Y: f32 = SPRITE_H;
const IDLE_FOR: f32 = 1.6;
const WALK_FOR: f32 = 2.8;
const SIT_FOR: f32 = 7.0;
const DOWN_FOR: f32 = 2.6;
const WALK_SPEED: f32 = 78.0;
pub const GRAVITY: f32 = 2400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Walk,
    Fall,
    Drag,
    Down,
    Sit,
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
    pub airborne: bool,
    pub tossed: bool,
    rests: u32,
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
            airborne: false,
            tossed: false,
            rests: 0,
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
            Activity::Down => self.down(room, dt),
            Activity::Sit => self.sit(room),
            Activity::Walk => self.walk(room, dt),
            Activity::Idle => {
                self.vx = 0.0;
                self.vy = 0.0;
                self.y = room.floor;
                self.x = self.x.clamp(room.left, room.right);
                if self.time >= IDLE_FOR {
                    self.rests += 1;
                    if self.rests.is_multiple_of(2) {
                        self.start(Activity::Sit);
                    } else {
                        self.start(Activity::Walk);
                    }
                }
            }
        }
    }

    pub fn grab(&mut self) {
        self.activity = Activity::Drag;
        self.vx = 0.0;
        self.vy = 0.0;
        self.tossed = false;
        self.time = 0.0;
    }

    /// Release a drag. A flick flies. A still hand just drops her.
    pub fn release(&mut self, room: &Room, vx: f32, vy: f32) {
        self.vx = vx;
        self.vy = vy;
        self.tossed = vx.abs() >= 80.0 || vy <= -80.0;
        let moving = self.tossed || self.y < room.floor - 2.0;
        if moving {
            self.airborne = self.y < room.floor - 14.0;
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
        if self.y < room.floor - 14.0 {
            self.airborne = true;
        }
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
            if self.airborne && self.tossed && self.vy > 80.0 {
                // A real throw skids a short way. A plain drop does not.
                self.vx *= 0.55;
                self.vy = 0.0;
                self.airborne = false;
                self.tossed = false;
                self.start(Activity::Down);
            } else if self.vy > 500.0 {
                self.vy = -self.vy * 0.28;
                self.vx *= 0.6;
            } else if self.vx.abs() > 36.0 && self.vy < 160.0 {
                // A sideways release along the floor slides instead of stopping dead.
                self.vy = 0.0;
                let friction = 900.0 * dt;
                if self.vx.abs() <= friction {
                    self.vx = 0.0;
                    self.start(Activity::Idle);
                } else {
                    self.vx -= self.vx.signum() * friction;
                }
            } else {
                self.vx = 0.0;
                self.vy = 0.0;
                self.start(Activity::Idle);
            }
        }
    }

    fn down(&mut self, room: &Room, dt: f32) {
        self.y = room.floor;
        self.vy = 0.0;
        let drag = (800.0 + 1.6 * self.vx.abs()) * dt;
        if self.vx.abs() <= drag {
            self.vx = 0.0;
        } else {
            self.vx -= self.vx.signum() * drag;
        }
        self.x += self.vx * dt;
        if self.x < room.left {
            self.x = room.left;
            self.vx = 0.0;
        } else if self.x > room.right {
            self.x = room.right;
            self.vx = 0.0;
        }
        // The lie only finishes after the skid. Standing up mid-slide looks like a pop.
        if self.vx.abs() > 36.0 {
            self.time = self.time.min(0.4);
        }
        if self.time >= DOWN_FOR {
            self.start(Activity::Idle);
        }
    }

    fn sit(&mut self, room: &Room) {
        self.vx = 0.0;
        self.vy = 0.0;
        self.y = room.floor;
        self.x = self.x.clamp(room.left, room.right);
        if self.time >= SIT_FOR {
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

    #[test]
    fn a_sideways_throw_arcs_up_then_lands() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        let start_x = pet.x;
        pet.y = room.floor - 80.0;
        pet.release(&room, 420.0, -900.0);
        let mut peak = pet.y;
        let mut far = pet.x;
        let mut landed = false;
        for _ in 0..300 {
            pet.step(&room, 1.0 / 60.0);
            peak = peak.min(pet.y);
            far = far.max(pet.x);
            if pet.activity != Activity::Fall && (pet.y - room.floor).abs() < 0.1 {
                landed = true;
            }
        }
        assert!(peak < room.floor - 140.0, "{peak}");
        assert!(far > start_x + 80.0, "{far}");
        assert!(landed);
    }

    #[test]
    fn a_flight_lands_face_down_instead_of_standing() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        pet.y = room.floor - 220.0;
        pet.release(&room, 80.0, 40.0);
        let mut saw_down = false;
        for _ in 0..180 {
            pet.step(&room, 1.0 / 60.0);
            if pet.activity == Activity::Down {
                saw_down = true;
                assert!((pet.y - room.floor).abs() < 0.1);
                break;
            }
        }
        assert!(saw_down);
    }

    #[test]
    fn a_faceplant_skids_a_short_way() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        pet.x = room.left + 30.0;
        pet.y = room.floor - 120.0;
        pet.release(&room, 700.0, 30.0);
        let mut began = None;
        let mut far = pet.x;
        for _ in 0..300 {
            pet.step(&room, 1.0 / 60.0);
            if pet.activity == Activity::Down {
                if began.is_none() {
                    began = Some(pet.x);
                }
                far = far.max(pet.x);
            }
        }
        let began = began.expect("down");
        let slid = far - began;
        assert!(slid > 25.0, "{slid}");
        assert!(slid < 180.0, "{slid}");
    }

    #[test]
    fn a_still_drop_lands_on_her_feet() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        pet.y = room.floor - 160.0;
        pet.release(&room, 0.0, 0.0);
        let mut saw_down = false;
        for _ in 0..240 {
            pet.step(&room, 1.0 / 60.0);
            if pet.activity == Activity::Down {
                saw_down = true;
            }
        }
        assert!(!saw_down);
        assert!((pet.y - room.floor).abs() < 0.1);
        assert_ne!(pet.activity, Activity::Fall);
    }

    #[test]
    fn the_second_rest_is_spent_sitting() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        pet.become_for_test(Activity::Idle);
        pet.time = IDLE_FOR;
        pet.step(&room, 1.0 / 60.0);
        assert_eq!(pet.activity, Activity::Walk);
        pet.become_for_test(Activity::Idle);
        pet.time = IDLE_FOR;
        pet.step(&room, 1.0 / 60.0);
        assert_eq!(pet.activity, Activity::Sit);
    }

    #[test]
    fn a_sideways_flick_on_the_floor_keeps_going() {
        let room = room();
        let mut pet = Pet::on_floor(&room);
        let start = pet.x;
        pet.release(&room, 700.0, 0.0);
        for _ in 0..20 {
            pet.step(&room, 1.0 / 60.0);
        }
        assert!(pet.x > start + 80.0, "{}", pet.x);
        assert_eq!(pet.activity, Activity::Fall);
    }

    impl Pet {
        fn become_for_test(&mut self, activity: Activity) {
            self.start(activity);
        }
    }
}
