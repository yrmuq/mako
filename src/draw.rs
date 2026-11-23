// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Bear girl, and any other loaded chibi, posed from separate parts on one skeleton.

use ab_glyph::{point, Font, FontVec, Glyph, PxScale, ScaleFont};
use std::sync::OnceLock;
use tiny_skia::{
    ColorU8, FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, PremultipliedColorU8, Transform,
};

use crate::i18n::{
    Catalog, ID_CHIBI, ID_DARK, ID_ENGLISH, ID_LANGUAGE, ID_LIGHT, ID_NAME, ID_QUIT, ID_RUSSIAN,
    ID_SIZE, ID_SYSTEM, ID_THEME,
};
use crate::pack::{Choice, Pack, Pose, BUILTIN_ID};
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
    pub sink: f32,
    pub pose: Vec<f32>,
    pub rows: Vec<Row>,
}

pub struct Frame {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub hits: Vec<Hit>,
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
    let width = logical_w * scale;
    let height = logical_h * scale;
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    let pet_x = if scene.menu && scene.menu_left {
        MENU_W
    } else {
        0.0
    };
    draw_pet(&mut pixmap, pet_x, scene, pack, scale);
    if scene.facing_left {
        mirror_sprite(&mut pixmap, pet_x, scale, scene.pet_scale);
    }
    let hits = if scene.menu {
        draw_menu(&mut pixmap, scene, scale, pet_x)
    } else {
        Vec::new()
    };
    Frame {
        bgra: rgba_to_bgra(pixmap.data()),
        width,
        height,
        hits,
    }
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

fn draw_pet(pixmap: &mut Pixmap, origin_x: f32, scene: &Scene, pack: &Pack, scale: u32) {
    if pack.canvas_w <= 0.0 || pack.canvas_h <= 0.0 {
        return;
    }
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
        pack.pose_with("idle", scene.cycle, Some(&scene.pose))
    };
    let pet_scale = scene.pet_scale.clamp(0.2, 1.0);
    let fit = (SPRITE_W / pack.canvas_w).min(SPRITE_H / pack.canvas_h) * pet_scale;
    let pet_h = SPRITE_H * pet_scale;
    let pet_top = if scene.menu {
        (SPRITE_H - pet_h).max(0.0)
    } else {
        0.0
    };
    let bob = pose.bob * fit;
    let view = Transform::from_scale(fit, fit)
        .post_translate(
            origin_x + FEET_X * pet_scale - pack.feet_x * fit,
            pet_top + FEET_Y * pet_scale - pack.feet_y * fit + bob + scene.sink,
        )
        .post_rotate_at(
            scene.tilt.to_degrees(),
            origin_x + FEET_X * pet_scale,
            pet_top + FEET_Y * pet_scale + bob + scene.sink,
        )
        .post_scale(scale as f32, scale as f32);
    let mut order: Vec<usize> = (0..pack.bones.len()).collect();
    order.sort_by_key(|&index| pack.bones[index].z);
    if scene.motion == Motion::Sit {
        // Legs stay in front of the body. The lid covers the lap, and both paws
        // stay on top of the keyboard.
        let front = |index: usize| {
            let name = pack.bones[index].name.as_str();
            name.ends_with("leg") || name.ends_with("foot")
        };
        for &index in order
            .iter()
            .filter(|&&index| !front(index) && !pack.bones[index].name.ends_with("hand"))
        {
            paint_bone(pixmap, pack, &pose, view, index);
        }
        for &index in order.iter().filter(|&&index| front(index)) {
            paint_bone(pixmap, pack, &pose, view, index);
        }
        draw_laptop(pixmap, origin_x, pet_top, pet_scale, scale, pack);
        for &index in order
            .iter()
            .filter(|&&index| pack.bones[index].name.ends_with("hand"))
        {
            paint_bone(pixmap, pack, &pose, view, index);
        }
    } else {
        for &index in &order {
            paint_bone(pixmap, pack, &pose, view, index);
        }
    }
}

fn paint_bone(pixmap: &mut Pixmap, pack: &Pack, pose: &Pose, view: Transform, index: usize) {
    let image_name = &pose.image[index];
    let Some(sprite) = pack.sprite(image_name) else {
        return;
    };
    if sprite.blank {
        return;
    }
    let placed = &pose.placed[index];
    let bone = &pack.bones[index];
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
    pixmap.draw_pixmap(
        0,
        0,
        sprite.image.as_ref().as_ref(),
        &PixmapPaint::default(),
        local.post_concat(view),
        None,
    );
}

fn draw_laptop(
    pixmap: &mut Pixmap,
    origin_x: f32,
    pet_top: f32,
    pet_scale: f32,
    scale: u32,
    pack: &Pack,
) {
    let Some(sprite) = pack.sprite("it-1") else {
        return;
    };
    if sprite.blank {
        return;
    }
    let sw = sprite.image.width() as f32;
    let sh = sprite.image.height() as f32;
    if sw < 1.0 || sh < 1.0 {
        return;
    }
    let dw = 118.0 * pet_scale;
    let dh = dw * sh / sw;
    let x = origin_x + FEET_X * pet_scale - dw * 0.5;
    let y = pet_top + FEET_Y * pet_scale - dh * 0.72;
    let px = scale as f32;
    let local = Transform::from_scale(dw / sw * px, dh / sh * px).post_translate(x * px, y * px);
    pixmap.draw_pixmap(
        0,
        0,
        sprite.image.as_ref().as_ref(),
        &PixmapPaint::default(),
        local,
        None,
    );
}

fn draw_menu(pixmap: &mut Pixmap, scene: &Scene, scale: u32, pet_x: f32) -> Vec<Hit> {
    let s = scale as f32;
    let n = scene.rows.len() as f32;
    let top = 6.0;
    let row_h = ((SPRITE_H - 12.0) / n).min(24.0);
    let x = if scene.menu_left {
        4.0
    } else {
        pet_x + SPRITE_W * scene.pet_scale.clamp(0.2, 1.0) + 4.0
    };
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

fn mirror_sprite(pixmap: &mut Pixmap, origin_x: f32, scale: u32, pet_scale: f32) {
    let x0 = (origin_x * scale as f32).round() as u32;
    let span = (SPRITE_W * pet_scale.clamp(0.2, 1.0) * scale as f32).round() as u32;
    let width = pixmap.width();
    let height = pixmap.height();
    let pixels = pixmap.pixels_mut();
    for y in 0..height {
        for x in 0..span / 2 {
            let left = (y * width + x0 + x) as usize;
            let right = (y * width + x0 + span - 1 - x) as usize;
            pixels.swap(left, right);
        }
    }
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
            sink: 0.0,
            pose: Vec::new(),
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
    fn sitting_draws_the_separate_laptop() {
        let pack = Pack::builtin();
        let idle = render(&still(), &pack, 1);
        let mut scene = still();
        scene.motion = Motion::Sit;
        let sitting = render(&scene, &pack, 1);
        let count = |frame: &Frame| {
            let (pixels, _) = frame.bgra.as_chunks::<4>();
            pixels.iter().filter(|pixel| pixel[3] > 16).count()
        };
        assert!(
            count(&sitting) > count(&idle) + 300,
            "{} {}",
            count(&sitting),
            count(&idle)
        );
        std::fs::write("/tmp/mako-sit.png", encode(&scene)).expect("preview");
        scene.motion = Motion::Down;
        scene.sink = 70.0;
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
