// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Loose joints while the pet is held or thrown.
//! Angle zero is the drawn pose. Positive rotation is clockwise on a y-down screen.

use crate::pack::Pack;
use crate::sim::GRAVITY;

struct Spec {
    dynamic: bool,
    limit: f32,
    spring: f32,
    length: f32,
}

struct Joint {
    parent: Option<usize>,
    angle: f32,
    omega: f32,
    limit: f32,
    spring: f32,
    length: f32,
    dynamic: bool,
}

pub struct Ragdoll {
    pub id: String,
    pub live: bool,
    joints: Vec<Joint>,
}

impl Ragdoll {
    pub fn new(pack: &Pack) -> Self {
        let joints = pack
            .bones
            .iter()
            .map(|bone| {
                let spec = spec(&bone.name);
                Joint {
                    parent: bone.parent,
                    angle: 0.0,
                    omega: 0.0,
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
            joints,
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
            let alpha =
                (-gx * cos - gy * sin) / length - joint.spring * joint.angle - 1.6 * joint.omega;
            joint.omega = (joint.omega + alpha * dt).clamp(-7.0, 7.0);
            joint.angle += joint.omega * dt;
            if joint.angle > joint.limit {
                joint.angle = joint.limit;
                if joint.omega > 0.0 {
                    joint.omega = 0.0;
                }
            } else if joint.angle < -joint.limit {
                joint.angle = -joint.limit;
                if joint.omega < 0.0 {
                    joint.omega = 0.0;
                }
            }
        }
    }

    pub fn relax(&mut self, dt: f32) {
        if !self.live {
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        let mut moving = false;
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

fn spec(name: &str) -> Spec {
    match name {
        "torso" => Spec {
            dynamic: true,
            limit: 0.55,
            spring: 26.0,
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
}
