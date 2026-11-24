// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::VecDeque;
use std::time::Instant;

use tiny_skia::Point;

use crate::season::OverlayImage;

pub struct Grab {
    offset: f32,
    points: VecDeque<(Instant, Point)>,
}

impl Grab {
    pub fn new(cursor: Point, centre: f32, now: Instant) -> Self {
        Self {
            offset: cursor.x - centre,
            points: VecDeque::from([(now, cursor)]),
        }
    }

    pub fn cursor(&self) -> Point {
        self.points.back().expect("grab position").1
    }

    pub fn move_to(&mut self, cursor: Point, now: Instant) -> f32 {
        if self.cursor() != cursor {
            self.points.push_back((now, cursor));
            while self.points.len() > 1 && now.duration_since(self.points[0].0).as_secs_f32() > 0.13
            {
                self.points.pop_front();
            }
        }
        cursor.x - self.offset
    }

    pub fn throw(&self, now: Instant) -> Option<Point> {
        let &(end, last) = self.points.back()?;
        let &(start, first) = self
            .points
            .iter()
            .rev()
            .find(|(at, _)| end.duration_since(*at).as_secs_f32() >= 0.035)
            .or_else(|| self.points.front())?;
        if now.duration_since(end).as_secs_f32() > 0.095 {
            return None;
        }
        let dx = last.x - first.x;
        let dy = last.y - first.y;
        let dt = end.duration_since(start).as_secs_f32().max(0.02);
        (dx.hypot(dy) >= 22.0 && dx.hypot(dy) / dt >= 700.0).then(|| {
            Point::from_xy(
                (dx / dt).clamp(-1500.0, 1500.0),
                (dy / dt).clamp(-1500.0, 1500.0),
            )
        })
    }
}

pub struct Fragment {
    at: Point,
    vx: f32,
    vy: f32,
    time: f32,
    kind: usize,
}

impl Fragment {
    pub fn scatter(out: &mut Vec<Self>, at: Point, push: Point, height: f32) {
        out.clear();
        out.extend((0..9).map(|kind| {
            let lift = fastrand::f32() * height;
            Self {
                at: Point::from_xy(at.x + fastrand::f32() * 26.0 - 13.0, at.y - lift - 7.0),
                vx: push.x * 0.23 + (kind as f32 - 4.0) * 54.0,
                vy: (push.y * 0.21 - 130.0 - fastrand::f32() * 255.0).min(-90.0),
                time: 0.85 + fastrand::f32() * 0.5,
                kind: kind % 3,
            }
        }));
    }

    pub fn tick(&mut self, dt: f32, floor: f32) -> bool {
        self.time -= dt;
        self.vy += 1200.0 * dt;
        self.at.x += self.vx * dt;
        self.at.y += self.vy * dt;
        if self.at.y > floor - 4.0 {
            self.at.y = floor - 4.0;
            self.vy = (self.vy * -0.24).min(0.0);
            self.vx *= 0.68;
        }
        self.time > 0.0
    }

    pub fn image<'a>(
        &self,
        art: &'a [OverlayImage],
        scale: f32,
    ) -> Option<(i32, i32, &'a OverlayImage)> {
        let image = art.get(self.kind)?;
        Some((
            (self.at.x * scale - image.width as f32 * 0.5).round() as i32,
            (self.at.y * scale - image.height as f32 * 0.5).round() as i32,
            image,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn gentle_drag_and_a_paused_flick_keep_the_snowman() {
        let t = Instant::now();
        let mut grab = Grab::new(Point::from_xy(110.0, 450.0), 100.0, t);
        assert_eq!(
            grab.move_to(Point::from_xy(127.0, 445.0), t + Duration::from_millis(100)),
            117.0
        );
        assert!(grab.throw(t + Duration::from_millis(100)).is_none());
        grab.move_to(Point::from_xy(240.0, 370.0), t + Duration::from_millis(155));
        assert!(grab.throw(t + Duration::from_millis(155)).is_some());
        assert!(grab.throw(t + Duration::from_millis(260)).is_none());
    }
}
