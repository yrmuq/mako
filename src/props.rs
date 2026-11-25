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

pub(crate) fn paint(
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
    let mut image = Pixmap::new(24 * scale, 24 * scale).expect("snowball dimensions");
    let s = scale as f32;
    snow_oval(&mut image, (0.8 * s, 0.8 * s, 22.4 * s, 22.4 * s));
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
const SNOWMAN_BASE_END: f32 = 0.39;
const SNOWMAN_MIDDLE_END: f32 = 0.71;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SculptStage {
    #[default]
    Base,
    Middle,
    Top,
}

impl SculptStage {
    pub fn for_step(step: u8) -> Self {
        let progress = f32::from(step) / f32::from(SNOWMAN_STEPS);
        if progress > SNOWMAN_MIDDLE_END {
            Self::Top
        } else if progress > SNOWMAN_BASE_END {
            Self::Middle
        } else {
            Self::Base
        }
    }
}

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
    let base = 10.0 + 48.0 * (progress / SNOWMAN_BASE_END).min(1.0);
    ball(39.0, 126.0, base, base * 0.85);
    if progress > SNOWMAN_BASE_END {
        let middle = 8.0
            + 36.0
                * ((progress - SNOWMAN_BASE_END) / (SNOWMAN_MIDDLE_END - SNOWMAN_BASE_END))
                    .min(1.0);
        ball(39.0, 88.0, middle, middle * 0.90);
    }
    if progress > SNOWMAN_MIDDLE_END {
        let head = 7.0 + 25.0 * ((progress - SNOWMAN_MIDDLE_END) / 0.22).min(1.0);
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

pub(crate) fn oval(image: &mut Pixmap, (x, y, w, h): (f32, f32, f32, f32), color: [u8; 4]) {
    paint(image, color, None, |p| {
        p.push_oval(tiny_skia::Rect::from_xywh(x, y, w, h).expect("oval bounds"));
    });
}

pub(crate) fn resize_art(image: Pixmap, scale: u32) -> Pixmap {
    if scale == 1 {
        return image;
    }
    let mut large =
        Pixmap::new(image.width() * scale, image.height() * scale).expect("scaled artwork");
    large.draw_pixmap(
        0,
        0,
        image.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        Transform::from_scale(scale as f32, scale as f32),
        None,
    );
    large
}

pub fn snow_wall(step: u8, slope_right: bool, scale: u32) -> Pixmap {
    use crate::ramps::{Cover, ART_WIDTH, HALF, HEIGHT, RUN};
    let mut image = Pixmap::new(ART_WIDTH, 86).expect("wall dimensions");
    let centre = ART_WIDTH as f32 * 0.5;
    let floor = 86.0;
    let complete = step >= SNOWMAN_STEPS;
    let top = floor - (8.0 + (HEIGHT - 8.0) * f32::from(step) / f32::from(SNOWMAN_STEPS));
    let side = if slope_right { 1.0 } else { -1.0 };
    let start = centre - side * HALF;
    let crest = centre + side * HALF;
    let end = crest + side * if complete { RUN } else { 0.0 };
    paint(&mut image, [164, 201, 226, 255], None, |p| {
        p.move_to(start, floor);
        p.line_to(start, top);
        p.line_to(crest, top);
        p.line_to(end, floor);
        p.close();
    });
    paint(&mut image, [226, 243, 252, 255], None, |p| {
        p.move_to(start + side * 3.0, floor - 1.0);
        p.line_to(start + side * 3.0, top + 3.0);
        p.line_to(crest, top + 3.0);
        p.line_to(end - side * 5.0, floor - 1.0);
        p.close();
    });
    paint(&mut image, [250, 254, 255, 255], Some(2.4), |p| {
        p.move_to(start + side * 2.0, top + 2.0);
        p.line_to(crest, top + 2.0);
        if complete {
            p.line_to(end - side * 4.0, floor - 2.0);
        }
    });
    if complete {
        let hole = Cover::new(centre, floor, slope_right).window;
        paint(&mut image, [148, 190, 218, 255], Some(3.5), |p| {
            p.push_rect(hole)
        });
        let clear = tiny_skia::PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap();
        for y in hole.top() as usize..hole.bottom() as usize {
            let start = y * ART_WIDTH as usize + hole.left() as usize;
            image.pixels_mut()[start..start + hole.width() as usize].fill(clear);
        }
        paint(&mut image, [104, 76, 63, 255], Some(3.5), |p| {
            let at = start - side * 8.0;
            for x in [at - 6.0, at + 6.0] {
                p.move_to(x, top + 1.0);
                p.line_to(x, floor);
            }
            for y in (11..85).step_by(10) {
                p.move_to(at - 6.0, y as f32);
                p.line_to(at + 6.0, y as f32);
            }
        });
    }
    resize_art(image, scale)
}

pub fn tree(stage: u8, decorations: u8, scale: u32) -> Pixmap {
    let mut image = Pixmap::new(145, 185).expect("tree dimensions");
    let green = [40, 112, 76, 255];
    let centre = 72.5;
    if stage > 0 {
        let size = [0.0, 0.10, 0.25, 0.46, 0.71, 1.0][usize::from(stage.min(5))];
        paint(
            &mut image,
            [128, 85, 57, 255],
            Some(2.0 + 7.0 * size),
            |p| {
                p.move_to(centre + 2.0, 162.0);
                p.quad_to(centre - 2.0, 153.0, centre, 155.0 - 93.0 * size);
            },
        );
        for tier in (0..stage.min(4)).rev() {
            let n = f32::from(tier);
            let y = 155.0 - (132.0 - n * 28.0) * size;
            let half = (19.0 + n * 13.0) * size;
            let height = (47.0 + n * 2.0) * size;
            let branch = |p: &mut PathBuilder| {
                p.move_to(centre, y);
                p.quad_to(
                    centre + half * 0.43,
                    y + height * 0.63,
                    centre + half,
                    y + height,
                );
                p.quad_to(
                    centre + half * 0.35,
                    y + height * 1.14,
                    centre,
                    y + height * 0.96,
                );
                p.quad_to(
                    centre - half * 0.49,
                    y + height * 1.16,
                    centre - half,
                    y + height,
                );
                p.quad_to(centre - half * 0.35, y + height * 0.54, centre, y);
                p.close();
            };
            paint(&mut image, green, None, branch);
            paint(
                &mut image,
                [66, 145, 97, 255],
                Some((2.4 * size).max(1.0)),
                |p| {
                    p.move_to(centre - half * 0.74, y + height * 0.91);
                    p.quad_to(
                        centre - half * 0.36,
                        y + height * 0.63,
                        centre - 1.0,
                        y + height * 0.27,
                    );
                },
            );
        }
    }
    paint(&mut image, [132, 70, 60, 255], None, |p| {
        p.move_to(50.0, 161.0);
        p.line_to(95.0, 161.0);
        p.line_to(89.0, 180.0);
        p.quad_to(73.0, 185.0, 56.0, 180.0);
        p.close();
    });
    oval(&mut image, (48.0, 156.0, 49.0, 12.0), [178, 104, 82, 255]);
    oval(&mut image, (53.0, 157.0, 39.0, 6.0), [91, 58, 51, 255]);
    if stage == 0 {
        oval(&mut image, (69.0, 156.0, 7.0, 4.0), [221, 181, 104, 255]);
    } else {
        oval(&mut image, (70.0, 154.0, 5.0, 6.0), green);
    }
    if decorations >= 2 {
        paint(&mut image, [241, 191, 97, 255], Some(2.8), |p| {
            p.move_to(58.0, 58.0);
            p.quad_to(75.0, 74.0, 92.0, 70.0);
            p.move_to(48.0, 85.0);
            p.quad_to(75.0, 106.0, 105.0, 102.0);
            p.move_to(32.0, 117.0);
            p.quad_to(73.0, 145.0, 123.0, 134.0);
        });
    }
    for (index, (x, y, color)) in [
        (65.0, 56.0, [227, 92, 100, 255]),
        (87.0, 79.0, [146, 205, 255, 255]),
        (49.0, 98.0, [244, 134, 161, 255]),
        (108.0, 120.0, [224, 81, 99, 255]),
        (71.0, 121.0, [191, 164, 249, 255]),
        (41.0, 135.0, [149, 213, 249, 255]),
    ]
    .into_iter()
    .enumerate()
    {
        if decorations >= if index < 3 { 1 } else { 3 } {
            oval(&mut image, (x, y, 9.0, 10.0), color);
            oval(
                &mut image,
                (x + 2.0, y + 1.0, 2.0, 3.0),
                [255, 246, 230, 235],
            );
        }
    }
    if decorations >= 4 {
        star(&mut image, centre, 21.0, 15.0, [255, 211, 73, 255]);
        star(&mut image, centre - 1.0, 20.0, 8.0, [255, 233, 152, 255]);
    }
    resize_art(image, scale)
}

fn star(image: &mut Pixmap, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
    paint(image, color, None, |p| {
        for i in 0..10 {
            let angle = i as f32 * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
            let r = if i % 2 == 0 { radius } else { radius * 0.43 };
            let (x, y) = (cx + r * angle.cos(), cy + r * angle.sin());
            if i == 0 {
                p.move_to(x, y);
            } else {
                p.line_to(x, y);
            }
        }
        p.close();
    });
}

pub fn hand_ornament(kind: crate::sim::Special) -> &'static Sprite {
    use crate::sim::Special;
    static BALL: OnceLock<Sprite> = OnceLock::new();
    static TINSEL: OnceLock<Sprite> = OnceLock::new();
    static STAR: OnceLock<Sprite> = OnceLock::new();
    let cache = match kind {
        Special::TinselTree => &TINSEL,
        Special::StarTree => &STAR,
        _ => &BALL,
    };
    cache.get_or_init(|| {
        let mut image = Pixmap::new(42, 42).expect("ornament dimensions");
        match kind {
            Special::TinselTree => {
                paint(&mut image, [244, 192, 89, 255], Some(4.5), |p| {
                    p.move_to(7.0, 9.0);
                    p.cubic_to(12.0, 20.0, 39.0, 1.0, 32.0, 21.0);
                    p.cubic_to(27.0, 35.0, 12.0, 22.0, 35.0, 35.0);
                });
            }
            Special::StarTree => star(&mut image, 21.0, 21.0, 19.0, [255, 211, 73, 255]),
            _ => {
                oval(&mut image, (7.0, 10.0, 28.0, 30.0), [218, 85, 121, 255]);
                oval(&mut image, (12.0, 15.0, 6.0, 10.0), [255, 175, 186, 255]);
                paint(&mut image, [240, 201, 105, 255], Some(3.0), |p| {
                    p.move_to(17.0, 12.0);
                    p.cubic_to(14.0, 2.0, 29.0, 2.0, 24.0, 12.0);
                });
            }
        }
        crop(image).expect("ornament artwork")
    })
}

pub const FESTIVE_COLORS: [[u8; 4]; 4] = [
    [219, 69, 104, 255],
    [85, 161, 195, 255],
    [238, 173, 62, 255],
    [168, 111, 211, 255],
];

pub fn gift(kind: usize, scale: u32) -> Pixmap {
    colored_gift(kind, [255, 226, 153, 255], scale)
}

pub fn colored_gift(kind: usize, ribbon: [u8; 4], scale: u32) -> Pixmap {
    let mut image = Pixmap::new(33, 33).expect("gift dimensions");
    let color = FESTIVE_COLORS[kind % FESTIVE_COLORS.len()];
    paint(&mut image, color, None, |p| {
        p.move_to(3.0, 11.0);
        p.line_to(29.0, 11.0);
        p.line_to(27.0, 33.0);
        p.line_to(5.0, 33.0);
        p.close();
    });
    paint(&mut image, ribbon, Some(3.0), |p| {
        p.move_to(16.0, 32.0);
        p.line_to(16.0, 10.0);
        p.cubic_to(1.0, 7.0, 9.0, 0.0, 16.0, 10.0);
        p.cubic_to(21.0, -1.0, 29.0, 7.0, 16.0, 10.0);
        p.move_to(4.0, 18.0);
        p.line_to(28.0, 18.0);
    });
    resize_art(image, scale)
}

pub fn confetti(kind: usize, scale: u32) -> Pixmap {
    let mut image = Pixmap::new(7, 7).expect("confetti dimensions");
    paint(
        &mut image,
        FESTIVE_COLORS[kind % FESTIVE_COLORS.len()],
        None,
        |p| {
            p.move_to(1.0, 2.0);
            p.line_to(6.0, 0.5);
            p.line_to(5.0, 6.0);
            p.line_to(2.0, 7.0);
            p.close();
        },
    );
    resize_art(image, scale)
}

pub fn water_drop(scale: u32) -> Pixmap {
    let mut image = Pixmap::new(6, 9).expect("water drop dimensions");
    paint(&mut image, [125, 204, 245, 242], None, |p| {
        p.move_to(3.0, 0.0);
        p.cubic_to(2.3, 3.6, 0.0, 4.4, 0.4, 6.4);
        p.cubic_to(1.0, 10.1, 6.0, 9.1, 5.7, 6.2);
        p.quad_to(5.3, 3.4, 3.0, 0.0);
        p.close();
    });
    resize_art(image, scale)
}

pub fn candy() -> &'static Sprite {
    static ART: OnceLock<Sprite> = OnceLock::new();
    ART.get_or_init(|| {
        let mut image = Pixmap::new(138, 82).expect("candy dimensions");
        paint(&mut image, [221, 88, 146, 255], None, |p| {
            p.move_to(33.0, 32.0);
            p.line_to(2.0, 14.0);
            p.line_to(5.0, 64.0);
            p.line_to(36.0, 48.0);
            p.line_to(103.0, 48.0);
            p.line_to(134.0, 64.0);
            p.line_to(137.0, 14.0);
            p.line_to(101.0, 31.0);
            p.close();
        });
        oval(&mut image, (26.0, 5.0, 87.0, 73.0), [174, 62, 119, 255]);
        oval(&mut image, (30.0, 8.0, 79.0, 66.0), [246, 131, 185, 255]);
        paint(&mut image, [255, 223, 232, 255], Some(8.0), |p| {
            p.move_to(45.0, 55.0);
            p.cubic_to(27.0, 12.0, 90.0, 8.0, 93.0, 39.0);
            p.cubic_to(94.0, 60.0, 57.0, 63.0, 55.0, 42.0);
            p.quad_to(58.0, 27.0, 74.0, 39.0);
        });
        crop(image).expect("candy artwork")
    })
}

pub fn open_mouth() -> &'static Sprite {
    static ART: OnceLock<Sprite> = OnceLock::new();
    ART.get_or_init(|| {
        let mut image = Pixmap::new(72, 61).expect("open mouth dimensions");
        oval(&mut image, (1.0, 1.0, 70.0, 59.0), [81, 40, 60, 255]);
        oval(&mut image, (17.0, 31.0, 39.0, 24.0), [232, 130, 156, 255]);
        oval(&mut image, (12.0, 4.0, 48.0, 13.0), [255, 232, 218, 255]);
        crop(image).expect("open mouth artwork")
    })
}

pub fn watering_can() -> &'static Sprite {
    static ART: OnceLock<Sprite> = OnceLock::new();
    ART.get_or_init(|| {
        let mut image = Pixmap::new(76, 56).expect("watering can dimensions");
        paint(&mut image, [79, 147, 188, 255], Some(5.0), |p| {
            p.move_to(22.0, 23.0);
            p.cubic_to(3.0, 3.0, 3.0, 41.0, 23.0, 39.0);
            p.move_to(46.0, 34.0);
            p.line_to(67.0, 16.0);
        });
        oval(&mut image, (17.0, 11.0, 37.0, 34.0), [113, 192, 221, 255]);
        oval(&mut image, (19.0, 9.0, 32.0, 10.0), [68, 135, 174, 255]);
        oval(&mut image, (65.0, 9.0, 9.0, 11.0), [94, 159, 191, 255]);
        crop(image).expect("watering can artwork")
    })
}

pub fn snow_cannon() -> &'static Sprite {
    static ART: OnceLock<Sprite> = OnceLock::new();
    ART.get_or_init(|| {
        let mut image = Pixmap::new(112, 59).expect("snow cannon dimensions");
        oval(&mut image, (6.0, 19.0, 50.0, 35.0), [112, 161, 204, 255]);
        oval(&mut image, (16.0, 2.0, 31.0, 28.0), [227, 247, 255, 255]);
        paint(&mut image, [100, 161, 209, 255], None, |p| {
            p.move_to(42.0, 17.0);
            p.line_to(97.0, 17.0);
            p.line_to(104.0, 40.0);
            p.line_to(40.0, 42.0);
            p.close();
        });
        oval(&mut image, (88.0, 14.0, 23.0, 29.0), [206, 233, 249, 255]);
        oval(&mut image, (95.0, 19.0, 15.0, 19.0), [68, 114, 157, 255]);
        paint(&mut image, [245, 190, 96, 255], Some(4.0), |p| {
            p.move_to(58.0, 20.0);
            p.line_to(78.0, 20.0);
            p.move_to(43.0, 42.0);
            p.line_to(40.0, 57.0);
        });
        crop(image).expect("snow cannon artwork")
    })
}

pub fn baseball_bat() -> &'static Sprite {
    static ART: OnceLock<Sprite> = OnceLock::new();
    ART.get_or_init(|| {
        let mut image = Pixmap::new(133, 34).expect("baseball bat dimensions");
        let outline = |p: &mut PathBuilder| {
            p.move_to(8.0, 13.0);
            p.cubic_to(32.0, 13.0, 43.0, 12.0, 64.0, 9.0);
            p.cubic_to(85.0, 5.0, 105.0, 4.0, 121.0, 4.0);
            p.cubic_to(134.0, 3.0, 134.0, 28.0, 121.0, 28.0);
            p.cubic_to(102.0, 29.0, 84.0, 27.0, 64.0, 24.0);
            p.cubic_to(41.0, 20.0, 29.0, 21.0, 8.0, 21.0);
            p.close();
        };
        paint(&mut image, [221, 166, 113, 255], None, outline);
        paint(&mut image, [136, 82, 64, 255], Some(2.1), outline);
        paint(&mut image, [251, 208, 143, 255], Some(2.2), |p| {
            p.move_to(47.0, 15.0);
            p.quad_to(84.0, 9.0, 119.0, 9.0);
        });
        paint(&mut image, [120, 73, 70, 255], Some(7.2), |p| {
            p.move_to(8.0, 17.0);
            p.line_to(30.0, 17.0);
        });
        oval(&mut image, (1.0, 10.0, 9.0, 14.0), [144, 92, 71, 255]);
        crop(image).expect("baseball bat artwork")
    })
}

pub fn sled_image(left: bool, scale: u32) -> Pixmap {
    let mut image = Pixmap::new(106, 32).expect("sled dimensions");
    let steel = [150, 72, 56, 255];
    for offset in [0.0, 4.0] {
        paint(&mut image, steel, Some(3.9), |p| {
            p.move_to(8.0, 24.0 + offset);
            p.line_to(79.0, 24.0 + offset);
            p.quad_to(99.0, 24.0 + offset, 100.0, 12.0 + offset);
            p.quad_to(99.0, 8.0 + offset, 94.0, 9.0 + offset);
        });
    }
    paint(&mut image, [122, 88, 75, 255], Some(4.4), |p| {
        for x in [20.0, 70.0] {
            p.move_to(x, 17.0);
            p.line_to(x + 3.0, 25.0);
        }
    });
    paint(&mut image, [184, 115, 67, 255], None, |p| {
        p.move_to(10.0, 10.0);
        p.line_to(84.0, 10.0);
        p.quad_to(89.0, 13.0, 83.0, 18.0);
        p.line_to(9.0, 18.0);
        p.quad_to(5.0, 15.0, 10.0, 10.0);
        p.close();
    });
    paint(&mut image, [236, 170, 104, 255], Some(2.0), |p| {
        p.move_to(12.0, 12.0);
        p.line_to(82.0, 12.0);
        p.move_to(10.0, 15.5);
        p.line_to(78.0, 15.5);
    });
    if left {
        let mut reversed = Pixmap::new(106, 32).unwrap();
        reversed.draw_pixmap(
            0,
            0,
            image.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::from_scale(-1.0, 1.0).post_translate(106.0, 0.0),
            None,
        );
        image = reversed;
    }
    resize_art(image, scale)
}

pub fn sled_rope(dx: i32, dy: i32, scale: u32) -> Pixmap {
    let width = dx.unsigned_abs() + 8;
    let height = dy.unsigned_abs() + 12;
    let mut image = Pixmap::new(width, height).expect("sled rope dimensions");
    let origin = (4.0 - dx.min(0) as f32, 4.0 - dy.min(0) as f32);
    let end = (origin.0 + dx as f32, origin.1 + dy as f32);
    paint(&mut image, [113, 82, 68, 255], Some(1.8), |p| {
        p.move_to(origin.0, origin.1);
        p.quad_to(
            (origin.0 + end.0) * 0.5,
            (origin.1 + end.1) * 0.5 + 6.0,
            end.0,
            end.1,
        );
    });
    resize_art(image, scale)
}
