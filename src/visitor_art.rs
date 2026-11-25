// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, SpreadMode,
    Transform,
};

use crate::props::{oval, paint, resize_art};

pub const SANTA_WIDTH: f32 = 230.0;
pub const SANTA_GRAB: f32 = -61.0;
pub const BEAM_WIDTH: f32 = 174.0;

pub fn bag(scale: u32) -> Pixmap {
    let mut art = Pixmap::new(49, 47).unwrap();
    paint(&mut art, [168, 32, 59, 255], None, |p| {
        p.move_to(15.0, 13.0);
        p.line_to(11.0, 2.0);
        p.quad_to(22.0, 7.0, 33.0, 2.0);
        p.line_to(30.0, 14.0);
        p.cubic_to(35.0, 23.0, 52.0, 32.0, 44.0, 42.0);
        p.cubic_to(37.0, 48.0, 6.0, 48.0, 3.0, 40.0);
        p.cubic_to(-1.0, 30.0, 10.0, 23.0, 15.0, 13.0);
        p.close();
    });
    oval(&mut art, (7.0, 23.0, 24.0, 17.0), [212, 54, 76, 255]);
    paint(&mut art, [255, 209, 120, 255], Some(3.0), |p| {
        p.move_to(13.0, 14.0);
        p.line_to(31.0, 14.0);
        p.quad_to(40.0, 11.0, 37.0, 19.0);
        p.move_to(29.0, 14.0);
        p.line_to(32.0, 23.0);
    });
    resize_art(art, scale)
}

pub fn ufo(scale: u32) -> Pixmap {
    let mut art = Pixmap::new(138, 55).unwrap();
    oval(&mut art, (23.0, 29.0, 92.0, 25.0), [51, 63, 107, 255]);
    oval(&mut art, (2.0, 19.0, 134.0, 28.0), [98, 108, 162, 255]);
    oval(&mut art, (7.0, 19.0, 124.0, 17.0), [157, 179, 216, 255]);
    oval(&mut art, (44.0, 1.0, 50.0, 29.0), [82, 193, 203, 255]);
    oval(&mut art, (52.0, 5.0, 19.0, 8.0), [175, 247, 228, 255]);
    for (i, x) in [20.0, 42.0, 64.0, 86.0, 108.0].into_iter().enumerate() {
        oval(
            &mut art,
            (x, 36.0, 9.0, 4.0),
            if i % 2 == 0 {
                [172, 252, 184, 255]
            } else {
                [246, 211, 128, 255]
            },
        );
    }
    oval(&mut art, (54.0, 46.0, 30.0, 8.0), [195, 255, 208, 255]);
    resize_art(art, scale)
}

pub fn beam(height: u32, scale: u32) -> Pixmap {
    let w = BEAM_WIDTH * scale as f32;
    let h = height.max(1) as f32 * scale as f32;
    let mut art = Pixmap::new(w as u32, h as u32).unwrap();
    let mut shape = PathBuilder::new();
    shape.move_to(w * 0.41, 0.0);
    shape.line_to(w * 0.59, 0.0);
    shape.line_to(w, h);
    shape.line_to(0.0, h);
    shape.close();
    let paint = Paint {
        shader: LinearGradient::new(
            Point::from_xy(0.0, 0.0),
            Point::from_xy(0.0, h),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(154, 251, 200, 166)),
                GradientStop::new(0.78, Color::from_rgba8(153, 251, 207, 62)),
                GradientStop::new(1.0, Color::from_rgba8(180, 252, 215, 108)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap(),
        anti_alias: true,
        ..Paint::default()
    };
    art.fill_path(
        &shape.finish().unwrap(),
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    art
}

pub fn santa(leg: usize, scale: u32) -> Pixmap {
    let mut art = Pixmap::new(SANTA_WIDTH as u32, 105).unwrap();
    let red = [202, 42, 67, 255];
    let gold = [246, 194, 100, 255];
    let snow = [255, 249, 241, 255];
    let brown = [137, 87, 61, 255];
    let dark = [74, 49, 56, 255];
    paint(&mut art, gold, Some(4.0), |p| {
        p.move_to(12.0, 94.0);
        p.quad_to(14.0, 101.0, 27.0, 101.0);
        p.line_to(98.0, 101.0);
        p.quad_to(117.0, 101.0, 120.0, 89.0);
        for x in [39.0, 90.0] {
            p.move_to(x, 91.0);
            p.line_to(x, 100.0);
        }
    });
    oval(&mut art, (13.0, 43.0, 40.0, 42.0), [164, 37, 65, 255]);
    oval(&mut art, (57.0, 41.0, 36.0, 50.0), red);
    oval(&mut art, (64.0, 20.0, 29.0, 27.0), [246, 193, 154, 255]);
    oval(&mut art, (69.0, 36.0, 25.0, 22.0), snow);
    oval(&mut art, (83.0, 29.0, 3.0, 4.0), dark);
    paint(&mut art, red, None, |p| {
        p.move_to(59.0, 25.0);
        p.quad_to(67.0, 0.0, 72.0, 8.0);
        p.line_to(92.0, 23.0);
        p.close();
    });
    oval(&mut art, (58.0, 20.0, 36.0, 6.0), snow);
    oval(&mut art, (66.0, 4.0, 9.0, 9.0), snow);
    paint(&mut art, red, None, |p| {
        p.move_to(15.0, 68.0);
        p.quad_to(57.0, 77.0, 94.0, 70.0);
        p.quad_to(105.0, 70.0, 114.0, 59.0);
        p.quad_to(120.0, 75.0, 106.0, 87.0);
        p.quad_to(68.0, 100.0, 28.0, 91.0);
        p.quad_to(18.0, 87.0, 15.0, 68.0);
        p.close();
    });
    paint(&mut art, gold, Some(3.0), |p| {
        p.move_to(22.0, 75.0);
        p.quad_to(81.0, 87.0, 109.0, 71.0);
    });
    paint(&mut art, red, Some(10.0), |p| {
        p.move_to(66.0, 51.0);
        p.quad_to(57.0, 62.0, 54.0, 75.0);
    });
    oval(&mut art, (48.0, 69.0, 13.0, 13.0), snow);
    paint(&mut art, dark, Some(2.0), |p| {
        p.move_to(84.0, 52.0);
        p.quad_to(127.0, 73.0, 188.0, 48.0);
    });
    paint(&mut art, brown, Some(4.0), |p| {
        let sway = [-11.0, 3.0, 12.0, -3.0][leg % 4];
        for (i, x) in [156.0, 163.0, 182.0, 188.0].into_iter().enumerate() {
            let kick = if i % 2 == 0 { sway } else { -sway };
            p.move_to(x, 68.0);
            p.line_to(x + kick * 0.6, 79.0);
            p.line_to(x + kick, 88.0);
        }
        p.move_to(151.0, 58.0);
        p.quad_to(140.0, 53.0, 141.0, 47.0);
    });
    oval(&mut art, (148.0, 52.0, 49.0, 23.0), brown);
    paint(&mut art, brown, Some(13.0), |p| {
        p.move_to(184.0, 64.0);
        p.line_to(195.0, 42.0);
    });
    oval(&mut art, (187.0, 34.0, 27.0, 19.0), brown);
    oval(&mut art, (210.0, 42.0, 8.0, 7.0), dark);
    oval(&mut art, (201.0, 38.0, 3.0, 4.0), dark);
    paint(&mut art, gold, Some(2.6), |p| {
        p.move_to(193.0, 38.0);
        p.quad_to(188.0, 27.0, 190.0, 17.0);
        p.move_to(190.0, 27.0);
        p.line_to(183.0, 22.0);
        p.move_to(191.0, 32.0);
        p.line_to(196.0, 25.0);
        p.move_to(200.0, 36.0);
        p.quad_to(204.0, 28.0, 203.0, 20.0);
        p.move_to(203.0, 29.0);
        p.line_to(210.0, 23.0);
    });
    paint(&mut art, red, Some(4.0), |p| {
        p.move_to(183.0, 49.0);
        p.line_to(196.0, 57.0);
    });
    resize_art(art, scale)
}

pub fn coal(scale: u32) -> Pixmap {
    let mut art = Pixmap::new(19, 15).unwrap();
    for (color, points) in [
        (
            [39, 37, 48, 255],
            [
                (0.5, 8.0),
                (5.0, 2.0),
                (12.0, 0.5),
                (18.0, 5.5),
                (17.3, 11.0),
                (10.2, 14.5),
                (3.5, 13.0),
            ],
        ),
        (
            [78, 76, 93, 255],
            [
                (2.0, 8.0),
                (6.0, 3.0),
                (12.0, 1.8),
                (15.0, 5.0),
                (10.0, 5.0),
                (7.0, 9.0),
                (5.0, 10.0),
            ],
        ),
    ] {
        paint(&mut art, color, None, |p| {
            p.move_to(points[0].0, points[0].1);
            for (x, y) in points.into_iter().skip(1) {
                p.line_to(x, y);
            }
            p.close();
        });
    }
    resize_art(art, scale)
}

pub fn bomb(lit: bool, scale: u32) -> Pixmap {
    let mut art = Pixmap::new(35, 37).unwrap();
    oval(&mut art, (3.0, 10.0, 29.0, 27.0), [35, 34, 46, 255]);
    oval(&mut art, (7.0, 14.0, 11.0, 9.0), [85, 82, 103, 255]);
    paint(&mut art, [128, 99, 73, 255], Some(2.4), |p| {
        p.move_to(19.0, 13.0);
        p.quad_to(16.0, 4.5, 28.0, 5.0);
    });
    let r = if lit { 5.0 } else { 3.0 };
    paint(&mut art, [247, 149, 52, 255], Some(1.5), |p| {
        p.move_to(28.0 - r, 5.0);
        p.line_to(28.0 + r, 5.0);
        p.move_to(28.0, 5.0 - r);
        p.line_to(28.0, 5.0 + r);
    });
    oval(&mut art, (26.5, 3.5, 3.0, 3.0), [255, 241, 153, 255]);
    resize_art(art, scale)
}

pub fn blast(frame: usize, scale: u32) -> Pixmap {
    let mut art = Pixmap::new(270, 182).unwrap();
    let radius = [44.0, 81.0, 120.0][frame];
    let opacity = [255, 233, 145][frame];
    let centre = Point::from_xy(135.0, 120.0);
    for (scale, color) in [
        (1.0, [239, 88, 52, opacity]),
        (0.71, [255, 172, 60, opacity]),
        (0.43, [255, 243, 157, opacity]),
    ] {
        paint(&mut art, color, None, |path| {
            for point in 0..24 {
                let turn = point as f32 * std::f32::consts::TAU / 24.0;
                let reach = radius * scale * if point % 2 == 0 { 1.0 } else { 0.62 };
                let x = centre.x + turn.cos() * reach;
                let y = centre.y + turn.sin() * reach * 0.48;
                if point == 0 {
                    path.move_to(x, y);
                } else {
                    path.line_to(x, y);
                }
            }
            path.close();
        });
    }
    resize_art(art, scale)
}
