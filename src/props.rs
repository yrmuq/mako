// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::OnceLock;

use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::pack::{crop, Sprite};

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
