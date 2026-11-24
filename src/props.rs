// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::OnceLock;

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::pack::{crop, Sprite};

pub fn talk_mouth() -> Sprite {
    let mut image = Pixmap::new(576, 864).expect("mouth artwork dimensions");
    paint(&mut image, [78, 37, 45, 255], None, |p| {
        p.move_to(339.0, 372.0);
        p.quad_to(349.0, 367.5, 360.0, 372.0);
        p.cubic_to(359.0, 388.0, 340.0, 388.0, 339.0, 372.0);
        p.close();
    });
    paint(&mut image, [219, 122, 130, 255], None, |p| {
        p.move_to(343.0, 379.0);
        p.quad_to(349.5, 376.0, 356.0, 379.0);
        p.quad_to(349.5, 385.0, 343.0, 379.0);
        p.close();
    });
    crop(image).expect("mouth artwork")
}

pub fn frown() -> &'static Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(|| {
        let mut image = Pixmap::new(30, 17).expect("frown dimensions");
        paint(&mut image, [88, 46, 54, 255], Some(4.5), |p| {
            p.move_to(4.0, 12.0);
            p.cubic_to(7.0, 10.0, 10.0, 3.0, 15.0, 5.0);
            p.cubic_to(20.0, 3.0, 23.0, 10.0, 26.0, 12.0);
        });
        crop(image).expect("frown artwork")
    })
}

pub fn tear() -> &'static Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(|| {
        let mut image = Pixmap::new(18, 38).expect("tear dimensions");
        paint(&mut image, [86, 176, 222, 245], None, |p| {
            p.move_to(9.0, 2.0);
            p.cubic_to(9.0, 9.0, 2.0, 21.0, 2.0, 27.0);
            p.cubic_to(2.0, 40.0, 16.0, 38.0, 16.0, 27.0);
            p.cubic_to(16.0, 21.0, 10.0, 9.0, 9.0, 2.0);
            p.close();
        });
        paint(&mut image, [224, 249, 255, 255], Some(2.2), |p| {
            p.move_to(7.0, 21.0);
            p.quad_to(3.0, 28.0, 6.0, 31.0);
        });
        crop(image).expect("tear artwork")
    })
}

pub fn parachute() -> &'static Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(|| {
        let mut image = Pixmap::new(256, 162).expect("parachute dimensions");
        paint(&mut image, [112, 77, 83, 255], Some(3.0), |p| {
            for start in [10.0, 76.0, 180.0, 246.0] {
                p.move_to(start, 79.0);
                p.line_to(if start < 128.0 { 118.0 } else { 138.0 }, 157.0);
            }
        });
        paint(&mut image, [222, 91, 103, 255], None, canopy_outline);
        paint(&mut image, [255, 181, 155, 255], None, |p| {
            p.move_to(128.0, 8.0);
            p.cubic_to(97.0, 16.0, 84.0, 49.0, 76.0, 82.0);
            p.quad_to(129.0, 64.0, 180.0, 82.0);
            p.cubic_to(173.0, 49.0, 159.0, 16.0, 128.0, 8.0);
            p.close();
        });
        paint(&mut image, [129, 57, 76, 255], Some(4.0), canopy_outline);
        crop(image).expect("parachute artwork")
    })
}

fn canopy_outline(p: &mut PathBuilder) {
    p.move_to(10.0, 82.0);
    p.cubic_to(32.0, 28.0, 73.0, 8.0, 128.0, 8.0);
    p.cubic_to(183.0, 8.0, 224.0, 28.0, 246.0, 82.0);
    p.quad_to(212.0, 68.0, 180.0, 82.0);
    p.quad_to(129.0, 64.0, 76.0, 82.0);
    p.quad_to(43.0, 68.0, 10.0, 82.0);
    p.close();
}

pub fn hand() -> &'static Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(|| {
        let mut image = Pixmap::new(109, 260).expect("hand dimensions");
        paint(&mut image, [86, 126, 155, 255], None, |p| {
            p.move_to(30.0, 0.0);
            p.line_to(80.0, 0.0);
            p.line_to(80.0, 150.0);
            p.quad_to(55.0, 165.0, 30.0, 150.0);
            p.close();
        });
        paint(&mut image, [247, 207, 172, 255], None, hand_outline);
        paint(&mut image, [130, 83, 88, 255], Some(3.0), hand_outline);
        paint(&mut image, [192, 134, 126, 255], Some(2.2), |p| {
            p.move_to(39.0, 210.0);
            p.quad_to(56.0, 191.0, 75.0, 200.0);
            p.move_to(54.0, 210.0);
            p.quad_to(51.0, 221.0, 56.0, 230.0);
        });
        crop(image).expect("hand artwork")
    })
}

fn hand_outline(p: &mut PathBuilder) {
    p.move_to(35.0, 149.0);
    p.quad_to(55.0, 156.0, 75.0, 147.0);
    p.cubic_to(78.0, 173.0, 94.0, 182.0, 98.0, 203.0);
    p.line_to(100.0, 227.0);
    p.cubic_to(100.0, 241.0, 87.0, 245.0, 84.0, 231.0);
    p.cubic_to(83.0, 261.0, 67.0, 258.0, 67.0, 239.0);
    p.cubic_to(63.0, 260.0, 49.0, 255.0, 49.0, 236.0);
    p.cubic_to(44.0, 250.0, 31.0, 244.0, 33.0, 224.0);
    p.cubic_to(29.0, 220.0, 25.0, 237.0, 16.0, 230.0);
    p.cubic_to(6.0, 226.0, 14.0, 211.0, 18.0, 198.0);
    p.cubic_to(23.0, 178.0, 32.0, 175.0, 35.0, 149.0);
    p.close();
}

fn paint(
    image: &mut Pixmap,
    color: [u8; 4],
    width: Option<f32>,
    build: impl FnOnce(&mut PathBuilder),
) {
    let mut path = PathBuilder::new();
    build(&mut path);
    let path = path.finish().expect("prop path");
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    if let Some(width) = width {
        let stroke = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Stroke::default()
        };
        image.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    } else {
        image.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

pub fn snowball() -> &'static Sprite {
    static SPRITE: OnceLock<Sprite> = OnceLock::new();
    SPRITE.get_or_init(|| crop(ball_image(1)).expect("snowball artwork"))
}

pub fn ball_image(scale: u32) -> Pixmap {
    let mut image = Pixmap::new(12 * scale, 12 * scale).expect("snowball dimensions");
    let s = scale as f32;
    snow_oval(&mut image, (0.4 * s, 0.4 * s, 11.2 * s, 11.2 * s));
    image
}

pub fn pile(servings: u8, scale: u32) -> Pixmap {
    let width = 27 + u32::from(servings) * 5;
    let height = 8 + u32::from(servings) * 3;
    let mut image = Pixmap::new(width * scale, height * scale).expect("snow pile dimensions");
    let (w, h, s) = (width as f32, height as f32, scale as f32);
    let mut path = PathBuilder::new();
    path.move_to(s, (h - 1.0) * s);
    path.cubic_to(
        w * 0.14 * s,
        h * 0.8 * s,
        w * 0.15 * s,
        h * 0.33 * s,
        w * 0.38 * s,
        h * 0.33 * s,
    );
    path.cubic_to(
        w * 0.48 * s,
        -h * 0.13 * s,
        w * 0.69 * s,
        h * 0.04 * s,
        w * 0.78 * s,
        h * 0.42 * s,
    );
    path.quad_to(w * 0.89 * s, h * 0.43 * s, (w - 1.0) * s, (h - 1.0) * s);
    path.close();
    let path = path.finish().unwrap();
    let paint = Paint {
        shader: tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, h * s),
            vec![
                tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(251, 253, 255, 255)),
                tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(144, 185, 212, 255)),
            ],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap(),
        ..Paint::default()
    };
    image.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    image
}

pub const SNOWMAN_STEPS: u8 = 20;

pub fn snow_chunk(kind: usize, scale: u32) -> Pixmap {
    let size = 8 + kind as u32 * 3;
    let mut image = Pixmap::new(size * scale, size * scale).expect("snow chunk dimensions");
    let s = (size * scale) as f32;
    paint(&mut image, [231, 246, 255, 255], None, |p| {
        p.move_to(s * 0.12, s * 0.52);
        p.line_to(s * 0.28, s * 0.13);
        p.line_to(s * 0.72, s * 0.09);
        p.line_to(s * 0.92, s * 0.43);
        p.line_to(s * 0.76, s * 0.79);
        p.line_to(s * 0.43, s * 0.91);
        p.close();
    });
    paint(&mut image, [152, 194, 222, 255], None, |p| {
        p.move_to(s * 0.12, s * 0.52);
        p.line_to(s * 0.60, s * 0.71);
        p.line_to(s * 0.92, s * 0.43);
        p.line_to(s * 0.76, s * 0.79);
        p.line_to(s * 0.43, s * 0.91);
        p.close();
    });
    image
}

pub fn snowman(step: u8, scale: u32) -> Pixmap {
    let mut image = Pixmap::new(78 * scale, 126 * scale).expect("snowman dimensions");
    let progress = f32::from(step) / f32::from(SNOWMAN_STEPS);
    let s = scale as f32;
    let mut ball = |x: f32, bottom: f32, width: f32, height: f32| {
        snow_oval(
            &mut image,
            (
                (x - width * 0.5) * s,
                (bottom - height) * s,
                width * s,
                height * s,
            ),
        );
    };
    let base = 10.0 + 48.0 * (progress / 0.39).min(1.0);
    ball(39.0, 126.0, base, base * 0.85);
    if progress > 0.39 {
        let middle = 8.0 + 36.0 * ((progress - 0.39) / 0.32).min(1.0);
        ball(39.0, 88.0, middle, middle * 0.90);
    }
    if progress > 0.71 {
        let head = 7.0 + 25.0 * ((progress - 0.71) / 0.22).min(1.0);
        ball(39.0, 56.0, head, head * 0.94);
    }
    if progress >= 0.90 {
        paint(&mut image, [103, 78, 57, 255], Some(2.5 * s), |p| {
            p.move_to(22.0 * s, 72.0 * s);
            p.line_to(4.0 * s, 58.0 * s);
            p.move_to(56.0 * s, 72.0 * s);
            p.line_to(73.0 * s, 55.0 * s);
            p.move_to(65.0 * s, 63.0 * s);
            p.line_to(65.0 * s, 53.0 * s);
        });
    }
    if step == SNOWMAN_STEPS {
        let charcoal = [61, 73, 89, 255];
        paint(&mut image, [202, 80, 112, 255], Some(6.0 * s), |p| {
            p.move_to(28.0 * s, 57.0 * s);
            p.quad_to(40.0 * s, 62.0 * s, 51.0 * s, 56.0 * s);
            p.line_to(53.0 * s, 68.0 * s);
        });
        paint(&mut image, charcoal, Some(4.0 * s), |p| {
            for (x, y) in [
                (34.0, 40.0),
                (44.0, 40.0),
                (39.0, 68.0),
                (39.0, 78.0),
                (39.0, 89.0),
            ] {
                p.move_to(x * s, y * s);
                p.line_to((x + 0.45) * s, y * s);
            }
            p.move_to(22.0 * s, 30.0 * s);
            p.line_to(56.0 * s, 30.0 * s);
        });
        paint(&mut image, charcoal, None, |p| {
            p.move_to(28.0 * s, 29.0 * s);
            p.line_to(30.0 * s, 16.0 * s);
            p.line_to(49.0 * s, 16.0 * s);
            p.line_to(50.0 * s, 29.0 * s);
            p.close();
        });
        paint(&mut image, [232, 137, 73, 255], None, |p| {
            p.move_to(39.0 * s, 43.0 * s);
            p.line_to(51.0 * s, 46.0 * s);
            p.line_to(39.0 * s, 49.0 * s);
            p.close();
        });
    }
    image
}

fn snow_oval(image: &mut Pixmap, (x, y, w, h): (f32, f32, f32, f32)) {
    let bounds = tiny_skia::Rect::from_xywh(x, y, w, h).expect("snowball oval");
    let mut path = PathBuilder::new();
    path.push_oval(bounds);
    let path = path.finish().unwrap();
    let paint = Paint {
        shader: tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(x, y + h * 0.12),
            tiny_skia::Point::from_xy(x + w * 0.65, y + h),
            vec![
                tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(253, 254, 255, 255)),
                tiny_skia::GradientStop::new(
                    0.55,
                    tiny_skia::Color::from_rgba8(226, 242, 253, 255),
                ),
                tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(150, 186, 214, 255)),
            ],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap(),
        ..Paint::default()
    };
    image.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}
