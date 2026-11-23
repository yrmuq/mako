// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Loose joints while the pet is held or thrown.
//! Angle zero is the drawn pose. Positive rotation is clockwise on a y-down screen.

use std::f32::consts::{PI, TAU};

use tiny_skia::Point;

use crate::pack::{BonePoint, Pack, Pose};
use crate::sim::GRAVITY;

struct Spec {
    dynamic: bool,
    limit: f32,
    spring: f32,
    length: f32,
}

struct Joint {
    name: String,
    parent: Option<usize>,
    angle: f32,
    omega: f32,
    target: f32,
    limit: f32,
    spring: f32,
    length: f32,
    dynamic: bool,
}

pub struct Ragdoll {
    pub id: String,
    pub live: bool,
    planted: bool,
    joints: Vec<Joint>,
    turn: f32,
    turn_speed: f32,
}

impl Ragdoll {
    pub fn new(pack: &Pack) -> Self {
        let joints = pack
            .bones
            .iter()
            .map(|bone| {
                let spec = spec(&bone.name);
                Joint {
                    name: bone.name.clone(),
                    parent: bone.parent,
                    angle: 0.0,
                    omega: 0.0,
                    target: 0.0,
                    limit: spec.limit,
                    spring: spec.spring,
                    length: spec.length,
                    dynamic: spec.dynamic,
                }
            })
            .collect();
        Self {
            id: pack.id.clone(),
            live: false,
            planted: false,
            joints,
            turn: 0.0,
            turn_speed: 0.0,
        }
    }

    pub fn seed(&mut self, degrees: &[f32]) {
        for (joint, degree) in self.joints.iter_mut().zip(degrees) {
            joint.angle = if joint.dynamic {
                degree.to_radians()
            } else {
                0.0
            };
            joint.omega = 0.0;
        }
    }

    /// `ax` and `ay` are the acceleration of the grabbed body, in pixels per second squared.
    pub fn step(&mut self, dt: f32, ax: f32, ay: f32, pet_scale: f32) {
        if !self.live {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        self.steer_turn(dt, 0.0, 18.0);
        let scale = pet_scale.clamp(0.2, 1.0);
        // Gravity pulls every loose part. Subtract the body's own acceleration so a
        // real fall is weightless and a yank leaves the limbs behind.
        let gx = -ax;
        let gy = GRAVITY - ay;
        let worlds: Vec<f32> = (0..self.joints.len())
            .map(|index| self.world(index))
            .collect();
        for (index, joint) in self.joints.iter_mut().enumerate() {
            if !joint.dynamic {
                joint.angle = 0.0;
                joint.omega = 0.0;
                continue;
            }
            let (sin, cos) = worlds[index].sin_cos();
            let length = (joint.length * scale).max(8.0);
            let alpha = (-gx * cos - gy * sin) / length
                - joint.spring * (joint.angle - joint.target)
                - 1.6 * joint.omega;
            let limit = joint.limit.max(joint.angle.abs());
            joint.omega = (joint.omega + alpha * dt).clamp(-7.0, 7.0);
            joint.angle += joint.omega * dt;
            if joint.angle > limit {
                joint.angle = limit;
                if joint.omega > 0.0 {
                    joint.omega = 0.0;
                }
            } else if joint.angle < -limit {
                joint.angle = -limit;
                if joint.omega < 0.0 {
                    joint.omega = 0.0;
                }
            }
        }
    }

    pub fn suspend(&mut self, dt: f32, gravity: Point, pack: &Pack, grip: BonePoint) {
        let pose = pack.pose_with("idle", 0.0, Some(&self.degrees()));
        let pin = grip.in_pose(&pose);
        let centre = mass_centre(pack, &pose);
        let pull = gravity.x.hypot(gravity.y);
        if pull < 1.0 {
            return;
        }
        let desired = gravity.y.atan2(gravity.x) - (centre.y - pin.y).atan2(centre.x - pin.x);
        self.steer_turn(dt, desired, 48.0);
        let (sin, cos) = self.turn.sin_cos();
        let gx = (cos * gravity.x + sin * gravity.y) / pull;
        let gy = (-sin * gravity.x + cos * gravity.y) / pull;
        let strength = (pull / GRAVITY).clamp(0.0, 2.5);
        let held_side = match pack.bones[grip.bone].name.as_str() {
            "left-hand" => 1.0,
            "right-hand" => -1.0,
            _ => 0.0,
        };
        for (index, joint) in self.joints.iter_mut().enumerate().rev() {
            if !joint.dynamic || joint.parent.is_none() {
                continue;
            }
            let placed = pose.placed[index];
            let held = is_ancestor(pack, index, Some(grip.bone));
            let held_hand = held && joint.name.ends_with("hand");
            let leaning_head = held_side != 0.0 && joint.name == "head";
            let torque = if held_hand || leaning_head {
                0.0
            } else if held {
                // The pin holds still. The rest of the body pulls the grabbed joint away from it.
                let (x, y) = (pin.x - placed.x, pin.y - placed.y);
                -(x * gy - y * gx) / x.hypot(y).max(25.0) * 86.0
            } else {
                let (sin, cos) = placed.rot.to_radians().sin_cos();
                (-gx * cos - gy * sin) * 34.0
            };
            let limit = if held_hand {
                2.9
            } else if !held && joint.name.ends_with("leg") {
                1.5
            } else {
                joint.limit
            };
            let (spring, rest) = if held_hand {
                // An extended sleeve keeps the grip outside the hood.
                (36.0, held_side * 2.78)
            } else if held {
                (2.0, 0.0)
            } else if leaning_head {
                (joint.spring, held_side * 0.65)
            } else {
                (joint.spring, 0.0)
            };
            joint.omega +=
                (torque * strength - spring * (joint.angle - rest) - 8.0 * joint.omega) * dt;
            joint.angle = (joint.angle + joint.omega * dt).clamp(-limit, limit);
            if joint.angle.abs() >= limit {
                joint.omega = 0.0;
            }
        }
    }

    pub fn turn(&self) -> f32 {
        self.turn
    }

    fn steer_turn(&mut self, dt: f32, target: f32, spring: f32) {
        let error = wrap_angle(target - self.turn);
        self.turn_speed += (spring * error - 9.0 * self.turn_speed) * dt;
        self.turn = wrap_angle(self.turn + self.turn_speed * dt);
    }

    pub fn plant(&mut self, face: f32, slide: f32) {
        let sign = if face < 0.0 { -1.0 } else { 1.0 };
        let first = !self.planted;
        self.planted = true;
        self.live = true;
        let drag = (slide / 500.0).clamp(-2.4, 2.4);
        for joint in &mut self.joints {
            let target = match joint.name.as_str() {
                "torso" => sign * 1.42,
                "head" => sign * 0.45,
                "left-hand" => -1.05,
                "right-hand" => 1.1,
                "left-leg" => 0.5,
                "right-leg" => -0.35,
                "left-foot" => 0.3,
                "right-foot" => -0.25,
                _ => 0.0,
            };
            joint.target = target;
            if !first {
                continue;
            }
            if joint.name == "torso" {
                joint.omega += sign * 7.0;
            } else if joint.name == "left-hand" {
                joint.omega -= 3.2 + drag * 1.4;
            } else if joint.name == "right-hand" {
                joint.omega += 3.2 - drag * 1.4;
            } else if joint.name == "left-leg" || joint.name == "right-leg" {
                joint.omega -= drag * 0.8;
            }
        }
    }

    pub fn clear_plant(&mut self) {
        self.planted = false;
        for joint in &mut self.joints {
            joint.target = 0.0;
        }
    }

    pub fn is_planted(&self) -> bool {
        self.planted
    }

    #[cfg(test)]
    fn torso_degrees(&self) -> f32 {
        self.joints
            .iter()
            .find(|joint| joint.name == "torso")
            .map(|joint| joint.angle.to_degrees())
            .unwrap_or(0.0)
    }

    pub fn relax(&mut self, dt: f32) {
        if !self.live {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        self.steer_turn(dt, 0.0, 36.0);
        let mut moving = self.turn.abs() > 0.015 || self.turn_speed.abs() > 0.1;
        for joint in &mut self.joints {
            if !joint.dynamic {
                joint.angle = 0.0;
                joint.omega = 0.0;
                continue;
            }
            joint.omega += (-36.0 * joint.angle - 10.0 * joint.omega) * dt;
            joint.angle += joint.omega * dt;
            if joint.angle.abs() > 0.03 || joint.omega.abs() > 0.25 {
                moving = true;
            }
        }
        if !moving {
            self.turn = 0.0;
            self.turn_speed = 0.0;
            self.live = false;
        }
    }

    pub fn degrees(&self) -> Vec<f32> {
        self.joints
            .iter()
            .map(|joint| joint.angle.to_degrees())
            .collect()
    }

    fn world(&self, mut index: usize) -> f32 {
        let mut total = 0.0;
        for _ in 0..8 {
            total += self.joints[index].angle;
            let Some(parent) = self.joints[index].parent else {
                break;
            };
            index = parent;
        }
        total
    }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn is_ancestor(pack: &Pack, joint: usize, mut child: Option<usize>) -> bool {
    while let Some(index) = child {
        if index == joint {
            return true;
        }
        child = pack.bones[index].parent;
    }
    false
}

fn mass_centre(pack: &Pack, pose: &Pose) -> Point {
    let mut point = Point::from_xy(0.0, 0.0);
    let mut total = 0.0;
    for (index, bone) in pack.bones.iter().enumerate() {
        let Some(sprite) = pack.sprite(&bone.image) else {
            continue;
        };
        if sprite.is_blank() {
            continue;
        }
        let mass = match bone.name.as_str() {
            "torso" => 5.0,
            "head" => 4.0,
            "left-hand" | "right-hand" | "left-leg" | "right-leg" => 1.0,
            _ => 0.2,
        };
        let centre = pose.placed[index].point(Point::from_xy(
            sprite.x + sprite.image.width() as f32 * 0.5 - bone.pivot_x,
            sprite.y + sprite.image.height() as f32 * 0.5 - bone.pivot_y,
        ));
        point.x += centre.x * mass;
        point.y += centre.y * mass;
        total += mass;
    }
    if total > 0.0 {
        Point::from_xy(point.x / total, point.y / total)
    } else {
        Point::from_xy(pack.feet_x, pack.feet_y * 0.5)
    }
}

fn spec(name: &str) -> Spec {
    match name {
        "torso" => Spec {
            dynamic: true,
            limit: 1.55,
            spring: 18.0,
            length: 96.0,
        },
        "head" => Spec {
            dynamic: true,
            limit: 0.75,
            spring: 16.0,
            length: 42.0,
        },
        "left-hand" | "right-hand" => Spec {
            dynamic: true,
            limit: 1.45,
            spring: 5.0,
            length: 68.0,
        },
        "left-leg" | "right-leg" => Spec {
            dynamic: true,
            limit: 1.05,
            spring: 9.0,
            length: 78.0,
        },
        "left-foot" | "right-foot" => Spec {
            dynamic: true,
            limit: 0.8,
            spring: 14.0,
            length: 30.0,
        },
        _ => Spec {
            dynamic: false,
            limit: 0.0,
            spring: 0.0,
            length: 1.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hand(rag: &Ragdoll) -> f32 {
        rag.joints
            .iter()
            .zip(Pack::builtin().bones)
            .find(|(_, bone)| bone.name == "left-hand")
            .map(|(joint, _)| joint.angle)
            .expect("left hand")
    }

    #[test]
    fn yanking_right_swings_the_arms_clockwise() {
        let mut rag = Ragdoll::new(&Pack::builtin());
        rag.live = true;
        for _ in 0..10 {
            rag.step(1.0 / 60.0, 7000.0, 0.0, 1.0);
        }
        assert!(hand(&rag) > 0.2, "{}", hand(&rag));
    }

    #[test]
    fn a_lifted_arm_falls_back_while_the_body_is_held() {
        let mut rag = Ragdoll::new(&Pack::builtin());
        rag.live = true;
        let index = Pack::builtin()
            .bones
            .iter()
            .position(|bone| bone.name == "left-hand")
            .expect("hand");
        rag.joints[index].angle = 0.9;
        for _ in 0..40 {
            rag.step(1.0 / 60.0, 0.0, 0.0, 1.0);
        }
        assert!(
            rag.joints[index].angle < 0.55,
            "{}",
            rag.joints[index].angle
        );
    }

    #[test]
    fn freefall_does_not_drop_a_posed_arm() {
        let mut rag = Ragdoll::new(&Pack::builtin());
        rag.live = true;
        let index = Pack::builtin()
            .bones
            .iter()
            .position(|bone| bone.name == "left-hand")
            .expect("hand");
        rag.joints[index].angle = 0.5;
        for _ in 0..12 {
            rag.step(1.0 / 60.0, 0.0, GRAVITY, 1.0);
        }
        assert!(
            (rag.joints[index].angle - 0.5).abs() < 0.2,
            "{}",
            rag.joints[index].angle
        );
    }

    #[test]
    fn a_landing_falls_flat_on_the_face() {
        let mut rag = Ragdoll::new(&Pack::builtin());
        rag.plant(1.0, 900.0);
        for _ in 0..50 {
            rag.step(1.0 / 60.0, 0.0, GRAVITY, 1.0);
        }
        assert!(rag.torso_degrees() > 50.0, "{}", rag.torso_degrees());
    }
}
