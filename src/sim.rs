// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Screen-edge movement. Coordinates grow right and down. `y` is the floor contact point.

pub const SPRITE_W: f32 = 288.0;
pub const SPRITE_H: f32 = 270.0;
pub const FEET_X: f32 = SPRITE_W * 0.5;
pub const FEET_Y: f32 = SPRITE_H;
const IDLE_FOR: f32 = 1.6;
const FALLBACK_WALK_CYCLE: f32 = 0.7;
const SIT_FOR: f32 = 7.0;
const DOWN_FOR: f32 = 2.6;
pub const RISE_FOR: f32 = 1.5;
const WALK_SPEED: f32 = 78.0;
const BOUNCE_RESTITUTION: f32 = 0.62;
const MIN_REBOUND_SPEED: f32 = 120.0;
pub const GRAVITY: f32 = 2400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Walk,
    Fall,
    Drag,
    Down,
    Rise,
    Sit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Landing {
    Drop,
    Throw,
    Bounce,
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
    landing: Landing,
    walk_cycle: f32,
    walk_for: f32,
}

impl Pet {
    pub fn on_floor(room: &Room, walk_cycle: Option<f32>) -> Self {
        let walk_cycle = walk_cycle.unwrap_or(FALLBACK_WALK_CYCLE);
        Self {
            x: ((room.left + room.right) * 0.5).clamp(room.left, room.right),
            y: room.floor,
            vx: 0.0,
            vy: 0.0,
            face: 1.0,
            activity: Activity::Idle,
            time: 0.0,
            phase: 0.0,
            landing: Landing::Drop,
            walk_cycle,
            walk_for: 4.0 * walk_cycle,
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
            Activity::Rise => {
                if self.time >= RISE_FOR {
                    self.start(Activity::Idle);
                }
            }
            Activity::Sit => self.sit(room),
            Activity::Walk => self.walk(room, dt),
            Activity::Idle => {
                self.vx = 0.0;
                self.vy = 0.0;
                self.y = room.floor;
                self.x = self.x.clamp(room.left, room.right);
                if self.time >= IDLE_FOR {
                    self.choose_activity(fastrand::u8(0..100), fastrand::u8(3..=5));
                }
            }
        }
    }

    fn choose_activity(&mut self, roll: u8, walk_cycles: u8) {
        match roll {
            0..50 => {
                self.walk_for = f32::from(walk_cycles) * self.walk_cycle;
                self.start(Activity::Walk);
            }
            50..80 => self.start(Activity::Sit),
            _ => self.start(Activity::Idle),
        }
    }

    pub fn can_strike(&self) -> bool {
        (matches!(self.activity, Activity::Fall) && self.landing != Landing::Drop
            || self.activity == Activity::Down)
            && self.vx.hypot(self.vy) >= 140.0
    }

    pub fn knock_back(&mut self, vx: f32, vy: f32) {
        self.vx = vx;
        self.vy = vy;
        self.landing = Landing::Throw;
        self.start(Activity::Fall);
    }

    pub fn grab(&mut self) {
        self.activity = Activity::Drag;
        self.vx = 0.0;
        self.vy = 0.0;
        self.landing = Landing::Drop;
        self.time = 0.0;
    }

    /// Release a drag. A flick flies. A still hand just drops her.
    pub fn release(&mut self, room: &Room, vx: f32, vy: f32) {
        self.vx = vx;
        self.vy = vy;
        self.landing = if vy >= 80.0 && vy >= vx.abs() {
            Landing::Bounce
        } else if vx.abs() >= 80.0 || vy <= -80.0 {
            Landing::Throw
        } else {
            Landing::Drop
        };
        self.y = self.y.min(room.floor);
        if self.landing != Landing::Drop || self.y < room.floor - 2.0 {
            self.start(Activity::Fall);
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
        self.phase = 0.0;
    }

    fn fall(&mut self, room: &Room, dt: f32) {
        let start_y = self.y;
        let start_vy = self.vy;
        self.vy += GRAVITY * dt;
        self.x += self.vx * dt;
        self.y += (start_vy + self.vy) * 0.5 * dt;
        if self.x < room.left {
            self.x = room.left;
            self.vx = self.vx.abs();
        } else if self.x > room.right {
            self.x = room.right;
            self.vx = -self.vx.abs();
        }
        if self.y >= room.floor {
            self.y = room.floor;
            if self.landing == Landing::Bounce {
                let distance = (room.floor - start_y).max(0.0);
                let impact_speed = (start_vy * start_vy + 2.0 * GRAVITY * distance).sqrt();
                let height_limit = room.floor * 0.45;
                let rebound =
                    (impact_speed * BOUNCE_RESTITUTION).min((2.0 * GRAVITY * height_limit).sqrt());
                if rebound >= MIN_REBOUND_SPEED {
                    // Use the remainder of this tick for upward motion after the impact.
                    let impact_time = (impact_speed - start_vy) / GRAVITY;
                    let remaining = (dt - impact_time).clamp(0.0, dt);
                    self.vy = -rebound + GRAVITY * remaining;
                    self.y -= remaining * (rebound - 0.5 * GRAVITY * remaining);
                    self.vx *= 0.8;
                    return;
                }
            }
            if self.landing != Landing::Drop {
                if self.vy > 80.0 {
                    self.vx *= 0.55;
                }
                self.vy = 0.0;
                self.landing = Landing::Drop;
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
        if self.time >= DOWN_FOR && self.vx == 0.0 {
            self.start(Activity::Rise);
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
        let travel_time = (self.walk_for - (self.time - dt)).clamp(0.0, dt);
        self.x += self.vx * travel_time;
        self.y = room.floor;
        if self.x <= room.left {
            self.x = room.left;
            self.face = 1.0;
        } else if self.x >= room.right {
            self.x = room.right;
            self.face = -1.0;
        }
        if self.time >= self.walk_for {
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
        let mut pet = Pet::on_floor(&room, None);
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
        let mut pet = Pet::on_floor(&room, None);
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
        let mut pet = Pet::on_floor(&room, None);
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
    fn a_throw_lands_in_a_down_pose() {
        let room = room();
        let mut pet = Pet::on_floor(&room, None);
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
    fn a_throw_skids_a_short_way() {
        let room = room();
        let mut pet = Pet::on_floor(&room, None);
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
        let mut pet = Pet::on_floor(&room, None);
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
    fn a_downward_throw_rebounds_with_smaller_hops_then_settles() {
        let room = room();
        for fps in [30.0, 60.0, 144.0] {
            let mut pet = Pet::on_floor(&room, None);
            pet.y += 10.0;
            pet.release(&room, 0.0, 1100.0);
            assert_eq!(pet.activity, Activity::Fall);
            let mut heights = Vec::new();
            for _ in 0..(fps * 4.0 + fps * RISE_FOR) as usize {
                let was_rising = pet.vy < 0.0;
                pet.step(&room, 1.0 / fps);
                assert!(pet.y <= room.floor);
                if was_rising && pet.vy >= 0.0 {
                    heights.push(room.floor - pet.y);
                }
                if pet.activity == Activity::Idle {
                    break;
                }
            }
            assert!(heights.len() >= 3, "{fps}: {heights:?}");
            assert!(heights[0] > 90.0, "{fps}: {heights:?}");
            assert!(heights.windows(2).all(|pair| pair[0] > pair[1]));
            assert_eq!(pet.activity, Activity::Idle);
        }
    }

    #[test]
    fn idle_rolls_choose_actions_and_walks_use_the_packs_cycle_length() {
        let room = Room {
            left: 0.0,
            right: 1600.0,
            floor: 800.0,
        };
        for count in 3..=5 {
            let mut pet = Pet::on_floor(&room, Some(0.8));
            let start_x = pet.x;
            pet.choose_activity(49, count);
            assert_eq!(pet.activity, Activity::Walk);
            while pet.activity == Activity::Walk {
                pet.step(&room, 0.04);
            }
            assert!((pet.x - start_x - WALK_SPEED * 0.8 * f32::from(count)).abs() < 0.1);
        }
        let mut pet = Pet::on_floor(&room, None);
        pet.choose_activity(50, 4);
        assert_eq!(pet.activity, Activity::Sit);
        pet.choose_activity(80, 4);
        assert_eq!(pet.activity, Activity::Idle);
    }

    #[test]
    fn a_sideways_flick_on_the_floor_keeps_going() {
        let room = room();
        let mut pet = Pet::on_floor(&room, None);
        let start = pet.x;
        pet.release(&room, 700.0, 0.0);
        for _ in 0..20 {
            pet.step(&room, 1.0 / 60.0);
        }
        assert!(pet.x > start + 80.0, "{}", pet.x);
        assert_eq!(pet.activity, Activity::Down);
    }

    impl Pet {
        fn become_for_test(&mut self, activity: Activity) {
            self.start(activity);
        }
    }
}
