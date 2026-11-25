// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::f32::consts::TAU;

use tiny_skia::{LinearGradient, Paint, PathBuilder, Pixmap, Point, SpreadMode, Stroke, Transform};

use crate::props::{oval, paint, resize_art};
use crate::season::OverlayImage;

pub fn star(scale: u32) -> OverlayImage {
    let mut art = Pixmap::new(48, 48).unwrap();
    oval(&mut art, (2.0, 2.0, 44.0, 44.0), [255, 207, 86, 21]);
    oval(&mut art, (7.0, 7.0, 34.0, 34.0), [255, 220, 108, 40]);
    let shape = |p: &mut PathBuilder| {
        for i in 0..10 {
            let angle = (i as f32 / 10.0 - 0.25) * TAU;
            let radius = if i % 2 == 0 { 16.0 } else { 7.7 };
            let (y, x) = angle.sin_cos();
            let point = (24.0 + x * radius, 23.0 + y * radius);
            if i == 0 {
                p.move_to(point.0, point.1);
            } else {
                p.line_to(point.0, point.1);
            }
        }
        p.close();
    };
    paint(&mut art, [255, 211, 76, 255], None, shape);
    paint(&mut art, [255, 244, 167, 255], Some(1.6), shape);
    oval(&mut art, (21.5, 13.0, 3.0, 13.0), [255, 255, 226, 215]);
    oval(&mut art, (18.0, 18.0, 10.0, 3.0), [255, 255, 226, 225]);
    resize_art(art, scale).into()
}

pub fn spark(scale: u32) -> OverlayImage {
    let mut art = Pixmap::new(16, 16).unwrap();
    oval(&mut art, (1.0, 1.0, 14.0, 14.0), [255, 210, 92, 65]);
    paint(&mut art, [255, 246, 189, 235], None, |p| {
        p.move_to(8.0, 1.0);
        for (x, y) in [
            (9.5, 6.5),
            (15.0, 8.0),
            (9.5, 9.5),
            (8.0, 15.0),
            (6.5, 9.5),
            (1.0, 8.0),
            (6.5, 6.5),
        ] {
            p.line_to(x, y);
        }
        p.close();
    });
    resize_art(art, scale).into()
}

pub fn rainbow(width: u32, height: u32, scale: u32) -> Option<OverlayImage> {
    use tiny_skia::{Color, GradientStop};
    let mut art = Pixmap::new(width.checked_mul(scale)?, height.checked_mul(scale)?)?;
    let w = art.width() as f32;
    let h = art.height() as f32;
    let band = w.min(h) * 0.021;
    for (i, rgb) in [
        [255, 105, 133],
        [255, 174, 111],
        [255, 233, 122],
        [156, 238, 147],
        [103, 216, 244],
        [129, 161, 255],
        [209, 148, 254],
    ]
    .into_iter()
    .enumerate()
    {
        let inset = i as f32 * band * 0.82;
        let mut arc = PathBuilder::new();
        arc.move_to(w * 0.045 + inset, h * 0.94);
        arc.cubic_to(
            w * 0.02 + inset,
            -h * 0.18 + inset * 1.28,
            w * 0.98 - inset,
            -h * 0.18 + inset * 1.28,
            w * 0.955 - inset,
            h * 0.94,
        );
        let shader = LinearGradient::new(
            Point::from_xy(0.0, h * 0.12),
            Point::from_xy(0.0, h * 0.94),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(rgb[0], rgb[1], rgb[2], 234)),
                GradientStop::new(0.67, Color::from_rgba8(rgb[0], rgb[1], rgb[2], 195)),
                GradientStop::new(1.0, Color::from_rgba8(rgb[0], rgb[1], rgb[2], 0)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )?;
        art.stroke_path(
            &arc.finish()?,
            &Paint {
                shader,
                ..Paint::default()
            },
            &Stroke {
                width: band,
                ..Stroke::default()
            },
            Transform::identity(),
            None,
        );
    }
    for _ in 0..48 {
        let t = fastrand::f32();
        let x = w * (0.05 + 0.9 * t);
        let arc_y = h * (0.13 + 0.80 * (2.0 * t - 1.0).powi(2));
        let y = arc_y + (fastrand::f32() - 0.45) * h * 0.24;
        let size = (2.0 + fastrand::f32() * 4.0) * scale as f32;
        oval(&mut art, (x, y, size, size), [255, 250, 214, 180]);
    }
    Some(art.into())
}

pub fn rift(open: f32, pupil: f32, scale: u32) -> OverlayImage {
    let mut art = Pixmap::new(226, 170).unwrap();
    for (end_x, end_y, bend_x, bend_y) in [
        (14.0, 39.0, 45.0, 63.0),
        (45.0, 6.0, 80.0, 33.0),
        (160.0, 12.0, 137.0, 37.0),
        (212.0, 48.0, 170.0, 61.0),
        (23.0, 134.0, 60.0, 104.0),
        (76.0, 155.0, 83.0, 114.0),
        (155.0, 161.0, 142.0, 119.0),
        (205.0, 126.0, 166.0, 104.0),
    ] {
        paint(
            &mut art,
            [201, 156, 255, (open * 142.0) as u8],
            Some(5.0),
            |p| {
                p.move_to(113.0, 83.0);
                p.line_to(
                    113.0 + (bend_x - 113.0) * open,
                    83.0 + (bend_y - 83.0) * open,
                );
                p.line_to(113.0 + (end_x - 113.0) * open, 83.0 + (end_y - 83.0) * open);
            },
        );
        paint(&mut art, [24, 12, 39, 244], Some(1.8), |p| {
            p.move_to(113.0, 83.0);
            p.line_to(
                113.0 + (bend_x - 113.0) * open,
                83.0 + (bend_y - 83.0) * open,
            );
            p.line_to(113.0 + (end_x - 113.0) * open, 83.0 + (end_y - 83.0) * open);
        });
    }
    let edge = |p: &mut PathBuilder| {
        p.move_to(43.0, 84.0);
        p.line_to(68.0, 84.0 - open * 22.0);
        p.line_to(96.0, 84.0 - open * 36.0);
        p.line_to(118.0, 84.0 - open * 30.0);
        p.line_to(132.0, 84.0 - open * 40.0);
        p.line_to(165.0, 84.0 - open * 18.0);
        p.line_to(183.0, 83.0);
        p.line_to(150.0, 84.0 + open * 26.0);
        p.line_to(126.0, 84.0 + open * 34.0);
        p.line_to(102.0, 84.0 + open * 27.0);
        p.line_to(81.0, 84.0 + open * 30.0);
        p.close();
    };
    paint(&mut art, [131, 75, 213, 225], Some(7.0), edge);
    paint(&mut art, [24, 12, 45, 255], None, edge);
    if open > 0.45 {
        let eye = |p: &mut PathBuilder| {
            p.move_to(76.0, 83.0);
            p.quad_to(113.0, 83.0 - 44.0 * open, 151.0, 83.0);
            p.quad_to(113.0, 83.0 + 38.0 * open, 76.0, 83.0);
            p.close();
        };
        paint(&mut art, [231, 214, 250, 247], None, eye);
        let iris = 103.0 + pupil * 11.0;
        oval(&mut art, (iris, 74.0, 21.0, 19.0), [155, 81, 201, 255]);
        oval(&mut art, (iris + 8.0, 75.0, 5.0, 17.0), [24, 12, 45, 255]);
        oval(&mut art, (iris + 4.0, 76.0, 5.0, 4.0), [255, 246, 255, 230]);
    }
    resize_art(art, scale).into()
}

pub fn bone_arm(delta: Point, scale: u32) -> Option<OverlayImage> {
    let pad = 31.0;
    let from = Point::from_xy(pad - delta.x.min(0.0), pad - delta.y.min(0.0));
    let to = Point::from_xy(from.x + delta.x, from.y + delta.y);
    let elbow = Point::from_xy(from.x + delta.x * 0.42 + 24.0, from.y + delta.y * 0.55);
    let width = (delta.x.abs() + pad * 2.0).ceil() as u32;
    let height = (delta.y.abs() + pad * 2.0).ceil() as u32;
    let mut art = Pixmap::new(width, height)?;
    for (width, hue) in [
        (15.0, [59, 39, 78, 255]),
        (10.0, [209, 200, 217, 255]),
        (3.0, [246, 234, 246, 250]),
    ] {
        paint(&mut art, hue, Some(width), |p| {
            p.move_to(from.x, from.y);
            p.line_to(elbow.x, elbow.y);
            p.line_to(to.x, to.y);
        });
    }
    oval(
        &mut art,
        (to.x - 10.0, to.y - 8.0, 20.0, 15.0),
        [224, 210, 228, 255],
    );
    for i in 0..5 {
        let x = to.x + (i as f32 - 2.0) * 4.4;
        let side = x - to.x;
        for (width, hue) in [(6.0, [66, 44, 80, 255]), (3.6, [242, 226, 241, 255])] {
            paint(&mut art, hue, Some(width), |p| {
                p.move_to(x, to.y + 3.0);
                p.line_to(x + side * 0.55, to.y + 16.0 - side.abs() * 0.25);
                p.line_to(x + side * 0.12, to.y + 23.0 - side.abs() * 0.54);
            });
        }
    }
    Some(resize_art(art, scale).into())
}

pub fn lightning(height: u32, scale: u32) -> Option<OverlayImage> {
    let mut art = Pixmap::new(142, height.max(1))?;
    let length = height as f32;
    let bolt = |p: &mut PathBuilder| {
        p.move_to(104.0, 0.0);
        for (x, y) in [
            (76.0, 0.23),
            (96.0, 0.32),
            (48.0, 0.55),
            (79.0, 0.57),
            (53.0, 0.78),
            (70.0, 1.0),
        ] {
            p.line_to(x, y * length);
        }
    };
    for (width, hue) in [
        (24.0, [113, 131, 255, 60]),
        (12.0, [139, 186, 255, 156]),
        (4.0, [246, 250, 255, 255]),
    ] {
        paint(&mut art, hue, Some(width), bolt);
    }
    paint(&mut art, [176, 206, 255, 213], Some(2.6), |p| {
        p.move_to(48.0, length * 0.55);
        p.line_to(24.0, length * 0.63);
        p.line_to(16.0, length * 0.80);
        p.move_to(76.0, length * 0.23);
        p.line_to(57.0, length * 0.28);
        p.line_to(45.0, length * 0.37);
    });
    Some(resize_art(art, scale).into())
}
