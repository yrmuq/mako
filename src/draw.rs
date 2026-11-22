// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Bear girl posed from one image. The hair clip is a round barrette, not a cross.
//! Limbs are pieces of that image on a skeleton, so a walk does not need a new drawing per frame.

use ab_glyph::{point, Font, FontVec, Glyph, PxScale, ScaleFont};
use std::io::Cursor;
use std::sync::OnceLock;
use tiny_skia::{
    ColorU8, FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, PremultipliedColorU8, Transform,
};

use crate::i18n::{
    Catalog, ID_DARK, ID_ENGLISH, ID_LANGUAGE, ID_LIGHT, ID_NAME, ID_QUIT, ID_RUSSIAN, ID_SYSTEM,
    ID_THEME,
};
use crate::sim::{FEET_X, FEET_Y, SPRITE_H, SPRITE_W};

pub const MENU_W: f32 = 228.0;

const TORSO: u8 = 0;
const LEFT_ARM: u8 = 1;
const RIGHT_ARM: u8 = 2;
const LEFT_LEG: u8 = 3;
const RIGHT_LEG: u8 = 4;

const LEFT_SHOULDER: (f32, f32) = (136.0, 320.0);
const RIGHT_SHOULDER: (f32, f32) = (264.0, 320.0);
const LEFT_LEG_PIVOT: (f32, f32) = (167.0, 430.0);
const RIGHT_LEG_PIVOT: (f32, f32) = (230.0, 430.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowId {
    Name,
    Language,
    Theme,
    SystemLang,
    English,
    Russian,
    SystemTheme,
    Light,
    Dark,
    Quit,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub id: RowId,
    pub text: String,
    pub selected: bool,
    pub label: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub id: RowId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub cycle: f32,
    pub walking: bool,
    pub tilt: f32,
    pub facing_left: bool,
    pub menu: bool,
    pub menu_left: bool,
    pub dark: bool,
    pub rows: Vec<Row>,
}

pub struct Frame {
    pub bgra: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub hits: Vec<Hit>,
}

pub fn scene_size(menu: bool) -> (u32, u32) {
    let width = if menu { SPRITE_W + MENU_W } else { SPRITE_W };
    (width.round() as u32, SPRITE_H.round() as u32)
}

pub fn render(scene: &Scene, scale: u32) -> Frame {
    let scale = scale.max(1);
    let (logical_w, logical_h) = scene_size(scene.menu);
    let width = logical_w * scale;
    let height = logical_h * scale;
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    let pet_x = if scene.menu && scene.menu_left {
        MENU_W
    } else {
        0.0
    };
    draw_pet(&mut pixmap, pet_x, scene, scale);
    if scene.facing_left {
        mirror_sprite(&mut pixmap, pet_x, scale);
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

pub fn rows(catalog: &Catalog, lang_sel: &str, theme_sel: &str) -> Vec<Row> {
    let row = |id, text_id, selected, label| Row {
        id,
        text: catalog.text(text_id).to_string(),
        selected,
        label,
    };
    vec![
        row(RowId::Name, ID_NAME, false, true),
        row(RowId::Language, ID_LANGUAGE, false, true),
        row(RowId::SystemLang, ID_SYSTEM, lang_sel == "system", false),
        row(RowId::English, ID_ENGLISH, lang_sel == "en", false),
        row(RowId::Russian, ID_RUSSIAN, lang_sel == "ru", false),
        row(RowId::Theme, ID_THEME, false, true),
        row(RowId::SystemTheme, ID_SYSTEM, theme_sel == "system", false),
        row(RowId::Light, ID_LIGHT, theme_sel == "light", false),
        row(RowId::Dark, ID_DARK, theme_sel == "dark", false),
        row(RowId::Quit, ID_QUIT, false, false),
    ]
}

struct Pose {
    l_leg: f32,
    r_leg: f32,
    l_arm: f32,
    r_arm: f32,
    bob: f32,
    left_front: bool,
    l_wide: f32,
    r_wide: f32,
}

fn pose(scene: &Scene) -> Pose {
    if !scene.walking {
        return Pose {
            l_leg: 0.0,
            r_leg: 0.0,
            l_arm: 0.0,
            r_arm: 0.0,
            bob: (scene.cycle * 2.0).sin() * 1.2,
            left_front: true,
            l_wide: 1.0,
            r_wide: 1.0,
        };
    }
    // Opposite limbs share one sine so the step stays one pose instead of a stack of drawings.
    let wave = (scene.cycle * 8.0).sin();
    Pose {
        l_leg: wave * 16.0,
        r_leg: -wave * 16.0,
        l_arm: -wave * 14.0,
        r_arm: wave * 14.0,
        bob: wave.abs() * -2.0,
        left_front: wave >= 0.0,
        l_wide: 1.0,
        r_wide: 1.0,
    }
}

fn draw_pet(pixmap: &mut Pixmap, origin_x: f32, scene: &Scene, scale: u32) {
    let doll = doll();
    let pose = pose(scene);
    let view = View {
        origin_x,
        bob: pose.bob,
        tilt: scene.tilt.to_degrees(),
        fit_x: SPRITE_W / doll.sprite_w as f32,
        fit_y: SPRITE_H / doll.sprite_h as f32,
        scale: scale as f32,
    };
    paint(pixmap, &doll.torso, &view, &Transform::identity());
    if pose.left_front {
        paint(
            pixmap,
            &doll.right_leg,
            &view,
            &pivot_bone(RIGHT_LEG_PIVOT, pose.r_leg, pose.r_wide),
        );
        paint(
            pixmap,
            &doll.left_arm,
            &view,
            &pivot_bone(LEFT_SHOULDER, pose.l_arm, 1.0),
        );
        paint(
            pixmap,
            &doll.right_arm,
            &view,
            &pivot_bone(RIGHT_SHOULDER, pose.r_arm, 1.0),
        );
        paint(
            pixmap,
            &doll.left_leg,
            &view,
            &pivot_bone(LEFT_LEG_PIVOT, pose.l_leg, pose.l_wide),
        );
    } else {
        paint(
            pixmap,
            &doll.left_leg,
            &view,
            &pivot_bone(LEFT_LEG_PIVOT, pose.l_leg, pose.l_wide),
        );
        paint(
            pixmap,
            &doll.right_arm,
            &view,
            &pivot_bone(RIGHT_SHOULDER, pose.r_arm, 1.0),
        );
        paint(
            pixmap,
            &doll.left_arm,
            &view,
            &pivot_bone(LEFT_SHOULDER, pose.l_arm, 1.0),
        );
        paint(
            pixmap,
            &doll.right_leg,
            &view,
            &pivot_bone(RIGHT_LEG_PIVOT, pose.r_leg, pose.r_wide),
        );
    }
}

fn pivot_bone(pivot: (f32, f32), angle: f32, wide: f32) -> Transform {
    Transform::from_translate(-pivot.0, -pivot.1)
        .post_rotate(angle)
        .post_scale(wide, 1.0)
        .post_translate(pivot.0, pivot.1)
}

struct View {
    origin_x: f32,
    bob: f32,
    tilt: f32,
    fit_x: f32,
    fit_y: f32,
    scale: f32,
}

fn paint(pixmap: &mut Pixmap, piece: &Piece, view: &View, bone: &Transform) {
    if piece.blank {
        return;
    }
    let placed = Transform::from_translate(piece.x, piece.y)
        .post_concat(*bone)
        .post_scale(view.fit_x, view.fit_y)
        .post_translate(view.origin_x, view.bob)
        .post_rotate_at(view.tilt, view.origin_x + FEET_X, FEET_Y + view.bob)
        .post_scale(view.scale, view.scale);
    pixmap.draw_pixmap(
        0,
        0,
        piece.image.as_ref(),
        &PixmapPaint::default(),
        placed,
        None,
    );
}

struct Piece {
    image: Pixmap,
    x: f32,
    y: f32,
    blank: bool,
}

struct Doll {
    sprite_w: u32,
    sprite_h: u32,
    torso: Piece,
    left_arm: Piece,
    right_arm: Piece,
    left_leg: Piece,
    right_leg: Piece,
}

fn doll() -> &'static Doll {
    static DOLL: OnceLock<Doll> = OnceLock::new();
    DOLL.get_or_init(|| {
        let sprite = decode_sprite(include_bytes!("../assets/bear-girl.png"));
        Doll {
            sprite_w: sprite.width(),
            sprite_h: sprite.height(),
            torso: extract(&sprite, TORSO),
            left_arm: extract(&sprite, LEFT_ARM),
            right_arm: extract(&sprite, RIGHT_ARM),
            left_leg: extract(&sprite, LEFT_LEG),
            right_leg: extract(&sprite, RIGHT_LEG),
        }
    })
}

fn in_piece(x: u32, y: u32, kind: u8) -> bool {
    // Sleeves and hands sit outside the torso. Legs split once the silhouette does.
    let arm_l = sleeve(x, y, true);
    let arm_r = sleeve(x, y, false);
    let outer_l = arm_l && x < 124;
    let outer_r = arm_r && x > 276;
    let lower_leg = y >= 450 && (120..280).contains(&x);
    match kind {
        TORSO => !lower_leg && !outer_l && !outer_r,
        LEFT_ARM => arm_l,
        RIGHT_ARM => arm_r,
        LEFT_LEG => y >= 426 && x < 200,
        RIGHT_LEG => y >= 426 && x >= 198,
        _ => false,
    }
}

fn sleeve(x: u32, y: u32, left: bool) -> bool {
    let y = y as f32;
    let x = x as f32;
    if !(312.0..402.0).contains(&y) {
        return false;
    }
    if left {
        x < 136.0
    } else {
        x > 264.0
    }
}

fn extract(src: &Pixmap, kind: u8) -> Piece {
    let width = src.width();
    let height = src.height();
    let pixels = src.pixels();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut any = false;
    for y in 0..height {
        for x in 0..width {
            let pixel = pixels[(y * width + x) as usize];
            if pixel.alpha() == 0 || !in_piece(x, y, kind) {
                continue;
            }
            any = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !any {
        return Piece {
            image: Pixmap::new(1, 1).expect("pixmap"),
            x: 0.0,
            y: 0.0,
            blank: true,
        };
    }
    let piece_w = max_x - min_x + 1;
    let piece_h = max_y - min_y + 1;
    let mut image = Pixmap::new(piece_w, piece_h).expect("pixmap");
    for y in 0..piece_h {
        for x in 0..piece_w {
            let src_x = min_x + x;
            let src_y = min_y + y;
            if !in_piece(src_x, src_y, kind) {
                continue;
            }
            let pixel = pixels[(src_y * width + src_x) as usize];
            if pixel.alpha() == 0 {
                continue;
            }
            image.pixels_mut()[(y * piece_w + x) as usize] = pixel;
        }
    }
    Piece {
        image,
        x: min_x as f32,
        y: min_y as f32,
        blank: false,
    }
}

fn decode_sprite(bytes: &[u8]) -> Pixmap {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .expect("MAKO-SPRITE-0001: The bundled pet image is invalid");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .expect("MAKO-SPRITE-0001: The bundled pet image is invalid");
    if info.bit_depth != png::BitDepth::Eight || info.color_type != png::ColorType::Rgba {
        panic!("MAKO-SPRITE-0001: The bundled pet image is invalid");
    }
    let mut pixmap = Pixmap::new(info.width, info.height).expect("pixmap");
    let data = &buf[..info.buffer_size()];
    let (src_chunks, _) = data.as_chunks::<4>();
    for (src, dst) in src_chunks.iter().zip(pixmap.pixels_mut()) {
        *dst = PremultipliedColorU8::from_rgba(src[0], src[1], src[2], src[3])
            .unwrap_or(PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap());
    }
    pixmap
}

fn draw_menu(pixmap: &mut Pixmap, scene: &Scene, scale: u32, pet_x: f32) -> Vec<Hit> {
    let s = scale as f32;
    let n = scene.rows.len() as f32;
    let top = 6.0;
    let row_h = ((SPRITE_H - 12.0) / n).min(24.0);
    let x = if scene.menu_left {
        4.0
    } else {
        pet_x + SPRITE_W + 4.0
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
    let font_px = (row_h * 0.58).clamp(12.0, 15.0);
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

fn mirror_sprite(pixmap: &mut Pixmap, origin_x: f32, scale: u32) {
    let x0 = (origin_x * scale as f32).round() as u32;
    let span = (SPRITE_W * scale as f32).round() as u32;
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
            walking: false,
            tilt: 0.0,
            facing_left: false,
            menu: false,
            menu_left: false,
            dark: false,
            rows: Vec::new(),
        }
    }

    #[test]
    fn idle_pose_is_mostly_transparent_around_a_solid_body() {
        let scene = still();
        let frame = render(&scene, 1);
        let (pixels, _) = frame.bgra.as_chunks::<4>();
        let opaque = pixels.iter().filter(|pixel| pixel[3] > 16).count();
        let fraction = opaque as f32 / pixels.len() as f32;
        assert!(fraction > 0.15);
        assert!(fraction < 0.75);
        let center = ((120 * frame.width + 100) * 4) as usize;
        assert!(frame.bgra[center + 3] > 200);
        std::fs::write("/tmp/mako-idle.png", encode(&scene)).expect("preview");
    }

    #[test]
    fn walk_cycle_moves_a_planted_pose() {
        let scene = still();
        let idle = render(&scene, 1);
        let mut stepping = scene.clone();
        stepping.walking = true;
        stepping.cycle = std::f32::consts::FRAC_PI_2 / 8.0;
        let walked = render(&stepping, 1);
        let (idle_px, _) = idle.bgra.as_chunks::<4>();
        let (walk_px, _) = walked.bgra.as_chunks::<4>();
        let changed = idle_px
            .iter()
            .zip(walk_px)
            .filter(|(left, right)| left != right)
            .count();
        assert!(changed > 400);
        std::fs::write("/tmp/mako-walk.png", encode(&stepping)).expect("preview");
        stepping.cycle += std::f32::consts::PI / 8.0;
        std::fs::write("/tmp/mako-walk-b.png", encode(&stepping)).expect("preview");
    }

    #[test]
    fn menu_hits_stay_inside_the_frame_and_off_the_pet() {
        let mut scene = still();
        scene.menu = true;
        scene.rows = menu_rows();
        let right = render(&scene, 1);
        assert_rows_fit(&right.hits, false);
        scene.menu_left = true;
        let left = render(&scene, 1);
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
        let frame = render(scene, 2);
        let mut pixmap = Pixmap::new(frame.width, frame.height).expect("pixmap");
        let (src_chunks, _) = frame.bgra.as_chunks::<4>();
        for (src, dst) in src_chunks.iter().zip(pixmap.pixels_mut()) {
            *dst = PremultipliedColorU8::from_rgba(src[2], src[1], src[0], src[3])
                .unwrap_or(PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap());
        }
        pixmap.encode_png().expect("png")
    }
}
