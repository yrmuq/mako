// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Bear girl, and any other loaded chibi, posed from separate parts on one skeleton.

#[cfg(test)]
use tiny_skia::PremultipliedColorU8;
use tiny_skia::{Pixmap, PixmapPaint, Point, Transform};

use crate::pack::{BonePoint, Pack, Pose, Sprite, BUILTIN_ID};
#[cfg(test)]
use crate::sim::FEET_Y;
use crate::sim::{FEET_X, SPRITE_H, SPRITE_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walk,
    Fall,
    Drag,
    Down,
    Sit,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub cycle: f32,
    pub motion: Motion,
    pub tilt: f32,
    pub facing_left: bool,
    pub scales: (f32, f32),
    pub pose: Vec<f32>,
    pub anchor: Option<BonePoint>,
}

pub struct Frame {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub parts: Vec<Part>,
    pub shift: (i32, i32),
    pub anchor: Option<Point>,
    pub support: Option<f32>,
}

pub struct Part {
    sprite: Sprite,
    transform: Transform,
    joint: Option<BonePoint>,
}

pub fn pick(parts: &[Part], x: f32, y: f32) -> Option<BonePoint> {
    for part in parts.iter().rev() {
        let Some(inverse) = part.transform.invert() else {
            continue;
        };
        let mut pixel = Point::from_xy(x, y);
        inverse.map_point(&mut pixel);
        let image = &part.sprite.image;
        if !(0.0..image.width() as f32).contains(&pixel.x)
            || !(0.0..image.height() as f32).contains(&pixel.y)
        {
            continue;
        }
        if image
            .pixel(pixel.x as u32, pixel.y as u32)
            .is_some_and(|color| color.alpha() >= 32)
        {
            return part.joint.map(|joint| BonePoint {
                bone: joint.bone,
                local: Point::from_xy(joint.local.x + pixel.x, joint.local.y + pixel.y),
            });
        }
    }
    None
}

pub fn portrait(pack: &Pack) -> Option<Pixmap> {
    let head = pack.bones.iter().position(|bone| bone.name == "head")?;
    let pose = pack.pose("idle", 0.0);
    let (sprite, transform) = posed_sprite(pack, &pose, head)?;
    let mut corners = [
        Point::from_xy(0.0, 0.0),
        Point::from_xy(sprite.image.width() as f32, 0.0),
        Point::from_xy(0.0, sprite.image.height() as f32),
        Point::from_xy(sprite.image.width() as f32, sprite.image.height() as f32),
    ];
    transform.map_points(&mut corners);
    let extent = |axis: fn(&Point) -> f32| {
        let min = corners.iter().map(axis).fold(f32::INFINITY, f32::min);
        let max = corners.iter().map(axis).fold(f32::NEG_INFINITY, f32::max);
        (min, max - min)
    };
    let (x, width) = extent(|point| point.x);
    let (y, height) = extent(|point| point.y);
    let scale = 80.0 / width.max(height).max(1.0);
    let mut portrait = Pixmap::new(88, 88)?;
    let view = Transform::from_translate(-x, -y)
        .post_scale(scale, scale)
        .post_translate((88.0 - width * scale) / 2.0, (88.0 - height * scale) / 2.0);
    let mut order: Vec<_> = (0..pack.bones.len()).collect();
    order.sort_by_key(|&index| pack.bones[index].z);
    for index in order {
        let mut parent = Some(index);
        while let Some(ancestor) = parent.filter(|ancestor| *ancestor != head) {
            parent = pack.bones[ancestor].parent;
        }
        if parent.is_some() {
            if let Some((sprite, transform)) = posed_sprite(pack, &pose, index) {
                portrait.draw_pixmap(
                    0,
                    0,
                    sprite.image.as_ref().as_ref(),
                    &PixmapPaint::default(),
                    transform.post_concat(view),
                    None,
                );
            }
        }
    }
    Some(portrait)
}

pub fn scene_size((x, y): (f32, f32)) -> (u32, u32) {
    ((SPRITE_W * x).round() as u32, (SPRITE_H * y).round() as u32)
}

pub fn render(scene: &Scene, pack: &Pack, scale: u32) -> Frame {
    let scale = scale.max(1);
    let (logical_w, logical_h) = scene_size(scene.scales);
    let (mut parts, mut anchor) = pet_parts(scene, pack);
    let dynamic_pose = matches!(scene.motion, Motion::Drag | Motion::Fall | Motion::Down)
        || !scene.pose.is_empty();
    let (left, top, right, bottom) = if dynamic_pose {
        let bounds = part_bounds(&parts, logical_w as i32, logical_h as i32);
        if scene.motion == Motion::Drag {
            bounds
        } else {
            // The transformed ink is already aligned to the floor contact line.
            (bounds.0, bounds.1, bounds.2, logical_h as i32)
        }
    } else {
        (0, 0, logical_w as i32, logical_h as i32)
    };
    let width = (right - left) as u32 * scale;
    let height = (bottom - top) as u32 * scale;
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    if let Some(anchor) = anchor.as_mut() {
        anchor.x -= left as f32;
        anchor.y -= top as f32;
    }
    let support = if scene.motion == Motion::Drag {
        parts
            .iter()
            .filter_map(|part| part.sprite.lower_edge(part.transform))
            .max_by(f32::total_cmp)
            .map(|edge| edge - top as f32)
    } else {
        None
    };
    for part in &mut parts {
        part.transform = part.transform.post_translate(-left as f32, -top as f32);
        pixmap.draw_pixmap(
            0,
            0,
            part.sprite.image.as_ref().as_ref(),
            &PixmapPaint::default(),
            part.transform.post_scale(scale as f32, scale as f32),
            None,
        );
    }
    Frame {
        bgra: rgba_to_bgra(pixmap.data()),
        width,
        height,
        parts,
        shift: (left, top),
        anchor,
        support,
    }
}

fn part_bounds(parts: &[Part], width: i32, height: i32) -> (i32, i32, i32, i32) {
    let (mut left, mut top, mut right, mut bottom) = (0, 0, width, height);
    for part in parts {
        let w = part.sprite.image.width() as f32;
        let h = part.sprite.image.height() as f32;
        let mut corners = [
            Point::from_xy(0.0, 0.0),
            Point::from_xy(w, 0.0),
            Point::from_xy(0.0, h),
            Point::from_xy(w, h),
        ];
        part.transform.map_points(&mut corners);
        for corner in corners {
            left = left.min(corner.x.floor() as i32);
            top = top.min(corner.y.floor() as i32);
            right = right.max(corner.x.ceil() as i32);
            bottom = bottom.max(corner.y.ceil() as i32);
        }
    }
    let pad = width.max(height);
    (
        left.max(-pad),
        top.max(-pad),
        right.min(width + pad),
        bottom.min(height + pad),
    )
}

fn pet_parts(scene: &Scene, pack: &Pack) -> (Vec<Part>, Option<Point>) {
    let anim = match scene.motion {
        Motion::Walk => "walk",
        Motion::Fall => "fall",
        Motion::Drag => "drag",
        Motion::Down => "idle",
        Motion::Sit => "sit",
        Motion::Idle => "idle",
    };
    let pose = if scene.pose.is_empty() {
        pack.pose(anim, scene.cycle)
    } else {
        pack.pose_with(anim, scene.cycle, Some(&scene.pose))
    };
    let (scale_x, scale_y) = scene.scales;
    let fit = (SPRITE_W / pack.canvas_w).min(SPRITE_H / pack.canvas_h);
    let (fit_x, fit_y) = (fit * scale_x, fit * scale_y);
    let ground = scene_size(scene.scales).1 as f32;
    let centre_x = FEET_X * scale_x;
    let bob = pose.bob * fit_y;
    let mut view = Transform::from_scale(fit_x, fit_y)
        .post_translate(
            centre_x - pack.feet_x * fit_x,
            ground - pack.feet_y * fit_y + bob,
        )
        .post_rotate_at(scene.tilt.to_degrees(), centre_x, ground + bob);
    let support_motion = match scene.motion {
        Motion::Idle | Motion::Walk if !scene.pose.is_empty() || scene.tilt.abs() > 0.001 => {
            Motion::Down
        }
        motion => motion,
    };
    if let Some(contact) = pose_contact(pack, &pose, support_motion, view) {
        view = view.post_translate(0.0, ground - contact);
    }
    let mirror = if scene.facing_left {
        let width = scene_size(scene.scales).0 as f32;
        Transform::from_row(-1.0, 0.0, 0.0, 1.0, width, 0.0)
    } else {
        Transform::identity()
    };
    let view = view.post_concat(mirror);
    let mut order: Vec<usize> = (0..pack.bones.len()).collect();
    order.sort_by_key(|&index| {
        let bone = &pack.bones[index];
        let layer = match scene.motion {
            Motion::Sit
                if bone.name.ends_with("hand")
                    && (pack.id != BUILTIN_ID || bone.name == "left-hand") =>
            {
                2
            }
            Motion::Sit if pack.id != BUILTIN_ID && leg(&bone.name) => 1,
            _ => 0,
        };
        (layer, bone.z)
    });
    let mut parts: Vec<_> = order
        .into_iter()
        .filter_map(|index| {
            let (sprite, local) = posed_sprite(pack, &pose, index)?;
            let bone = &pack.bones[index];
            Some(Part {
                sprite: sprite.clone(),
                transform: local.post_concat(view),
                joint: Some(BonePoint {
                    bone: index,
                    local: Point::from_xy(sprite.x - bone.pivot_x, sprite.y - bone.pivot_y),
                }),
            })
        })
        .collect();
    if scene.motion == Motion::Sit {
        parts.extend(laptop_part(ground, scene.scales, mirror, pack));
    }
    let anchor = scene.anchor.map(|anchor| {
        let mut position = anchor.in_pose(&pose);
        view.map_point(&mut position);
        position
    });
    (parts, anchor)
}

fn leg(name: &str) -> bool {
    name.ends_with("leg") || name.ends_with("foot")
}

fn pose_contact(pack: &Pack, pose: &Pose, motion: Motion, view: Transform) -> Option<f32> {
    let supports = |bone: &str| match motion {
        Motion::Idle | Motion::Walk => leg(bone),
        Motion::Sit => bone == "torso" || leg(bone),
        Motion::Down | Motion::Fall => true,
        Motion::Drag => false,
    };
    pack.bones
        .iter()
        .enumerate()
        .filter(|(_, bone)| supports(&bone.name))
        .filter_map(|(index, _)| {
            let (sprite, local) = posed_sprite(pack, pose, index)?;
            sprite.lower_edge(local.post_concat(view))
        })
        .max_by(f32::total_cmp)
}

fn posed_sprite<'a>(pack: &'a Pack, pose: &Pose, index: usize) -> Option<(&'a Sprite, Transform)> {
    let bone = &pack.bones[index];
    let image = &pose.image[index];
    let sprite = match image.strip_suffix('?') {
        Some(preferred) => pack.sprite(preferred).or_else(|| pack.sprite(&bone.image)),
        None => pack.sprite(image),
    }?;
    if sprite.is_blank() {
        return None;
    }
    let placed = &pose.placed[index];
    let rad = placed.rot.to_radians();
    let (sin, cos) = rad.sin_cos();
    let local = Transform::from_row(
        cos,
        sin,
        -sin,
        cos,
        cos * (sprite.x - bone.pivot_x) - sin * (sprite.y - bone.pivot_y) + placed.x,
        sin * (sprite.x - bone.pivot_x) + cos * (sprite.y - bone.pivot_y) + placed.y,
    );
    Some((sprite, local))
}

fn laptop_part(
    ground: f32,
    (scale_x, scale_y): (f32, f32),
    mirror: Transform,
    pack: &Pack,
) -> Option<Part> {
    let sprite = pack.sprite("it-1")?;
    if sprite.is_blank() {
        return None;
    }
    let sw = sprite.image.width() as f32;
    let sh = sprite.image.height() as f32;
    let dw = 86.0 * scale_x;
    let dh = 86.0 * scale_y * sh / sw;
    let x = (FEET_X + 50.0) * scale_x - dw * 0.5;
    let y = ground - dh;
    Some(Part {
        sprite: sprite.clone(),
        transform: Transform::from_scale(dw / sw, dh / sh)
            .post_translate(x, y)
            .post_concat(mirror),
        joint: None,
    })
}

fn rgba_to_bgra(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0; data.len()];
    let (src_chunks, _) = data.as_chunks::<4>();
    let (dst_chunks, _) = out.as_chunks_mut::<4>();
    for (src, dst) in src_chunks.iter().zip(dst_chunks) {
        *dst = [src[2], src[1], src[0], src[3]];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn still() -> Scene {
        Scene {
            cycle: 0.0,
            motion: Motion::Idle,
            tilt: 0.0,
            facing_left: false,
            scales: (1.0, 1.0),
            pose: Vec::new(),
            anchor: None,
        }
    }

    #[test]
    fn pick_does_not_wrap_pixels_past_the_right_edge() {
        let pack = Pack::builtin();
        let sprite = pack.sprite("torso").unwrap().clone();
        let width = sprite.image.width();
        let height = sprite.image.height();
        let mut opaque = None;
        for y in 1..height {
            if let Some(x) = (0..width).find(|x| sprite.image.pixel(*x, y).unwrap().alpha() >= 32) {
                opaque = Some((x, y));
                break;
            }
        }
        let (x, y) = opaque.expect("torso has opaque pixels after its first row");
        let part = Part {
            sprite,
            transform: Transform::identity(),
            joint: Some(BonePoint {
                bone: 0,
                local: Point::from_xy(0.0, 0.0),
            }),
        };
        let parts = [part];
        assert!(pick(&parts, x as f32, y as f32).is_some());
        assert!(pick(&parts, (width + x) as f32, (y - 1) as f32).is_none());
    }

    #[test]
    fn idle_pose_is_mostly_transparent_around_a_solid_body() {
        let pack = Pack::builtin();
        let scene = still();
        let frame = render(&scene, &pack, 1);
        let (pixels, _) = frame.bgra.as_chunks::<4>();
        let opaque = pixels.iter().filter(|pixel| pixel[3] > 16).count();
        let fraction = opaque as f32 / pixels.len() as f32;
        assert!(fraction > 0.08, "{fraction}");
        assert!(fraction < 0.75, "{fraction}");
        assert!(pixels.iter().any(|pixel| pixel[3] > 200));
        std::fs::write("/tmp/mako-idle.png", encode(&scene)).expect("preview");
    }

    #[test]
    fn physics_angles_keep_a_body_in_the_frame() {
        let pack = Pack::builtin();
        let mut scene = still();
        scene.motion = Motion::Drag;
        scene.pose = pack
            .bones
            .iter()
            .map(|bone| match bone.name.as_str() {
                "left-hand" => 48.0,
                "right-hand" => -42.0,
                "left-leg" => 28.0,
                "right-leg" => -22.0,
                "head" => -18.0,
                "torso" => 12.0,
                _ => 0.0,
            })
            .collect();
        let frame = render(&scene, &pack, 1);
        let (pixels, _) = frame.bgra.as_chunks::<4>();
        let opaque = pixels.iter().filter(|pixel| pixel[3] > 16).count();
        assert!(opaque > 800, "{opaque}");
        std::fs::write("/tmp/mako-rag.png", encode(&scene)).expect("preview");
    }

    #[test]
    fn a_turned_impact_extends_the_frame_without_putting_ink_below_the_floor() {
        let pack = Pack::builtin();
        let mut scene = still();
        scene.tilt = 1.3;
        scene.pose = vec![0.0; pack.bones.len()];
        for motion in [Motion::Fall, Motion::Down, Motion::Idle] {
            scene.motion = motion;
            for facing_left in [false, true] {
                scene.facing_left = facing_left;
                let frame = render(&scene, &pack, 2);
                assert!(frame.width > 2 * SPRITE_W as u32);
                let width = (frame.width / 2) as i32;
                let height = (frame.height / 2) as i32;
                let (left, top, right, _) = part_bounds(&frame.parts, width, height);
                assert_eq!((left, top, right), (0, 0, width));
                assert_eq!(frame.shift.1 + height, FEET_Y as i32);
                let lowest_ink = frame
                    .parts
                    .iter()
                    .filter_map(|part| part.sprite.lower_edge(part.transform))
                    .max_by(f32::total_cmp)
                    .unwrap();
                assert!((lowest_ink - height as f32).abs() < 0.01);
            }
        }
    }

    #[test]
    fn sitting_draws_the_separate_laptop() {
        let pack = Pack::builtin();
        let mut scene = still();
        scene.motion = Motion::Sit;
        let sitting = render(&scene, &pack, 1);
        let rightmost_pixels = sitting
            .bgra
            .chunks_exact(sitting.width as usize * 4)
            .flat_map(|row| row[(FEET_X as usize + 80) * 4..].as_chunks::<4>().0);
        assert!(rightmost_pixels.filter(|pixel| pixel[3] > 16).count() > 100);
        std::fs::write("/tmp/mako-sit.png", encode(&scene)).expect("preview");
        scene.motion = Motion::Down;
        scene.pose = pack
            .bones
            .iter()
            .map(|bone| match bone.name.as_str() {
                "torso" => 81.0,
                "head" => 26.0,
                "left-hand" => -60.0,
                "right-hand" => 63.0,
                "left-leg" => 28.0,
                "right-leg" => -20.0,
                _ => 0.0,
            })
            .collect();
        std::fs::write("/tmp/mako-down.png", encode(&scene)).expect("preview");
    }

    #[test]
    fn walk_cycle_moves_a_planted_pose() {
        let pack = Pack::builtin();
        let scene = still();
        let idle = render(&scene, &pack, 1);
        let mut stepping = scene.clone();
        stepping.motion = Motion::Walk;
        stepping.cycle = 0.35;
        let walked = render(&stepping, &pack, 1);
        let (idle_px, _) = idle.bgra.as_chunks::<4>();
        let (walk_px, _) = walked.bgra.as_chunks::<4>();
        let changed = idle_px
            .iter()
            .zip(walk_px)
            .filter(|(left, right)| left != right)
            .count();
        assert!(changed > 400, "{changed}");
        std::fs::write("/tmp/mako-walk.png", encode(&stepping)).expect("preview");
        stepping.cycle = 0.0;
        std::fs::write("/tmp/mako-walk-b.png", encode(&stepping)).expect("preview");
    }

    #[test]
    fn walking_and_sitting_touch_the_same_ground_at_each_size() {
        let pack = Pack::builtin();
        let mut scene = still();
        for pet_scale in [0.2, 0.4, 0.6, 0.8, 1.0] {
            scene.scales = (pet_scale, pet_scale);
            for facing_left in [false, true] {
                scene.facing_left = facing_left;
                for motion in [Motion::Walk, Motion::Sit] {
                    scene.motion = motion;
                    for phase in [0.0, 0.14, 0.28, 0.42, 0.56, 0.69] {
                        scene.cycle = phase;
                        let scale = 2;
                        let frame = render(&scene, &pack, scale);
                        let last_ink_row = frame
                            .bgra
                            .chunks_exact(frame.width as usize * 4)
                            .rposition(|row| {
                                row.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 16)
                            })
                            .unwrap();
                        let ground = (FEET_Y * pet_scale * scale as f32).round() as usize;
                        assert!(
                            (last_ink_row + 1).abs_diff(ground) <= 2,
                            "{motion:?} at size {pet_scale}, phase {phase}: {last_ink_row} vs {ground}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn facing_left_mirrors_the_sprite() {
        let pack = Pack::builtin();
        let scene = still();
        let right = render(&scene, &pack, 1);
        let mut turned = scene.clone();
        turned.facing_left = true;
        let left = render(&turned, &pack, 1);
        let width = right.width as i32;
        let (right_px, _) = right.bgra.as_chunks::<4>();
        let (left_px, _) = left.bgra.as_chunks::<4>();
        let mut found = false;
        for y in 0..right.height as i32 {
            for x in width / 2..width {
                let index = (y * width + x) as usize;
                if right_px[index][3] > 200 {
                    let mirrored = (y * width + (width - 1 - x)) as usize;
                    assert_eq!(right_px[index], left_px[mirrored]);
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        assert!(found);
    }

    fn encode(scene: &Scene) -> Vec<u8> {
        let frame = render(scene, &Pack::builtin(), 2);
        let mut pixmap = Pixmap::new(frame.width, frame.height).expect("pixmap");
        let (src_chunks, _) = frame.bgra.as_chunks::<4>();
        for (src, dst) in src_chunks.iter().zip(pixmap.pixels_mut()) {
            *dst = PremultipliedColorU8::from_rgba(src[2], src[1], src[0], src[3])
                .unwrap_or(PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap());
        }
        pixmap.encode_png().expect("png")
    }
}
