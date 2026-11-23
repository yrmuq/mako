// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Bear girl, and any other loaded chibi, posed from separate parts on one skeleton.

use ab_glyph::{point, Font, FontVec, Glyph, PxScale, ScaleFont};
use std::sync::OnceLock;
use tiny_skia::{
    ColorU8, FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Point, PremultipliedColorU8,
    Transform,
};

use crate::i18n::{
    Catalog, ID_CHIBI, ID_DARK, ID_ENGLISH, ID_LANGUAGE, ID_LIGHT, ID_NAME, ID_QUIT, ID_RUSSIAN,
    ID_SIZE, ID_SYSTEM, ID_THEME,
};
use crate::pack::{BonePoint, Choice, Pack, Pose, Sprite, BUILTIN_ID};
use crate::sim::{FEET_X, FEET_Y, SPRITE_H, SPRITE_W};

pub const MENU_W: f32 = 228.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walk,
    Fall,
    Drag,
    Down,
    Sit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    Name,
    ChibiLabel,
    Chibi,
    Language,
    Theme,
    SystemLang,
    English,
    Russian,
    SystemTheme,
    Light,
    Dark,
    Size,
    Scale,
    Quit,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: RowId,
    pub text: String,
    pub selected: bool,
    pub label: bool,
    pub key: String,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub id: RowId,
    pub key: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub cycle: f32,
    pub motion: Motion,
    pub tilt: f32,
    pub facing_left: bool,
    pub menu: bool,
    pub menu_left: bool,
    pub dark: bool,
    pub pet_scale: f32,
    pub pose: Vec<f32>,
    pub anchor: Option<BonePoint>,
    pub hold_sitting: bool,
    pub rows: Vec<Row>,
}

pub struct Frame {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub hits: Vec<Hit>,
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
        if pixel.x < 0.0 || pixel.y < 0.0 {
            continue;
        }
        if part
            .sprite
            .image
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

pub fn scene_size(menu: bool, pet_scale: f32) -> (u32, u32) {
    let scale = pet_scale.clamp(0.2, 1.0);
    let pet_w = SPRITE_W * scale;
    let pet_h = SPRITE_H * scale;
    let width = if menu { pet_w + MENU_W } else { pet_w };
    let height = if menu { pet_h.max(SPRITE_H) } else { pet_h };
    (width.round() as u32, height.round() as u32)
}

pub fn render(scene: &Scene, pack: &Pack, scale: u32) -> Frame {
    let scale = scale.max(1);
    let (logical_w, logical_h) = scene_size(scene.menu, scene.pet_scale);
    let pet_x = if scene.menu && scene.menu_left {
        MENU_W
    } else {
        0.0
    };
    let (mut parts, mut anchor) = pet_parts(pet_x, scene, pack);
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
    let hits = if scene.menu {
        draw_menu(&mut pixmap, scene, scale, pet_x, (left, top))
    } else {
        Vec::new()
    };
    Frame {
        bgra: rgba_to_bgra(pixmap.data()),
        width,
        height,
        hits,
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

pub fn rows(
    catalog: &Catalog,
    title: &str,
    lang_sel: &str,
    theme_sel: &str,
    size_sel: &str,
    chibis: &[Choice],
    selected: &str,
) -> Vec<Row> {
    let row = |id, text: String, selected, label, key: &str| Row {
        id,
        text,
        selected,
        label,
        key: key.to_string(),
    };
    let mut rows = vec![
        row(RowId::Name, title.to_string(), false, true, ""),
        row(
            RowId::ChibiLabel,
            catalog.text(ID_CHIBI).to_string(),
            false,
            true,
            "",
        ),
    ];
    for chibi in chibis {
        let name = if chibi.id == BUILTIN_ID {
            catalog.text(ID_NAME).to_string()
        } else {
            clip_name(&chibi.name)
        };
        rows.push(row(
            RowId::Chibi,
            name,
            chibi.id == selected,
            false,
            &chibi.id,
        ));
    }
    rows.extend([
        row(
            RowId::Language,
            catalog.text(ID_LANGUAGE).to_string(),
            false,
            true,
            "",
        ),
        row(
            RowId::SystemLang,
            catalog.text(ID_SYSTEM).to_string(),
            lang_sel == "system",
            false,
            "",
        ),
        row(
            RowId::English,
            catalog.text(ID_ENGLISH).to_string(),
            lang_sel == "en",
            false,
            "",
        ),
        row(
            RowId::Russian,
            catalog.text(ID_RUSSIAN).to_string(),
            lang_sel == "ru",
            false,
            "",
        ),
        row(
            RowId::Theme,
            catalog.text(ID_THEME).to_string(),
            false,
            true,
            "",
        ),
        row(
            RowId::SystemTheme,
            catalog.text(ID_SYSTEM).to_string(),
            theme_sel == "system",
            false,
            "",
        ),
        row(
            RowId::Light,
            catalog.text(ID_LIGHT).to_string(),
            theme_sel == "light",
            false,
            "",
        ),
        row(
            RowId::Dark,
            catalog.text(ID_DARK).to_string(),
            theme_sel == "dark",
            false,
            "",
        ),
        row(
            RowId::Size,
            catalog.text(ID_SIZE).to_string(),
            false,
            true,
            "",
        ),
        row(RowId::Scale, "1×".to_string(), size_sel == "1", false, "1"),
        row(
            RowId::Scale,
            "0.8×".to_string(),
            size_sel == "0.8",
            false,
            "0.8",
        ),
        row(
            RowId::Scale,
            "0.6×".to_string(),
            size_sel == "0.6",
            false,
            "0.6",
        ),
        row(
            RowId::Scale,
            "0.4×".to_string(),
            size_sel == "0.4",
            false,
            "0.4",
        ),
        row(
            RowId::Scale,
            "0.2×".to_string(),
            size_sel == "0.2",
            false,
            "0.2",
        ),
        row(
            RowId::Quit,
            catalog.text(ID_QUIT).to_string(),
            false,
            false,
            "",
        ),
    ]);
    rows
}

fn clip_name(text: &str) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index == 18 {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

fn pet_parts(origin_x: f32, scene: &Scene, pack: &Pack) -> (Vec<Part>, Option<Point>) {
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
        pack.pose_with(
            if scene.hold_sitting { "sit" } else { "idle" },
            scene.cycle,
            Some(&scene.pose),
        )
    };
    let pet_scale = scene.pet_scale.clamp(0.2, 1.0);
    let fit = (SPRITE_W / pack.canvas_w).min(SPRITE_H / pack.canvas_h) * pet_scale;
    let pet_h = SPRITE_H * pet_scale;
    let pet_top = if scene.menu {
        (SPRITE_H - pet_h).max(0.0)
    } else {
        0.0
    };
    let ground = pet_top + FEET_Y * pet_scale;
    let bob = pose.bob * fit;
    let mut view = Transform::from_scale(fit, fit)
        .post_translate(
            origin_x + FEET_X * pet_scale - pack.feet_x * fit,
            ground - pack.feet_y * fit + bob,
        )
        .post_rotate_at(
            scene.tilt.to_degrees(),
            origin_x + FEET_X * pet_scale,
            ground + bob,
        );
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
        let width = scene_size(false, pet_scale).0 as f32;
        Transform::from_row(-1.0, 0.0, 0.0, 1.0, 2.0 * origin_x + width, 0.0)
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
        parts.extend(laptop_part(origin_x, ground, pet_scale, mirror, pack));
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
    origin_x: f32,
    ground: f32,
    pet_scale: f32,
    mirror: Transform,
    pack: &Pack,
) -> Option<Part> {
    let sprite = pack.sprite("it-1")?;
    if sprite.is_blank() {
        return None;
    }
    let sw = sprite.image.width() as f32;
    let sh = sprite.image.height() as f32;
    let dw = 86.0 * pet_scale;
    let dh = dw * sh / sw;
    let x = origin_x + (FEET_X + 50.0) * pet_scale - dw * 0.5;
    let y = ground - dh;
    Some(Part {
        sprite: sprite.clone(),
        transform: Transform::from_scale(dw / sw, dh / sh)
            .post_translate(x, y)
            .post_concat(mirror),
        joint: None,
    })
}

fn draw_menu(
    pixmap: &mut Pixmap,
    scene: &Scene,
    scale: u32,
    pet_x: f32,
    shift: (i32, i32),
) -> Vec<Hit> {
    let s = scale as f32;
    let n = scene.rows.len() as f32;
    let top = 6.0 - shift.1 as f32;
    let row_h = ((SPRITE_H - 12.0) / n).min(24.0);
    let x = if scene.menu_left {
        4.0
    } else {
        pet_x + SPRITE_W * scene.pet_scale.clamp(0.2, 1.0) + 4.0
    } - shift.0 as f32;
    let w = MENU_W - 8.0;
    let h = row_h * n;
    let panel = if scene.dark {
        rgba(42, 36, 34, 235)
    } else {
        rgba(255, 248, 240, 238)
    };
    let ink = if scene.dark {
        rgba(248, 240, 232, 255)
    } else {
        rgba(58, 40, 32, 255)
    };
    let muted = if scene.dark {
        rgba(196, 176, 164, 255)
    } else {
        rgba(110, 84, 72, 255)
    };
    let mark = rgba(176, 112, 72, 255);
    fill_rect(pixmap, x, top, w, h, 10.0, panel, s);

    let mut hits = Vec::new();
    let font = font();
    let font_px = (row_h * 0.58).clamp(9.0, 15.0);
    for (index, row) in scene.rows.iter().enumerate() {
        let ry = top + index as f32 * row_h;
        let color = if row.selected {
            mark
        } else if row.label {
            muted
        } else {
            ink
        };
        let prefix = if row.selected { "● " } else { "" };
        let text = format!("{prefix}{}", row.text);
        draw_text(
            pixmap,
            font,
            x + 12.0,
            ry + row_h * 0.72,
            font_px * s,
            &text,
            color,
            s,
        );
        if !row.label {
            hits.push(Hit {
                id: row.id,
                key: row.key.clone(),
                x,
                y: ry,
                w,
                h: row_h,
            });
        }
    }
    hits
}

#[allow(clippy::too_many_arguments)]
fn fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: ColorU8,
    scale: f32,
) {
    let mut builder = PathBuilder::new();
    let r = radius.min(w * 0.5).min(h * 0.5);
    builder.move_to(x + r, y);
    builder.line_to(x + w - r, y);
    builder.quad_to(x + w, y, x + w, y + r);
    builder.line_to(x + w, y + h - r);
    builder.quad_to(x + w, y + h, x + w - r, y + h);
    builder.line_to(x + r, y + h);
    builder.quad_to(x, y + h, x, y + h - r);
    builder.line_to(x, y + r);
    builder.quad_to(x, y, x + r, y);
    builder.close();
    let Some(path) = builder.finish() else {
        return;
    };
    pixmap.fill_path(
        &path,
        &paint_color(color),
        FillRule::Winding,
        Transform::from_scale(scale, scale),
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    pixmap: &mut Pixmap,
    font: &FontVec,
    x: f32,
    y: f32,
    size: f32,
    text: &str,
    color: ColorU8,
    scale: f32,
) {
    let px = PxScale::from(size);
    let mut caret = x * scale;
    let baseline = y * scale;
    for ch in text.chars() {
        let glyph = Glyph {
            id: font.glyph_id(ch),
            scale: px,
            position: point(caret, baseline),
        };
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pixmap.width() as i32 || py >= pixmap.height() as i32 {
                    return;
                }
                blend(pixmap, px as u32, py as u32, color, coverage);
            });
        }
        caret += font.as_scaled(px).h_advance(font.glyph_id(ch));
    }
}

fn blend(pixmap: &mut Pixmap, x: u32, y: u32, color: ColorU8, coverage: f32) {
    let width = pixmap.width();
    let Some(pixel) = pixmap.pixels_mut().get_mut((y * width + x) as usize) else {
        return;
    };
    let alpha = color.alpha() as f32 / 255.0 * coverage.clamp(0.0, 1.0);
    let inv = 1.0 - alpha;
    let r = color.red() as f32 * alpha + pixel.red() as f32 * inv;
    let g = color.green() as f32 * alpha + pixel.green() as f32 * inv;
    let b = color.blue() as f32 * alpha + pixel.blue() as f32 * inv;
    let a = alpha * 255.0 + pixel.alpha() as f32 * inv;
    *pixel = PremultipliedColorU8::from_rgba(
        r.round() as u8,
        g.round() as u8,
        b.round() as u8,
        a.round() as u8,
    )
    .unwrap_or(PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap());
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

fn font() -> &'static FontVec {
    static FONT: OnceLock<FontVec> = OnceLock::new();
    FONT.get_or_init(|| {
        FontVec::try_from_vec(include_bytes!("../assets/DejaVuSans.ttf").to_vec())
            .expect("MAKO-FONT-0001: The bundled menu font is invalid")
    })
}

fn paint_color(color: ColorU8) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.red(), color.green(), color.blue(), color.alpha());
    paint.anti_alias = true;
    paint
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> ColorU8 {
    ColorU8::from_rgba(r, g, b, a)
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
            menu: false,
            menu_left: false,
            dark: false,
            pet_scale: 1.0,
            pose: Vec::new(),
            anchor: None,
            hold_sitting: false,
            rows: Vec::new(),
        }
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
            scene.pet_scale = pet_scale;
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

    #[test]
    fn menu_hits_stay_inside_the_frame_and_off_the_pet() {
        let mut scene = still();
        scene.menu = true;
        scene.rows = menu_rows();
        let right = render(&scene, &Pack::builtin(), 1);
        assert_rows_fit(&right.hits, false);
        scene.menu_left = true;
        let left = render(&scene, &Pack::builtin(), 1);
        assert_rows_fit(&left.hits, true);
        std::fs::write("/tmp/mako-menu.png", encode(&scene)).expect("preview");
    }

    fn assert_rows_fit(hits: &[Hit], menu_left: bool) {
        assert!(hits.len() >= 7);
        for hit in hits {
            assert!(hit.y >= 0.0);
            assert!(hit.y + hit.h <= SPRITE_H + 0.01);
            assert!(hit.h > 8.0);
            if menu_left {
                assert!(hit.x + hit.w <= MENU_W + 0.01);
            } else {
                assert!(hit.x >= SPRITE_W - 0.01);
            }
        }
        for (index, hit) in hits.iter().enumerate() {
            for other in &hits[index + 1..] {
                let separated = hit.x + hit.w <= other.x
                    || other.x + other.w <= hit.x
                    || hit.y + hit.h <= other.y
                    || other.y + other.h <= hit.y;
                assert!(separated);
            }
        }
    }

    fn menu_rows() -> Vec<Row> {
        let row = |id, label| Row {
            id,
            text: "item".to_string(),
            selected: false,
            label,
            key: String::new(),
        };
        vec![
            row(RowId::Name, true),
            row(RowId::Language, true),
            row(RowId::SystemLang, false),
            row(RowId::English, false),
            row(RowId::Russian, false),
            row(RowId::Theme, true),
            row(RowId::SystemTheme, false),
            row(RowId::Light, false),
            row(RowId::Dark, false),
            row(RowId::Quit, false),
        ]
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
