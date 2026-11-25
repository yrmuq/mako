// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Bear girl, and any other loaded chibi, posed from separate parts on one skeleton.

#[cfg(test)]
use tiny_skia::PremultipliedColorU8;
use tiny_skia::{IntRect, Pixmap, PixmapPaint, Point, Rect, Transform};

use crate::pack::{is_builtin, BonePoint, Face, Pack, Pose, Sprite};
use crate::props::SculptStage;
#[cfg(test)]
use crate::sim::FEET_Y;
use crate::sim::{Special, FEET_X, SPRITE_H, SPRITE_W, WALL_STROKE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
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
    Talk,
    Listen,
    PackSnow,
    ThrowSnow,
    Sculpt(SculptStage),
    Special(Special),
}

impl Motion {
    pub fn animation(self, pack: &Pack) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Fall => "fall",
            Self::Drag => "drag",
            Self::Down | Self::Rise | Self::Idle | Self::HandRelease => "idle",
            Self::Sit => "sit",
            Self::ClimbWall | Self::ClimbCeiling | Self::SlideWall => "climb",
            Self::Parachute if pack.animation_length("parachute").is_some() => "parachute",
            Self::Parachute | Self::Hand => "drag",
            Self::Talk => "talk",
            Self::Listen => "listen",
            Self::PackSnow => "pack-snow",
            Self::ThrowSnow => "throw-snow",
            Self::Sculpt(SculptStage::Base) => "sculpt",
            Self::Sculpt(SculptStage::Middle) => "sculpt-middle",
            Self::Sculpt(SculptStage::Top) => "sculpt-top",
            Self::Special(action) => action.animation(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub cycle: f32,
    pub blink: Option<f32>,
    pub sad: bool,
    pub grounded: f32,
    pub motion: Motion,
    pub tilt: f32,
    pub facing_left: bool,
    pub scales: (f32, f32),
    pub pose: Vec<f32>,
    pub anchor: Option<BonePoint>,
    pub stuck_balls: Vec<BonePoint>,
    pub effect_amount: f32,
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

impl Frame {
    pub fn input_rects(&self, scale: u32) -> Vec<IntRect> {
        let scale = scale.max(1) as usize;
        let width = self.width as usize;
        let mut strips: Vec<(i32, i32, i32, u32)> = Vec::new();
        let mut previous: Vec<usize> = Vec::new();
        let mut current = Vec::new();
        for (y, rows) in self.bgra.chunks_exact(width * scale * 4).enumerate() {
            let row = &rows[width * (scale / 2) * 4..][..width * 4];
            let mut start = None;
            let mut last = 0;
            for (x, alpha) in row
                .as_chunks::<4>()
                .0
                .iter()
                .skip(scale / 2)
                .step_by(scale)
                .map(|pixel| pixel[3])
                .chain(std::iter::once(0))
                .enumerate()
            {
                if alpha >= 32 {
                    start.get_or_insert(x as i32);
                    continue;
                }
                let Some(left) = start.take() else {
                    continue;
                };
                let right = x as i32;
                while last < previous.len() && strips[previous[last]].0 < left {
                    last += 1;
                }
                if let Some(&index) = previous.get(last) {
                    let strip = &mut strips[index];
                    if (strip.0, strip.1) == (left, right) {
                        strip.3 += 1;
                        current.push(index);
                        continue;
                    }
                }
                current.push(strips.len());
                strips.push((left, right, y as i32, 1));
            }
            std::mem::swap(&mut previous, &mut current);
            current.clear();
        }
        strips
            .into_iter()
            .filter_map(|(left, right, y, height)| {
                IntRect::from_xywh(left, y, (right - left) as u32, height)
            })
            .collect()
    }
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

pub fn collision_bounds(
    scene: &Scene,
    pack: &Pack,
    origin: Point,
    screen_width: f32,
) -> Option<Rect> {
    let (parts, _) = pet_parts(scene, pack);
    let body = parts
        .iter()
        .filter(|part| part.joint.is_some())
        .filter_map(|part| {
            Rect::from_xywh(
                0.0,
                0.0,
                part.sprite.image.width() as f32,
                part.sprite.image.height() as f32,
            )?
            .transform(part.transform)
        })
        .reduce(|a, b| {
            Rect::from_ltrb(
                a.left().min(b.left()),
                a.top().min(b.top()),
                a.right().max(b.right()),
                a.bottom().max(b.bottom()),
            )
            .expect("valid body bounds")
        })?;
    let (left, top, right, bottom) = frame_bounds(scene, &parts);
    let width = (right - left) as f32;
    let visible = Rect::from_xywh(left as f32, top as f32, width, (bottom - top) as f32)?;
    let crosses_edge =
        matches!(scene.motion, Motion::Special(action) if action.crosses_screen_edge());
    let x = frame_world_x(scene, origin.x, left, width, screen_width);
    let world_body = body
        .intersect(&visible)?
        .transform(Transform::from_translate(x - left as f32, origin.y.round()))?;
    if crosses_edge {
        world_body.intersect(&Rect::from_xywh(
            0.0,
            world_body.top(),
            screen_width,
            world_body.height(),
        )?)
    } else {
        Some(world_body)
    }
}

fn frame_world_x(scene: &Scene, origin: f32, left: i32, width: f32, screen_width: f32) -> f32 {
    let x = (origin + left as f32).round();
    let rightmost = (screen_width - width).max(0.0);
    match scene.motion {
        Motion::Special(action) if action.crosses_screen_edge() => x,
        Motion::ClimbWall | Motion::SlideWall => {
            if scene.facing_left {
                0.0
            } else {
                rightmost
            }
        }
        _ => x.clamp(0.0, rightmost),
    }
}

pub fn hit_anchor(
    scene: &Scene,
    pack: &Pack,
    origin: Point,
    width: f32,
    at: Point,
) -> Option<BonePoint> {
    let (parts, _) = pet_parts(scene, pack);
    let (left, _, right, _) = frame_bounds(scene, &parts);
    let x = frame_world_x(scene, origin.x, left, (right - left) as f32, width);
    let at = Point::from_xy(at.x - x + left as f32, at.y - origin.y.round());
    parts
        .iter()
        .rev()
        .filter_map(|part| {
            let joint = part.joint?;
            let mut local = at;
            part.transform.invert()?.map_point(&mut local);
            let nearest = part.sprite.closest_ink(local)?;
            let mut visible = nearest;
            part.transform.map_point(&mut visible);
            let distance = (visible.x - at.x).powi(2) + (visible.y - at.y).powi(2);
            let anchor = BonePoint {
                bone: joint.bone,
                local: Point::from_xy(joint.local.x + nearest.x, joint.local.y + nearest.y),
            };
            Some((distance, anchor))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, anchor)| anchor)
}

pub fn scene_size((x, y): (f32, f32)) -> (u32, u32) {
    ((SPRITE_W * x).round() as u32, (SPRITE_H * y).round() as u32)
}

pub fn render(scene: &Scene, pack: &Pack, scale: u32) -> Frame {
    let scale = scale.max(1);
    let (mut parts, mut anchor) = pet_parts(scene, pack);
    let (left, top, right, bottom) = frame_bounds(scene, &parts);
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

fn frame_bounds(scene: &Scene, parts: &[Part]) -> (i32, i32, i32, i32) {
    let (logical_w, logical_h) = scene_size(scene.scales);
    let extra_pad = matches!(
        scene.motion,
        Motion::Parachute | Motion::Hand | Motion::HandRelease
    );
    let pad = logical_w.max(logical_h) as i32 * if extra_pad { 2 } else { 1 };
    let surface_bounds = || part_bounds(parts, logical_w as i32, logical_h as i32, pad);
    match scene.motion {
        Motion::ClimbWall | Motion::SlideWall => {
            let (left, top, right, bottom) = surface_bounds();
            return if scene.facing_left {
                (0, top, right, bottom)
            } else {
                (left, top, logical_w as i32, bottom)
            };
        }
        Motion::ClimbCeiling => {
            let (left, _, right, bottom) = surface_bounds();
            return (left, 0, right, bottom);
        }
        Motion::Parachute
        | Motion::Hand
        | Motion::HandRelease
        | Motion::ThrowSnow
        | Motion::Special(_) => {
            return surface_bounds();
        }
        _ => {}
    }
    let dynamic_pose = matches!(
        scene.motion,
        Motion::Drag | Motion::Fall | Motion::Down | Motion::Sculpt(_)
    ) || !scene.pose.is_empty()
        || !scene.stuck_balls.is_empty();
    if dynamic_pose {
        let bounds = surface_bounds();
        if scene.motion == Motion::Drag {
            bounds
        } else {
            // The transformed ink is already aligned to the floor contact line.
            (bounds.0, bounds.1, bounds.2, logical_h as i32)
        }
    } else {
        (0, 0, logical_w as i32, logical_h as i32)
    }
}

fn part_bounds(parts: &[Part], width: i32, height: i32, pad: i32) -> (i32, i32, i32, i32) {
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
    (
        left.max(-pad),
        top.max(-pad),
        right.min(width + pad),
        bottom.min(height + pad),
    )
}

fn pet_parts(scene: &Scene, pack: &Pack) -> (Vec<Part>, Option<Point>) {
    let anim = scene.motion.animation(pack);
    let face = if scene.sad {
        Face::Neutral
    } else {
        match scene.motion {
            Motion::Fall
            | Motion::Drag
            | Motion::SlideWall
            | Motion::Special(Special::RunCover | Special::Captive | Special::RampTumble) => {
                Face::Shock
            }
            _ => scene.blink.map_or(Face::Neutral, Face::Blink),
        }
    };
    let degrees = (!scene.pose.is_empty()).then_some(scene.pose.as_slice());
    let cycle = if scene.motion == Motion::Rise {
        0.0
    } else {
        scene.cycle
    };
    let mut pose = pack.pose_for(anim, cycle, degrees, face);
    if plants_feet(scene.motion) {
        plant_standing_feet(pack, &mut pose);
    }
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
    let width = scene_size(scene.scales).0 as f32;
    match scene.motion {
        Motion::ClimbWall | Motion::SlideWall => {
            let as_y = Transform::from_row(0.0, 1.0, 1.0, 0.0, 0.0, 0.0);
            if let Some(contact) = pose_contact(pack, &pose, Motion::Fall, view.post_concat(as_y)) {
                // Subpixel overlap keeps the antialiased edge against the wall.
                view = view.post_translate(width - contact + 0.5, 0.0);
            }
        }
        Motion::ClimbCeiling => {
            let flip = Transform::from_scale(1.0, -1.0);
            if let Some(contact) = pose_contact(pack, &pose, Motion::Fall, view.post_concat(flip)) {
                view = view.post_translate(0.0, contact - 0.5);
            }
        }
        _ => {
            if let Some(contact) = pose_contact(pack, &pose, support_motion, view) {
                view = view.post_translate(0.0, ground - contact);
            }
        }
    }
    let mirror = if scene.facing_left {
        Transform::from_row(-1.0, 0.0, 0.0, 1.0, width, 0.0)
    } else {
        Transform::identity()
    };
    let wall_grips = (scene.motion == Motion::ClimbWall)
        .then(|| wall_grips(pack, &pose, view, width, scene.cycle, scale_x));
    let view = view.post_concat(mirror);
    let mut order: Vec<usize> = (0..pack.bones.len()).collect();
    order.sort_by_key(|&index| {
        let bone = &pack.bones[index];
        let layer = match scene.motion {
            Motion::Sit
                if bone.name.ends_with("hand")
                    && (!is_builtin(&pack.id) || bone.name == "left-hand") =>
            {
                2
            }
            Motion::Sit if !is_builtin(&pack.id) && leg(&bone.name) => 1,
            _ => 0,
        };
        (layer, bone.z)
    });
    let mut parts: Vec<_> = order
        .into_iter()
        .filter_map(|index| {
            let (sprite, mut local) = posed_sprite(pack, &pose, index)?;
            let bone = &pack.bones[index];
            if scene.motion == Motion::Talk
                && bone.name == "mouth"
                && pack.sprite("mouth-talk").is_none()
            {
                let time = cycle.rem_euclid(0.88);
                if (0.08..0.24).contains(&time)
                    || (0.34..0.49).contains(&time)
                    || (0.58..0.75).contains(&time)
                {
                    local = Transform::from_scale(1.0, 1.85)
                        .post_translate(0.0, -(sprite.image.height() as f32) * 0.19)
                        .post_concat(local);
                }
            }
            let mut transform = local.post_concat(view);
            if let Some(grips) = &wall_grips {
                if let Some((_, stretch, pivot)) = grips
                    .iter()
                    .flatten()
                    .find(|(root, _, _)| index == *root || bone.parent == Some(*root))
                {
                    let pivot = if scene.facing_left {
                        width - pivot
                    } else {
                        *pivot
                    };
                    transform = transform
                        .post_scale(*stretch, 1.0)
                        .post_translate(pivot * (1.0 - stretch), 0.0);
                }
            }
            let picture = if scene.sad && bone.name == "mouth" {
                let frown = crate::props::frown();
                let size =
                    (sprite.image.width() as f32 * 0.9 / frown.image.width() as f32).max(0.12);
                transform = Transform::from_scale(size, size)
                    .post_translate(sprite.image.width() as f32 * 0.05, -2.0)
                    .post_concat(transform);
                frown
            } else {
                sprite
            };
            Some(Part {
                sprite: picture.clone(),
                transform,
                joint: Some(BonePoint {
                    bone: index,
                    local: Point::from_xy(sprite.x - bone.pivot_x, sprite.y - bone.pivot_y),
                }),
            })
        })
        .collect();
    if scene.grounded > 0.0 || plants_feet(scene.motion) {
        align_floor_contacts(&mut parts, pack, ground, scene);
    }
    if scene.sad {
        if let Some(eyes) = parts.iter().find(|part| {
            part.joint
                .is_some_and(|joint| pack.bones[joint.bone].name == "eyes")
        }) {
            let width = eyes.sprite.image.width() as f32;
            let height = eyes.sprite.image.height() as f32;
            let view = eyes.transform;
            let tear = crate::props::tear();
            let size = (width * 0.0044).clamp(0.48, 1.2);
            for (x, y) in [(0.18, 0.78), (0.77, 0.92)] {
                parts.push(Part {
                    sprite: tear.clone(),
                    transform: Transform::from_scale(size, size)
                        .post_translate(x * width, y * height)
                        .post_concat(view),
                    joint: None,
                });
            }
        }
    }
    if scene.motion == Motion::Sit {
        parts.extend(laptop_part(ground, scene.scales, mirror, pack));
    }
    if scene.effect_amount > 0.0 {
        add_prop(scene, &mut parts, centre_x);
    }
    if let Motion::Special(action) = scene.motion {
        add_special(scene, action, &mut parts, pack);
    }
    let throwing_coal = scene.motion == Motion::Special(Special::ThrowCoal);
    if (scene.motion == Motion::PackSnow && scene.cycle > 0.25)
        || ((scene.motion == Motion::ThrowSnow || throwing_coal)
            && scene.cycle < crate::interactions::RELEASE_AT)
    {
        if let Some(centre) = snowball_centre(&parts, pack) {
            let (sprite, logical_size) = if throwing_coal {
                (crate::props::coal(), 42.0)
            } else {
                (crate::props::snowball(), 56.0)
            };
            let sprite = sprite.clone();
            let size = (logical_size * scale_x.min(scale_y)).max(8.0);
            let height = size * sprite.image.height() as f32 / sprite.image.width() as f32;
            let scale = size / sprite.image.width() as f32;
            parts.push(Part {
                sprite,
                transform: Transform::from_scale(scale, scale)
                    .post_translate(centre.x - size * 0.5, centre.y - height * 0.5),
                joint: None,
            });
        }
    }
    for anchor in &scene.stuck_balls {
        if let Some((part, joint)) = parts.iter().find_map(|part| {
            let joint = part.joint.filter(|joint| joint.bone == anchor.bone)?;
            Some((part, joint))
        }) {
            let mut centre = Point::from_xy(
                anchor.local.x - joint.local.x,
                anchor.local.y - joint.local.y,
            );
            part.transform.map_point(&mut centre);
            let sprite = crate::props::snowball_splat().clone();
            let width = sprite.image.width() as f32;
            let height = sprite.image.height() as f32;
            let mut axis = Point::from_xy(1.0, 0.0);
            let mut origin = Point::from_xy(0.0, 0.0);
            part.transform.map_point(&mut axis);
            part.transform.map_point(&mut origin);
            let turn = (axis.y - origin.y).atan2(axis.x - origin.x).to_degrees();
            parts.push(Part {
                sprite,
                transform: Transform::from_translate(-width * 0.5, -height * 0.5)
                    .post_rotate(turn)
                    .post_translate(centre.x, centre.y),
                joint: None,
            });
        }
    }
    let anchor = scene.anchor.map(|anchor| {
        let mut position = anchor.in_pose(&pose);
        view.map_point(&mut position);
        position
    });
    (parts, anchor)
}

pub fn held_snowball(scene: &Scene, pack: &Pack) -> Option<Point> {
    let (parts, _) = pet_parts(scene, pack);
    snowball_centre(&parts, pack)
}

fn snowball_centre(parts: &[Part], pack: &Pack) -> Option<Point> {
    let hand = parts.iter().find(|part| {
        part.joint
            .is_some_and(|joint| pack.bones[joint.bone].name == "left-hand")
    })?;
    let mut centre = Point::from_xy(
        hand.sprite.image.width() as f32 * 0.5,
        hand.sprite.image.height() as f32 * 0.9,
    );
    hand.transform.map_point(&mut centre);
    Some(centre)
}

fn floor_group(pack: &Pack, mut index: usize) -> usize {
    loop {
        let bone = &pack.bones[index];
        match bone.name.as_str() {
            "head" => return 1,
            "left-hand" => return 2,
            "right-hand" => return 3,
            "left-leg" => return 4,
            "right-leg" => return 5,
            _ => {}
        }
        let Some(parent) = bone.parent else { return 0 };
        index = parent;
    }
}

fn plants_feet(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::Sculpt(_)
            | Motion::Special(Special::Plant | Special::Water | Special::OpenGift | Special::Hide)
    )
}

fn plant_standing_feet(pack: &Pack, pose: &mut Pose) {
    let hip = |name| pack.bones.iter().position(|bone| bone.name == name);
    let (Some(left), Some(right)) = (hip("left-leg"), hip("right-leg")) else {
        return;
    };
    let standing = pack.pose("idle", 0.0);
    let delta_x = (standing.placed[left].x + standing.placed[right].x
        - pose.placed[left].x
        - pose.placed[right].x)
        * 0.5;
    let delta_y = (standing.placed[left].y + standing.placed[right].y
        - pose.placed[left].y
        - pose.placed[right].y)
        * 0.5;
    // Bend the torso around the hips while the leg chains stay in the standing pose.
    for (index, part) in pose.placed.iter_mut().enumerate() {
        if floor_group(pack, index) >= 4 {
            *part = standing.placed[index];
        } else {
            part.x += delta_x;
            part.y += delta_y;
        }
    }
    pose.bob = 0.0;
}

fn align_floor_contacts(parts: &mut [Part], pack: &Pack, floor: f32, scene: &Scene) {
    let standing = plants_feet(scene.motion);
    let amount = if standing { 1.0 } else { scene.grounded };
    let mut contacts = [f32::NEG_INFINITY; 6];
    for part in parts.iter() {
        if let Some(joint) = part.joint {
            let group = floor_group(pack, joint.bone);
            if let Some(edge) = part.sprite.lower_edge(part.transform) {
                contacts[group] = contacts[group].max(edge);
            }
        }
    }
    for part in parts {
        if let Some(joint) = part.joint {
            let group = floor_group(pack, joint.bone);
            // Ground the lying legs together so their spacing follows the skeleton.
            let edge = if !standing && group >= 4 {
                contacts[4].max(contacts[5])
            } else {
                contacts[group]
            };
            if edge.is_finite() && (!standing || group >= 4) {
                let offset = (floor - edge).clamp(0.0, floor * 0.24) * amount;
                part.transform = part.transform.post_translate(0.0, offset);
            }
        }
    }
}

fn wall_grips(
    pack: &Pack,
    pose: &Pose,
    view: Transform,
    wall: f32,
    time: f32,
    scale: f32,
) -> [Option<(usize, f32, f32)>; 4] {
    let stroke = time / WALL_STROKE;
    let left_leads = (stroke.floor() as u32).is_multiple_of(2);
    let reach = (stroke.fract() / 0.22).clamp(0.0, 1.0);
    let contact = reach * reach * (3.0 - 2.0 * reach);
    let as_y = Transform::from_row(0.0, 1.0, 1.0, 0.0, 0.0, 0.0);
    std::array::from_fn(|i| {
        let (root, tip) = [
            ("left-hand", "left-hand"),
            ("right-hand", "right-hand"),
            ("left-leg", "left-foot"),
            ("right-leg", "right-foot"),
        ][i];
        let limb = pack.bones.iter().position(|bone| bone.name == root)?;
        let (sprite, local) = pack
            .bones
            .iter()
            .position(|bone| bone.name == tip)
            .and_then(|tip| posed_sprite(pack, pose, tip))
            .or_else(|| posed_sprite(pack, pose, limb))?;
        let contact_x = sprite.lower_edge(local.post_concat(view).post_concat(as_y))?;
        let mut pivot = Point::from_xy(pose.placed[limb].x, pose.placed[limb].y);
        view.map_point(&mut pivot);
        let lead = matches!(i, 0 | 3) == left_leads;
        let gap = if lead {
            26.0 * (1.0 - contact)
        } else {
            26.0 * contact
        };
        let end = wall + 0.5 - gap * scale;
        let reach = contact_x - pivot.x;
        if reach <= 2.0 * scale {
            return None;
        }
        let stretch = ((end - pivot.x) / reach).clamp(0.4, 2.7);
        Some((limb, stretch, pivot.x))
    })
}

fn add_prop(scene: &Scene, parts: &mut Vec<Part>, centre_x: f32) {
    let top = parts
        .iter()
        .filter_map(|part| {
            part.sprite
                .lower_edge(part.transform.post_scale(1.0, -1.0))
                .map(|edge| -edge)
        })
        .min_by(f32::total_cmp)
        .unwrap_or(0.0);
    let (sprite, scale_x, scale_y, contact, behind) = if scene.motion == Motion::Parachute {
        let open = 0.25 + scene.effect_amount * 0.75;
        (
            crate::props::parachute(),
            scene.scales.0 * 1.24 * open,
            scene.scales.1 * 0.87 * open,
            top + 26.0 * scene.scales.1,
            true,
        )
    } else {
        (
            crate::props::hand(),
            scene.scales.0 * 0.9,
            scene.scales.1,
            top + 36.0 * scene.scales.1 - (1.0 - scene.effect_amount) * 190.0 * scene.scales.1,
            false,
        )
    };
    let x = centre_x - sprite.image.width() as f32 * scale_x * 0.5;
    let y = contact - sprite.image.height() as f32 * scale_y;
    let prop = Part {
        sprite: sprite.clone(),
        transform: Transform::from_scale(scale_x, scale_y).post_translate(x, y),
        joint: None,
    };
    if behind {
        parts.insert(0, prop);
    } else {
        parts.push(prop);
    }
}

fn add_special(scene: &Scene, action: Special, parts: &mut Vec<Part>, pack: &Pack) {
    let hand = snowball_centre(parts, pack);
    let direction = if scene.facing_left { -1.0 } else { 1.0 };
    if let Some(grip) = hand.filter(|_| action == Special::Parry) {
        let bat = crate::props::baseball_bat();
        let scale = scene.scales.0 * 137.0 / bat.image.width() as f32;
        let phase = scene.cycle.rem_euclid(0.55);
        let swing = ((phase - 0.12) / 0.22).clamp(0.0, 1.0);
        let angle = -106.0 + 145.0 * swing;
        parts.push(Part {
            sprite: bat.clone(),
            transform: Transform::from_translate(
                -(bat.image.width() as f32) * 0.12,
                -(bat.image.height() as f32) * 0.5,
            )
            .post_scale(scale * direction, scale)
            .post_rotate(angle * direction)
            .post_translate(grip.x, grip.y),
            joint: None,
        });
    }
    if matches!(
        action,
        Special::Decorate | Special::TinselTree | Special::StarTree
    ) {
        if let Some(centre) = hand {
            let sprite = crate::props::hand_ornament(action);
            let size = scene.scales.0 * 41.0;
            let scale = size / sprite.image.width() as f32;
            parts.push(Part {
                sprite: sprite.clone(),
                transform: Transform::from_scale(scale, scale).post_translate(
                    centre.x + direction * size * 0.26 - size * 0.5,
                    centre.y - sprite.image.height() as f32 * scale * 0.72,
                ),
                joint: None,
            });
        }
    }
    if matches!(action, Special::Water | Special::Cannon) {
        let Some(mut centre) = hand else { return };
        let (sprite, size, angle) = if action == Special::Water {
            (
                crate::props::watering_can(),
                86.0,
                26.0 * (scene.cycle * 5.0).sin().max(0.0),
            )
        } else {
            (crate::props::snow_cannon(), 128.0, -11.0)
        };
        let size = size * scene.scales.0;
        centre.x += direction * size * 0.20;
        let scale = size / sprite.image.width() as f32;
        parts.push(Part {
            sprite: sprite.clone(),
            transform: Transform::from_translate(
                -(sprite.image.width() as f32) * 0.5,
                -(sprite.image.height() as f32) * 0.5,
            )
            .post_scale(scale * direction, scale)
            .post_rotate(angle * direction)
            .post_translate(centre.x, centre.y),
            joint: None,
        });
    }
    if action != Special::EatCandy || scene.cycle >= 1.72 {
        return;
    }
    let bone_named = |part: &Part, name| {
        part.joint
            .is_some_and(|joint| pack.bones[joint.bone].name == name)
    };
    let Some(head) = parts.iter().find(|part| bone_named(part, "head")) else {
        return;
    };
    let head_width = head.sprite.image.width() as f32 * head.transform.get_scale().0;
    let Some(mouth) = parts.iter_mut().find(|part| bone_named(part, "mouth")) else {
        return;
    };
    let mut centre = Point::from_xy(
        mouth.sprite.image.width() as f32 * 0.5,
        mouth.sprite.image.height() as f32 * 0.5,
    );
    mouth.transform.map_point(&mut centre);
    let opening = ((scene.cycle - 0.35) / 0.36).clamp(0.0, 1.0);
    if opening > 0.0 {
        let sprite = crate::props::open_mouth();
        let scale = head_width * (0.14 + 0.43 * opening) / sprite.image.width() as f32;
        mouth.sprite = sprite.clone();
        mouth.transform = Transform::from_scale(scale, scale).post_translate(
            centre.x - sprite.image.width() as f32 * scale * 0.5,
            centre.y - sprite.image.height() as f32 * scale * 0.37,
        );
        mouth.joint = None;
    }
    let raise = ((scene.cycle - 0.35) / 0.82).clamp(0.0, 1.0);
    let bite = ((scene.cycle - 1.17) / 0.55).clamp(0.0, 1.0);
    let start = hand.unwrap_or(Point::from_xy(centre.x + direction * 40.0, centre.y + 30.0));
    let destination = Point::from_xy(centre.x + direction * head_width * 0.5, centre.y);
    let x = start.x + (destination.x - start.x) * raise + (centre.x - destination.x) * bite;
    let y = start.y + (destination.y - start.y) * raise;
    let sprite = crate::props::candy();
    let size = head_width * 1.22 * (1.0 - bite);
    let scale = size / sprite.image.width() as f32;
    parts.push(Part {
        sprite: sprite.clone(),
        transform: Transform::from_scale(scale, scale).post_translate(
            x - size * 0.5,
            y - sprite.image.height() as f32 * scale * 0.5,
        ),
        joint: None,
    });
}

fn leg(name: &str) -> bool {
    name.ends_with("leg") || name.ends_with("foot")
}

fn pose_contact(pack: &Pack, pose: &Pose, motion: Motion, view: Transform) -> Option<f32> {
    let supports = |bone: &str| match motion {
        Motion::Sit
        | Motion::Special(Special::RideSled | Special::RampSlide | Special::WallRecover) => {
            bone == "torso" || leg(bone)
        }
        Motion::Special(Special::RampTumble) => true,
        Motion::Idle
        | Motion::Walk
        | Motion::HandRelease
        | Motion::Talk
        | Motion::Listen
        | Motion::PackSnow
        | Motion::ThrowSnow
        | Motion::Sculpt(_)
        | Motion::Special(_) => leg(bone),
        Motion::Down
        | Motion::Rise
        | Motion::Fall
        | Motion::ClimbWall
        | Motion::ClimbCeiling
        | Motion::SlideWall
        | Motion::Parachute
        | Motion::Hand => true,
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

pub(crate) fn rgba_to_bgra(data: &[u8]) -> Vec<u8> {
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
            blink: None,
            sad: false,
            grounded: 0.0,
            motion: Motion::Idle,
            tilt: 0.0,
            facing_left: false,
            scales: (1.0, 1.0),
            pose: Vec::new(),
            anchor: None,
            stuck_balls: Vec::new(),
            effect_amount: 0.0,
        }
    }

    #[test]
    fn hit_regions_leave_gaps_and_use_logical_coordinates() {
        let pack = Pack::builtin();
        let mut frame = render(&still(), &pack, 1);
        frame.width = 8;
        frame.height = 4;
        frame.bgra = vec![0; 8 * 4 * 4];
        for y in 0..4 {
            for x in [2, 3, 6, 7] {
                frame.bgra[(y * 8 + x) * 4 + 3] = 255;
            }
        }
        for scale in [1, 2] {
            let rects = frame.input_rects(scale);
            assert_eq!(rects.len(), 2);
            assert_eq!(
                rects[0],
                IntRect::from_xywh(2 / scale as i32, 0, 2 / scale, 4 / scale).unwrap()
            );
            assert_eq!(rects[1].x(), 6 / scale as i32);
        }
    }

    #[test]
    fn climbing_and_the_ceiling_exits_draw_visible_art() {
        let pack = Pack::builtin();
        for motion in [
            Motion::ClimbWall,
            Motion::ClimbCeiling,
            Motion::Parachute,
            Motion::Hand,
        ] {
            let mut scene = still();
            scene.motion = motion;
            scene.tilt = match motion {
                Motion::ClimbWall => -0.19,
                Motion::ClimbCeiling => std::f32::consts::PI,
                _ => 0.0,
            };
            scene.effect_amount = f32::from(matches!(motion, Motion::Parachute | Motion::Hand));
            let frame = render(&scene, &pack, 1);
            assert!(frame
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] > 0));
            match motion {
                Motion::ClimbWall => {
                    assert!(frame
                        .bgra
                        .chunks_exact(frame.width as usize * 4)
                        .any(|row| *row.last().unwrap() > 0));
                    scene.facing_left = true;
                    let frame = render(&scene, &pack, 1);
                    assert!(frame
                        .bgra
                        .chunks_exact(frame.width as usize * 4)
                        .any(|row| row[3] > 0));
                }
                Motion::ClimbCeiling => {
                    assert!(frame.bgra[..frame.width as usize * 4]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|pixel| pixel[3] > 0));
                }
                Motion::Parachute | Motion::Hand => {
                    assert_eq!(
                        frame
                            .parts
                            .iter()
                            .filter(|part| part.joint.is_none())
                            .count(),
                        1
                    );
                    assert!(frame.shift.1 < 0);
                }
                _ => unreachable!(),
            }
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
                let (left, top, right, _) =
                    part_bounds(&frame.parts, width, height, width.max(height));
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
    fn both_legs_are_visible_when_lying_on_either_side() {
        use crate::rag::Lying;

        for pack in crate::pack::installed().into_iter().take(2) {
            let mut scene = still();
            scene.motion = Motion::Down;
            scene.grounded = 1.0;
            scene.scales = (0.5, 0.5);
            for side in [Lying::Front, Lying::Back] {
                scene.tilt = side.tilt();
                scene.pose = pack.rise_degrees(side.rise_animation(), 0.0);
                for facing_left in [false, true] {
                    scene.facing_left = facing_left;
                    let frame = render(&scene, &pack, 1);
                    let mut visible = [0; 6];
                    for y in 0..frame.height {
                        for x in 0..frame.width {
                            if let Some(joint) = pick(&frame.parts, x as f32 + 0.5, y as f32 + 0.5)
                            {
                                visible[floor_group(&pack, joint.bone)] += 1;
                            }
                        }
                    }
                    assert!(
                        visible[4] > 35 && visible[5] > 35,
                        "{} {side:?} with facing_left={facing_left}: {visible:?}",
                        pack.id
                    );
                    let mut ungrounded = scene.clone();
                    ungrounded.grounded = 0.0;
                    let legs = |parts: &[Part]| {
                        [4, 5].map(|group| {
                            let transform = parts
                                .iter()
                                .find(|part| floor_group(&pack, part.joint.unwrap().bone) == group)
                                .unwrap()
                                .transform;
                            (transform.tx, transform.ty)
                        })
                    };
                    let [left, right] = legs(&frame.parts);
                    let [raw_left, raw_right] = legs(&pet_parts(&ungrounded, &pack).0);
                    assert!(((left.0 - right.0) - (raw_left.0 - raw_right.0)).abs() < 0.01);
                    assert!(((left.1 - right.1) - (raw_left.1 - raw_right.1)).abs() < 0.01);
                }
            }
        }
    }

    #[test]
    fn sculpture_changes_hand_height_with_both_feet_planted() {
        for pack in crate::pack::installed().into_iter().take(2) {
            let mut scene = still();
            let floor = scene_size(scene.scales).1 as f32;
            let mut standing_feet = None;
            let mut hand_edges = Vec::new();
            for stage in [SculptStage::Base, SculptStage::Middle, SculptStage::Top] {
                scene.motion = Motion::Sculpt(stage);
                let mut lowest_hand = f32::NEG_INFINITY;
                for phase in [0.0, 0.22, 0.44, 0.7] {
                    scene.cycle = phase;
                    let (parts, _) = pet_parts(&scene, &pack);
                    let mut contacts = [f32::NEG_INFINITY; 6];
                    let mut feet = Vec::new();
                    for part in &parts {
                        let group = floor_group(&pack, part.joint.unwrap().bone);
                        let Some(edge) = part.sprite.lower_edge(part.transform) else {
                            continue;
                        };
                        contacts[group] = contacts[group].max(edge);
                        if group >= 4 {
                            feet.push(part.transform.tx);
                        }
                    }
                    for edge in &contacts[4..] {
                        assert!((edge - floor).abs() < 0.01, "{} {stage:?}", pack.id);
                    }
                    assert_eq!(&feet, standing_feet.get_or_insert_with(|| feet.clone()));
                    lowest_hand = lowest_hand.max(contacts[2].max(contacts[3]));
                }
                hand_edges.push(lowest_hand);
            }
            assert!(
                hand_edges.windows(2).all(|edges| edges[0] > edges[1] + 5.0),
                "{} {hand_edges:?}",
                pack.id
            );
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
