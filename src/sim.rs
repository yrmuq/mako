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
pub const REFERENCE_RISE: f32 = 1.5;
const WALK_SPEED: f32 = 78.0;
const CLIMB_SPEED: f32 = 115.0;
pub const WALL_STROKE: f32 = 0.48;
const WALL_SPEED: f32 = 72.0;
const SLIDE_SPEED: f32 = 235.0;
const PARACHUTE_SPEED: f32 = 125.0;
const HAND_SPEED: f32 = 375.0;
pub const HAND_REACH: f32 = 0.4;
pub const HAND_RELEASE: f32 = 0.3;
const BOUNCE_RESTITUTION: f32 = 0.62;
const MIN_REBOUND_SPEED: f32 = 120.0;
const SOFT_RESTITUTION: f32 = 0.42;
const MIN_SOFT_REBOUND: f32 = 90.0;
const MAX_SOFT_REBOUND: f32 = 270.0;
pub const GRAVITY: f32 = 2400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    Plant,
    Water,
    Decorate,
    TinselTree,
    StarTree,
    UntrimTree,
    OpenGift,
    EatCandy,
    ThrowCoal,
    CarryTree,
    Parry,
    RunCover,
    LeaveCover,
    Hide,
    Cannon,
    FetchSled,
    PullSled,
    RideSled,
    WallRecover,
    RampClimb,
    RampScramble,
    RampWave,
    RampSlide,
    RampTumble,
    RampLand,
    Captive,
    FoldPhone,
    LionCrouch,
    LionRun,
    TurtleBounce,
    LionBite,
    LionHeld,
    Scuffle,
    BobComputer,
    BobCloak,
    BobCall,
    BobWait,
    BobLaser,
}

impl Special {
    pub fn animation(self) -> &'static str {
        match self {
            Self::Plant => "open-gift",
            Self::Water => "water-tree",
            Self::Decorate | Self::TinselTree | Self::StarTree | Self::UntrimTree => {
                "decorate-tree"
            }
            Self::OpenGift => "open-gift",
            Self::EatCandy => "eat-candy",
            Self::ThrowCoal => "throw-snow",
            Self::CarryTree => "carry-tree",
            Self::Parry => "parry-snow",
            Self::RunCover | Self::LeaveCover => "walk",
            Self::Hide => "hide-snow",
            Self::Cannon => "snow-cannon",
            Self::FetchSled => "walk",
            Self::PullSled => "pull-sled",
            Self::RideSled => "ride-sled",
            Self::WallRecover | Self::RampLand => "wall-recover",
            Self::RampClimb | Self::RampScramble => "climb",
            Self::RampWave => "ramp-wave",
            Self::RampSlide => "ride-sled",
            Self::RampTumble => "idle",
            Self::Captive => "drag",
            Self::FoldPhone => "fold-phone",
            Self::LionCrouch => "lion-crouch",
            Self::LionRun => "lion-run",
            Self::TurtleBounce => "turtle-bounce",
            Self::LionBite => "lion-bite",
            Self::LionHeld => "drag",
            Self::Scuffle => "scuffle",
            Self::BobComputer | Self::BobCloak | Self::BobCall => "bob-computer",
            Self::BobWait => "idle",
            Self::BobLaser => "bob-laser",
        }
    }

    pub fn crosses_screen_edge(self) -> bool {
        matches!(self, Self::FetchSled | Self::PullSled | Self::Captive)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    Idle,
    Walk,
    Fall,
    Drag,
    Down,
    Rise,
    Sit,
    ClimbWall,
    SlideWall,
    ClimbCeiling,
    Parachute,
    Hand,
    HandRelease,
    Approach,
    Talk,
    Listen,
    PackSnow,
    ThrowSnow,
    Sculpt,
    Special(Special),
}

impl Activity {
    pub fn routine(self) -> bool {
        matches!(self, Self::Idle | Self::Walk | Self::Sit)
    }
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
    pub ceiling: f32,
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
    pub rise_for: f32,
    landing: Landing,
    soft_bounced: bool,
    walk_cycle: f32,
    walk_for: f32,
    down_for: f32,
    sled_edge: Option<bool>,
    relocation: Option<(f32, f32, f32)>,
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
            rise_for: REFERENCE_RISE,
            landing: Landing::Drop,
            soft_bounced: false,
            walk_cycle,
            walk_for: 4.0 * walk_cycle,
            down_for: 2.6,
            sled_edge: None,
            relocation: None,
        }
    }

    pub fn step(&mut self, room: &Room, dt: f32, slippery: bool) {
        self.sled_edge = None;
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
                if self.time >= self.rise_for {
                    self.start(Activity::Idle);
                }
            }
            Activity::Sit => self.sit(room),
            Activity::Walk => self.walk(room, dt),
            Activity::ClimbWall => self.climb_wall(room, dt, slippery),
            Activity::SlideWall => self.slide_wall(room, dt),
            Activity::ClimbCeiling => self.climb_ceiling(room, dt),
            Activity::Parachute => self.parachute(room, dt),
            Activity::Hand => self.hand(room, dt),
            Activity::HandRelease => {
                self.y = room.floor;
                if self.time >= HAND_RELEASE {
                    self.start(Activity::Idle);
                }
            }
            Activity::Special(action @ (Special::WallRecover | Special::RampLand)) => {
                self.vx = 0.0;
                self.vy = 0.0;
                self.y = room.floor;
                let end = if action == Special::RampLand {
                    0.48
                } else {
                    1.18
                };
                if self.time >= end {
                    if action == Special::WallRecover {
                        self.face = -self.face;
                    }
                    self.walk_for = 3.0 * self.walk_cycle;
                    self.start(Activity::Walk);
                }
            }
            Activity::Special(
                Special::Captive
                | Special::LionHeld
                | Special::RampClimb
                | Special::RampScramble
                | Special::RampWave
                | Special::RampSlide
                | Special::RampTumble,
            ) => {}
            Activity::Approach
            | Activity::Talk
            | Activity::Listen
            | Activity::PackSnow
            | Activity::ThrowSnow
            | Activity::Sculpt
            | Activity::Special(_) => {
                self.y = room.floor;
                self.vx = 0.0;
                self.vy = 0.0;
            }
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
            Activity::ClimbWall => -0.19,
            Activity::SlideWall => -0.32,
            Activity::ClimbCeiling => std::f32::consts::PI,
            Activity::Hand if self.relocation.is_none() && self.time < HAND_REACH => {
                std::f32::consts::PI * (1.0 - self.time / HAND_REACH)
            }
            _ => 0.0,
        }
    }

    pub fn start(&mut self, activity: Activity) {
        if activity != Activity::Hand {
            self.relocation = None;
        }
        self.activity = activity;
        self.time = 0.0;
        self.phase = 0.0;
        match activity {
            Activity::Fall => self.soft_bounced = false,
            Activity::Down => self.down_for = 1.5 + 1.5 * fastrand::f32(),
            Activity::Rise => self.rise_for = 1.0 + fastrand::f32(),
            _ => {}
        }
    }

    pub fn approach(&mut self, room: &Room, x: f32, dt: f32) -> bool {
        self.approach_at(room, x, WALK_SPEED, dt)
    }

    pub fn approach_at(&mut self, room: &Room, x: f32, speed: f32, dt: f32) -> bool {
        let goal = x.clamp(room.left, room.right);
        let distance = goal - self.x;
        if distance.abs() > 0.01 {
            self.face = distance.signum();
        }
        let travel = speed * dt;
        self.x += distance.clamp(-travel, travel);
        distance.abs() <= travel
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
            if !self.soft_bounced {
                let hard_throw = self.landing == Landing::Bounce;
                let distance = (room.floor - start_y).max(0.0);
                let impact_speed = (start_vy * start_vy + 2.0 * GRAVITY * distance).sqrt();
                let (rebound, minimum) = if hard_throw {
                    let speed_limit = (2.0 * GRAVITY * room.floor * 0.45).sqrt();
                    (
                        (impact_speed * BOUNCE_RESTITUTION).min(speed_limit),
                        MIN_REBOUND_SPEED,
                    )
                } else {
                    (
                        (impact_speed * SOFT_RESTITUTION).min(MAX_SOFT_REBOUND),
                        MIN_SOFT_REBOUND,
                    )
                };
                if rebound >= minimum {
                    // Use the remainder of this tick for upward motion after the impact.
                    let impact_time = (impact_speed - start_vy) / GRAVITY;
                    let remaining = (dt - impact_time).clamp(0.0, dt);
                    self.vy = -rebound + GRAVITY * remaining;
                    self.y -= remaining * (rebound - 0.5 * GRAVITY * remaining);
                    self.vx *= 0.8;
                    self.soft_bounced = !hard_throw;
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
        if self.vx != 0.0 {
            self.time = 0.0;
        }
        if self.time >= self.down_for {
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
        if (self.face < 0.0 && self.x <= room.left) || (self.face > 0.0 && self.x >= room.right) {
            self.x = self.x.clamp(room.left, room.right);
            let roll = fastrand::u8(0..100);
            if (10..15).contains(&roll) {
                self.sled_edge = Some(self.face > 0.0);
            }
            if self.reach_wall(room, roll) {
                return;
            }
        }
        if self.time >= self.walk_for {
            self.start(Activity::Idle);
        }
    }

    pub fn take_sled_edge(&mut self) -> Option<bool> {
        self.sled_edge.take()
    }

    fn reach_wall(&mut self, room: &Room, roll: u8) -> bool {
        if roll < 10 && room.floor - room.ceiling > 40.0 {
            self.vx = 0.0;
            self.vy = 0.0;
            self.start(Activity::ClimbWall);
            true
        } else {
            self.face = -self.face;
            false
        }
    }

    fn climb_wall(&mut self, room: &Room, dt: f32, slippery: bool) {
        self.x = if self.face < 0.0 {
            room.left
        } else {
            room.right
        };
        let limit = if slippery {
            room.ceiling + (room.floor - room.ceiling) * 0.25
        } else {
            room.ceiling
        };
        let advance = |time: f32| {
            let strokes = time.max(0.0) / WALL_STROKE;
            let pull = ((strokes.fract() - 0.22) / 0.78).clamp(0.0, 1.0);
            WALL_SPEED * WALL_STROKE * (strokes.floor() + pull * pull * (3.0 - 2.0 * pull))
        };
        let distance = advance(self.time) - advance(self.time - dt);
        self.y = (self.y - distance).max(limit);
        if self.y <= limit {
            self.leave_wall(slippery, fastrand::u8(0..100));
        }
    }

    fn leave_wall(&mut self, slippery: bool, roll: u8) {
        if slippery {
            if roll < 50 {
                self.start(Activity::SlideWall);
            } else {
                self.knock_back(-self.face * 145.0, 20.0);
            }
        } else {
            self.face = -self.face;
            self.start(if roll < 95 {
                Activity::Parachute
            } else {
                Activity::ClimbCeiling
            });
        }
    }

    fn slide_wall(&mut self, room: &Room, dt: f32) {
        self.y = (self.y + SLIDE_SPEED * dt).min(room.floor);
        if self.y >= room.floor {
            self.start(Activity::Special(Special::WallRecover));
        }
    }

    fn climb_ceiling(&mut self, room: &Room, dt: f32) {
        self.y = room.ceiling;
        let centre = (room.left + room.right) * 0.5;
        let distance = centre - self.x;
        self.x += distance.clamp(-CLIMB_SPEED * dt, CLIMB_SPEED * dt);
        if self.x == centre {
            self.start(Activity::Hand);
        }
    }

    fn parachute(&mut self, room: &Room, dt: f32) {
        self.x = (self.x + self.face * 52.0 * dt).clamp(room.left, room.right);
        self.y = (self.y + PARACHUTE_SPEED * dt).min(room.floor);
        if self.y >= room.floor {
            self.start(Activity::Idle);
        }
    }

    pub fn walk_for(&mut self, seconds: f32) {
        self.walk_for = seconds;
        self.start(Activity::Walk);
    }

    pub fn move_by_hand(&mut self, target: f32) {
        self.start(Activity::Hand);
        self.relocation = Some((self.x, self.y, target));
    }

    fn hand(&mut self, room: &Room, dt: f32) {
        if let Some((x, y, target)) = self.relocation {
            let p = ((self.time - HAND_REACH) / 1.45).clamp(0.0, 1.0);
            let travel = p * p * (3.0 - 2.0 * p);
            self.x = (x + (target - x) * travel).clamp(room.left, room.right);
            self.y = (y + (room.floor - y) * travel - (p * std::f32::consts::PI).sin() * 95.0)
                .max(room.ceiling);
            if p >= 1.0 {
                self.start(Activity::HandRelease);
            }
            return;
        }
        if self.time >= HAND_REACH {
            self.y = (self.y + HAND_SPEED * dt).min(room.floor);
            if self.y >= room.floor {
                self.start(Activity::HandRelease);
            }
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
            ceiling: 55.0,
        }
    }

    #[test]
    fn fall_lands_on_the_floor() {
        let room = room();
        let mut pet = Pet::on_floor(&room, None);
        pet.y = 100.0;
        pet.become_for_test(Activity::Fall);
        for _ in 0..180 {
            pet.step(&room, 1.0 / 60.0, false);
        }
        assert_ne!(pet.activity, Activity::Fall);
        assert!((pet.y - room.floor).abs() < 0.1);
    }

    #[test]
    fn walk_turns_around_at_the_right_edge() {
        let room = room();
        let mut pet = Pet::on_floor(&room, None);
        pet.x = room.right;
        pet.face = 1.0;
        pet.become_for_test(Activity::Walk);
        assert!(!pet.reach_wall(&room, 10));
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
            pet.step(&room, 1.0 / 60.0, false);
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
            pet.step(&room, 1.0 / 60.0, false);
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
            pet.step(&room, 1.0 / 60.0, false);
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
            pet.step(&room, 1.0 / 60.0, false);
            if pet.activity == Activity::Down {
                saw_down = true;
            }
        }
        assert!(!saw_down);
        assert!((pet.y - room.floor).abs() < 0.1);
        assert_ne!(pet.activity, Activity::Fall);
    }

    #[test]
    fn a_drop_and_a_knockback_bounce_once_before_landing() {
        let room = room();
        for fps in [30.0, 60.0, 144.0] {
            for knocked_back in [false, true] {
                let mut pet = Pet::on_floor(&room, None);
                let landing = if knocked_back {
                    pet.knock_back(900.0, -310.0);
                    Activity::Down
                } else {
                    pet.y -= 160.0;
                    pet.release(&room, 0.0, 0.0);
                    Activity::Idle
                };
                let mut rebounds = 0;
                let mut height: f32 = 0.0;
                for _ in 0..(fps * 2.0) as usize {
                    let falling = pet.vy > 0.0;
                    pet.step(&room, 1.0 / fps, false);
                    assert!(pet.y <= room.floor);
                    if falling && pet.vy < 0.0 {
                        rebounds += 1;
                    }
                    if rebounds > 0 {
                        height = height.max(room.floor - pet.y);
                    }
                    if pet.activity != Activity::Fall {
                        break;
                    }
                }
                assert_eq!(pet.activity, landing);
                assert_eq!(rebounds, 1, "{fps} fps, knocked_back={knocked_back}");
                assert!((2.0..=16.0).contains(&height), "{height}");
            }
        }
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
            for _ in 0..(fps * 7.0) as usize {
                let was_rising = pet.vy < 0.0;
                pet.step(&room, 1.0 / fps, false);
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
            ceiling: 55.0,
        };
        for count in 3..=5 {
            let mut pet = Pet::on_floor(&room, Some(0.8));
            let start_x = pet.x;
            pet.choose_activity(49, count);
            assert_eq!(pet.activity, Activity::Walk);
            while pet.activity == Activity::Walk {
                pet.step(&room, 0.04, false);
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
            pet.step(&room, 1.0 / 60.0, false);
        }
        assert!(pet.x > start + 80.0, "{}", pet.x);
        assert_eq!(pet.activity, Activity::Down);
    }

    #[test]
    fn recovery_times_are_chosen_once_for_each_fall() {
        let room = room();
        let mut pet = Pet::on_floor(&room, None);
        let mut previous = None;
        let mut varied = false;
        for _ in 0..12 {
            pet.start(Activity::Down);
            let down = pet.down_for;
            assert!((1.5..=3.0).contains(&down));
            pet.step(&room, 0.02, false);
            assert_eq!(pet.down_for, down);
            pet.time = down - 0.01;
            pet.step(&room, 0.02, false);
            assert_eq!(pet.activity, Activity::Rise);
            let rise = pet.rise_for;
            assert!((1.0..=2.0).contains(&rise));
            pet.step(&room, 0.02, false);
            assert_eq!(pet.rise_for, rise);
            varied |= previous.is_some_and(|timing| timing != (down, rise));
            previous = Some((down, rise));
        }
        assert!(varied);
    }

    #[test]
    fn wall_routes_return_to_the_floor_for_both_seasons() {
        let room = room();
        for (slippery, roll, expected) in [
            (true, 49, Activity::SlideWall),
            (true, 50, Activity::Fall),
            (false, 94, Activity::Parachute),
            (false, 95, Activity::ClimbCeiling),
        ] {
            let mut pet = Pet::on_floor(&room, None);
            pet.x = room.left;
            pet.face = -1.0;
            assert!(pet.reach_wall(&room, 9));
            for _ in 0..6 {
                pet.step(&room, 0.05, slippery);
            }
            assert!(pet.y < room.floor);
            pet.y = if slippery { 150.0 } else { room.ceiling };
            pet.leave_wall(slippery, roll);
            assert_eq!(pet.activity, expected);
            let mut used_hand = false;
            let mut crouched = false;
            let mut walked_after_crouching = false;
            for _ in 0..600 {
                used_hand |= pet.activity == Activity::Hand;
                crouched |= pet.activity == Activity::Special(Special::WallRecover);
                walked_after_crouching |= crouched && pet.activity == Activity::Walk;
                pet.step(&room, 1.0 / 60.0, slippery);
                if pet.activity == Activity::Idle {
                    break;
                }
            }
            assert_eq!(used_hand, expected == Activity::ClimbCeiling);
            assert_eq!(crouched, expected == Activity::SlideWall);
            assert_eq!(walked_after_crouching, crouched);
            assert_eq!(pet.y, room.floor);
            assert_eq!(pet.activity, Activity::Idle);
            if used_hand {
                assert_eq!(pet.x, (room.left + room.right) * 0.5);
            }
        }
    }

    impl Pet {
        fn become_for_test(&mut self, activity: Activity) {
            self.start(activity);
        }
    }
}
