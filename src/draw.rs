// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

//! Bear girl, drawn from one posed image. The hair clip is a round barrette, not a cross.

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
use crate::sim::{SPRITE_H, SPRITE_W};

pub const MENU_W: f32 = 228.0;

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
    pub swing: f32,
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

fn draw_pet(pixmap: &mut Pixmap, origin_x: f32, scene: &Scene, scale: u32) {
    let s = scale as f32;
    let bob = (scene.swing * 2.0).sin().abs() * -2.0;
    let tilt = scene.tilt.to_degrees();
    let sprite = sprite_pixmap();
    let fit = Transform::from_scale(
        SPRITE_W / sprite.width() as f32,
        SPRITE_H / sprite.height() as f32,
    )
    .post_translate(origin_x, bob)
    .post_rotate_at(tilt, origin_x + 100.0, 258.0 + bob)
    .post_scale(s, s);
    // shortcut: one posed image cannot blink or swing separate limbs; split those parts when they must move on their own.
    pixmap.draw_pixmap(0, 0, sprite.as_ref(), &PixmapPaint::default(), fit, None);
}

fn sprite_pixmap() -> &'static Pixmap {
    static SPRITE: OnceLock<Pixmap> = OnceLock::new();
    SPRITE.get_or_init(|| decode_sprite(include_bytes!("../assets/bear-girl.png")))
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
    let x = if scene.menu_left {
        12.0
    } else {
        pet_x + SPRITE_W - 8.0
    };
    let y = 36.0;
    let w = MENU_W - 24.0;
    let row_h = 22.0;
    let h = 16.0 + scene.rows.len() as f32 * row_h;
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
    fill_rect(pixmap, x, y, w, h, 10.0, panel, s);

    let mut hits = Vec::new();
    let font = font();
    for (index, row) in scene.rows.iter().enumerate() {
        let ry = y + 10.0 + index as f32 * row_h;
        let color = if row.selected {
            mark
        } else if row.label {
            muted
        } else {
            ink
        };
        let prefix = if row.selected { "● " } else { "" };
        let text = format!("{prefix}{}", row.text);
        draw_text(pixmap, font, x + 14.0, ry + 15.0, 15.0 * s, &text, color, s);
        if !row.label {
            hits.push(Hit {
                id: row.id,
                x,
                y: ry - 2.0,
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
        &paint(color),
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

fn paint(color: ColorU8) -> Paint<'static> {
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

    #[test]
    fn idle_pose_is_mostly_transparent_around_a_solid_body() {
        let scene = Scene {
            swing: 0.0,
            tilt: 0.0,
            facing_left: false,
            menu: false,
            menu_left: false,
            dark: false,
            rows: Vec::new(),
        };
        let frame = render(&scene, 1);
        let (pixels, _) = frame.bgra.as_chunks::<4>();
        let opaque = pixels.iter().filter(|pixel| pixel[3] > 16).count();
        let fraction = opaque as f32 / pixels.len() as f32;
        assert!(fraction > 0.15);
        assert!(fraction < 0.75);
        let center = ((120 * frame.width + 100) * 4) as usize;
        assert!(frame.bgra[center + 3] > 200);
        std::fs::write("/tmp/bear-girl.png", encode(&scene)).expect("preview");
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
