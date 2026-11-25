// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, registry_handlers,
};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, QueueHandle};

use crate::app::{startup_room, Pointer};
use crate::config::Settings;
use crate::game::{Command, Game, Startup};
use crate::place::FullscreenOutput;
use crate::Fault;
use std::sync::mpsc::Receiver;

const BTN_LEFT: u32 = 0x110;

pub fn run(
    settings: Settings,
    startup: Startup,
    commands: Receiver<Command>,
    fullscreen_updates: Receiver<Vec<FullscreenOutput>>,
) -> Result<(), Fault> {
    let conn = Connection::connect_to_env().map_err(|error| {
        Fault::new(
            "MAKO-WL-0001",
            format!("Could not connect to Wayland: {error}"),
        )
    })?;
    let (globals, mut queue) = registry_queue_init(&conn).map_err(|error| {
        Fault::new(
            "MAKO-WL-0001",
            format!("Could not read Wayland globals: {error}"),
        )
    })?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)
        .map_err(|_| Fault::new("MAKO-WL-0001", "The compositor did not offer wl_compositor"))?;
    let layer_shell = LayerShell::bind(&globals, &qh).map_err(|_| {
        Fault::new(
            "MAKO-WL-0002",
            "zwlr_layer_shell_v1 is unavailable, so the pet cannot open on this Wayland session",
        )
    })?;
    let shm = Shm::bind(&globals, &qh)
        .map_err(|_| Fault::new("MAKO-WL-0001", "The compositor did not offer wl_shm"))?;
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("mako"), None);
    // One anchored surface. Moving it under a held pointer makes KWin drop the connection.
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(0, 0);
    let region = Region::new(&compositor)
        .map_err(|_| Fault::new("MAKO-WL-0003", "Could not limit pointer input to the pet"))?;
    layer
        .wl_surface()
        .set_input_region(Some(region.wl_region()));
    layer.commit();
    drop(region);
    let pool = SlotPool::new(1920 * 1080 * 4, &shm).map_err(|error| {
        Fault::new(
            "MAKO-WL-0003",
            format!("Could not create a shared buffer: {error}"),
        )
    })?;
    let mut state = Host {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        frames: Vec::new(),
        frame_px: None,
        shown: Vec::new(),
        frame_requested: false,
        needs_full_damage: true,
        layer,
        compositor,
        game: Game::new(startup_room(1920.0, 1080.0), settings, startup, commands),
        pointer: None,
        configured: false,
        last_time: 0,
        output_ready: false,
        screen: None,
        output: None,
        fullscreen: Vec::new(),
        fullscreen_updates,
    };
    use rustix::event::{poll, PollFd, PollFlags, Timespec};
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: 16_000_000,
    };
    while !state.game.exit {
        let mut changed = state.game.commands();
        if let Some(outputs) = state.fullscreen_updates.try_iter().last() {
            state.fullscreen = outputs;
            changed = true;
        }
        if changed {
            let accepts_input = state.accepts_input();
            state.game.cancel_grab(|_, _| accepts_input);
        }
        if state.configured && !state.frame_requested {
            if let Some((width, _)) = state.screen {
                state.game.tick_snow(0.0, width);
            }
        }
        if (changed || state.game.wants_frames()) && state.configured && !state.frame_requested {
            state.present(&qh);
        }
        queue.dispatch_pending(&mut state).map_err(dispatch_error)?;
        queue.flush().map_err(dispatch_error)?;
        if let Some(read) = queue.prepare_read() {
            let socket = read.connection_fd();
            let mut fd = [PollFd::new(&socket, PollFlags::IN)];
            match poll(&mut fd, Some(&timeout)) {
                Ok(0) | Err(rustix::io::Errno::INTR) => {}
                Ok(_) => {
                    read.read().map_err(dispatch_error)?;
                }
                Err(error) => return Err(dispatch_error(error)),
            }
        }
    }
    Ok(())
}

fn dispatch_error(error: impl std::fmt::Display) -> Fault {
    Fault::new(
        "MAKO-WL-0004",
        format!("The Wayland connection failed: {error}"),
    )
}

struct Host {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    frames: Vec<Buffer>,
    frame_px: Option<(i32, i32)>,
    shown: Vec<PxRect>,
    frame_requested: bool,
    needs_full_damage: bool,
    layer: LayerSurface,
    compositor: CompositorState,
    game: Game,
    pointer: Option<wl_pointer::WlPointer>,
    configured: bool,
    last_time: u32,
    output_ready: bool,
    screen: Option<(u32, u32)>,
    output: Option<wl_output::WlOutput>,
    fullscreen: Vec<FullscreenOutput>,
    fullscreen_updates: Receiver<Vec<FullscreenOutput>>,
}

impl Host {
    fn accepts_input(&self) -> bool {
        let fullscreen = self
            .output
            .as_ref()
            .and_then(|output| self.output_state.info(output))
            .is_some_and(|info| {
                self.fullscreen
                    .iter()
                    .any(|output| match (&info.name, output.name.is_empty()) {
                        (Some(name), false) => *name == output.name,
                        _ => info.logical_position == Some((output.x, output.y)),
                    })
            });
        self.game.input_mode().accepts_input(fullscreen)
    }

    fn refresh_output(&mut self) {
        let found = self
            .output
            .as_ref()
            .and_then(|output| self.output_state.info(output))
            .or_else(|| {
                self.output_state
                    .outputs()
                    .find_map(|output| self.output_state.info(&output))
            });
        if let Some(info) = found {
            self.game.set_scale(info.scale_factor.max(1) as u32);
            self.output_ready = true;
            if self.screen.is_none() {
                let (width, height) = info.logical_size.unwrap_or((1280, 720));
                self.note_screen(width as u32, height as u32);
            }
        }
    }

    fn note_screen(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || self.screen == Some((width, height)) {
            return;
        }
        self.screen = Some((width, height));
        self.game.set_room(
            startup_room(width as f32, height as f32),
            crate::config::bottom_panel(),
        );
    }

    fn present(&mut self, qh: &QueueHandle<Self>) {
        let Some((screen_w, screen_h)) = self.screen else {
            let surface = self.layer.wl_surface();
            surface.frame(qh, surface.clone());
            self.frame_requested = true;
            self.layer.commit();
            return;
        };
        let scale = self.game.scale().max(1);
        let buf_w = (screen_w as i32).saturating_mul(scale as i32);
        let buf_h = (screen_h as i32).saturating_mul(scale as i32);
        let Some(stride) = buf_w.checked_mul(4) else {
            let surface = self.layer.wl_surface();
            surface.frame(qh, surface.clone());
            self.frame_requested = true;
            self.layer.commit();
            return;
        };
        // A new full-screen wl_buffer every frame makes KWin keep a GPU texture.
        self.ensure_frames(buf_w, buf_h, stride);
        let Some(index) = self
            .frames
            .iter()
            .position(|frame| !frame.slot().has_active_buffers())
        else {
            let surface = self.layer.wl_surface();
            surface.frame(qh, surface.clone());
            self.frame_requested = true;
            self.layer.commit();
            return;
        };
        let surface = self.layer.wl_surface();
        let accepts_input = self.accepts_input();
        let region = Region::new(&self.compositor)
            .expect("MAKO-WL-0003: Could not limit pointer input to the pet");
        if accepts_input {
            for bounds in self.game.prop_input_bounds() {
                let x = bounds.left().floor() as i32;
                let y = bounds.top().floor() as i32;
                region.add(
                    x,
                    y,
                    bounds.right().ceil() as i32 - x,
                    bounds.bottom().ceil() as i32 - y,
                );
            }
        }
        let mut next = Vec::with_capacity(self.game.pets.len());
        let canvas = self.frames[index]
            .canvas(&mut self.pool)
            .expect("MAKO-WL-0003: Could not draw a frame");
        canvas.fill(0);
        for (x, y, image) in self.game.overlay_images() {
            blit(
                canvas,
                buf_w as u32,
                buf_h as u32,
                &image.bgra,
                image.width,
                image.height,
                x,
                y,
            );
            next.push(PxRect {
                x,
                y,
                w: image.width as i32,
                h: image.height as i32,
            });
        }
        for pet in &mut self.game.pets {
            let app = &mut pet.app;
            app.prepare();
            let frame = app.render();
            let rect = PxRect {
                x: app.origin_x.saturating_mul(scale as i32),
                y: app.origin_y.saturating_mul(scale as i32),
                w: frame.width as i32,
                h: frame.height as i32,
            };
            blit(
                canvas,
                buf_w as u32,
                buf_h as u32,
                &frame.bgra,
                frame.width,
                frame.height,
                rect.x,
                rect.y,
            );
            if accepts_input {
                for hit in frame.input_rects(scale) {
                    region.add(
                        app.origin_x + hit.x(),
                        app.origin_y + hit.y(),
                        hit.width() as i32,
                        hit.height() as i32,
                    );
                }
            }
            next.push(rect);
            app.note_committed();
        }
        surface.set_input_region(Some(region.wl_region()));
        surface.set_buffer_scale(scale as i32);
        for damage in damages(self.needs_full_damage, buf_w, buf_h, &self.shown, &next) {
            surface.damage_buffer(damage.x, damage.y, damage.w, damage.h);
        }
        self.needs_full_damage = false;
        self.frame_requested = !next.is_empty() || self.game.wants_frames();
        if self.frame_requested {
            surface.frame(qh, surface.clone());
        }
        self.frames[index]
            .attach_to(surface)
            .expect("MAKO-WL-0003: Could not attach a frame");
        self.layer.commit();
        drop(region);
        self.shown = next;
    }

    fn ensure_frames(&mut self, width: i32, height: i32, stride: i32) {
        if self.frame_px == Some((width, height)) && self.frames.len() == 2 {
            return;
        }
        self.frames.clear();
        self.frame_px = Some((width, height));
        self.shown.clear();
        self.needs_full_damage = true;
        let bytes = (height as usize).saturating_mul(stride as usize);
        for _ in 0..2 {
            let slot = self
                .pool
                .new_slot(bytes)
                .expect("MAKO-WL-0003: Could not allocate a frame");
            self.pool.raw_data_mut(&slot).fill(0);
            let buffer = self
                .pool
                .create_buffer_in(&slot, width, height, stride, wl_shm::Format::Argb8888)
                .expect("MAKO-WL-0003: Could not allocate a frame");
            self.frames.push(buffer);
        }
    }
}

impl CompositorHandler for Host {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        scale: i32,
    ) {
        self.game.set_scale(scale.max(1) as u32);
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        time: u32,
    ) {
        let dt = if self.last_time == 0 {
            1.0 / 60.0
        } else {
            time.wrapping_sub(self.last_time) as f32 / 1000.0
        };
        self.last_time = time;
        self.frame_requested = false;
        let accepts_input = self.accepts_input();
        self.game.cancel_grab(|_, _| accepts_input);
        if let Some((width, _)) = self.screen {
            self.game.tick_snow(dt, width);
        }
        self.game.tick(dt);
        self.present(qh);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        self.output = Some(output.clone());
        self.refresh_output();
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        if self.output.as_ref() == Some(output) {
            self.output = None;
            self.refresh_output();
        }
    }
}

impl OutputHandler for Host {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_output();
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_output();
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Host {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.game.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if !self.output_ready {
            self.refresh_output();
        }
        let (width, height) = configure.new_size;
        self.note_screen(width, height);
        let first = !self.configured;
        self.configured = true;
        if first {
            self.present(qh);
        }
    }
}

impl SeatHandler for Host {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = Some(self.seat_state.get_pointer(qh, &seat).expect("pointer"));
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Host {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        if !self.accepts_input() {
            return;
        }
        for event in events {
            if &event.surface != self.layer.wl_surface() {
                continue;
            }
            let kind = match event.kind {
                PointerEventKind::Motion { .. } => Some(Pointer::Motion),
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
                    Some(Pointer::LeftDown)
                }
                PointerEventKind::Release { button, .. } if button == BTN_LEFT => {
                    Some(Pointer::LeftUp)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                self.game
                    .pointer(event.position.0 as f32, event.position.1 as f32, kind);
            }
        }
    }
}

impl ShmHandler for Host {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(Host);
delegate_output!(Host);
delegate_shm!(Host);
delegate_seat!(Host);
delegate_pointer!(Host);
delegate_layer!(Host);
delegate_registry!(Host);

impl ProvidesRegistryState for Host {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PxRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

fn clip_rect(rect: PxRect, buf_w: i32, buf_h: i32) -> Option<PxRect> {
    if rect.w <= 0 || rect.h <= 0 || buf_w <= 0 || buf_h <= 0 {
        return None;
    }
    let x0 = rect.x.max(0);
    let y0 = rect.y.max(0);
    let x1 = rect.x.saturating_add(rect.w).min(buf_w);
    let y1 = rect.y.saturating_add(rect.h).min(buf_h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(PxRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

fn damages(
    full: bool,
    width: i32,
    height: i32,
    before: &[PxRect],
    after: &[PxRect],
) -> Vec<PxRect> {
    if full {
        return vec![PxRect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        }];
    }
    let mut areas = Vec::new();
    for rect in before
        .iter()
        .chain(after)
        .filter_map(|rect| clip_rect(*rect, width, height))
    {
        if !areas.contains(&rect) {
            areas.push(rect);
        }
    }
    areas
}

#[allow(clippy::too_many_arguments)]
fn blit(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
    x: i32,
    y: i32,
) {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return;
    }
    let dst_stride = dst_w as usize * 4;
    let src_stride = src_w as usize * 4;
    for row in 0..src_h as i32 {
        let dy = y + row;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        let mut src_x = 0i32;
        let mut dst_x = x;
        let mut len = src_w as i32;
        if dst_x < 0 {
            src_x -= dst_x;
            len += dst_x;
            dst_x = 0;
        }
        if dst_x + len > dst_w as i32 {
            len = dst_w as i32 - dst_x;
        }
        if len <= 0 {
            continue;
        }
        let dst_off = dy as usize * dst_stride + dst_x as usize * 4;
        let src_off = row as usize * src_stride + src_x as usize * 4;
        let bytes = len as usize * 4;
        let target = dst[dst_off..dst_off + bytes].as_chunks_mut::<4>().0;
        let pixels = src[src_off..src_off + bytes].as_chunks::<4>().0;
        for (target, pixel) in target.iter_mut().zip(pixels) {
            if pixel[3] == 255 || target[3] == 0 {
                *target = *pixel;
            } else if pixel[3] != 0 {
                let remainder = 255 - u16::from(pixel[3]);
                for (channel, above) in target.iter_mut().zip(pixel) {
                    *channel =
                        above.saturating_add(((u16::from(*channel) * remainder + 127) / 255) as u8);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blit, damages, PxRect};

    #[test]
    fn blit_clips_a_frame_to_the_screen() {
        let mut dst = vec![0u8; 8 * 4 * 4];
        let src = vec![9u8; 4 * 3 * 4];
        blit(&mut dst, 8, 4, &src, 4, 3, 6, -1);
        assert_eq!(dst[6 * 4], 9);
        assert_eq!(dst[5 * 4], 0);
        assert_eq!(dst[(2 * 8 + 6) * 4], 0);
    }

    #[test]
    fn a_moved_pet_damages_only_the_old_and_new_rect() {
        let prev = PxRect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        };
        let next = PxRect {
            x: 18,
            y: 20,
            w: 30,
            h: 40,
        };
        let damages = damages(false, 1920, 1080, &[prev], &[next]);
        assert_eq!(damages, vec![prev, next]);
        assert!(damages.iter().all(|rect| rect.w * rect.h < 1920 * 1080));
    }

    #[test]
    fn the_first_frame_damages_the_whole_buffer_once() {
        let next = PxRect {
            x: 4,
            y: 6,
            w: 20,
            h: 20,
        };
        let damages = damages(true, 800, 600, &[], &[next]);
        assert_eq!(
            damages,
            vec![PxRect {
                x: 0,
                y: 0,
                w: 800,
                h: 600
            }]
        );
    }

    #[test]
    fn damage_clips_a_pet_that_hangs_off_the_screen() {
        let next = PxRect {
            x: -10,
            y: 500,
            w: 40,
            h: 80,
        };
        let damages = damages(false, 100, 520, &[], &[next]);
        assert_eq!(
            damages,
            vec![PxRect {
                x: 0,
                y: 500,
                w: 30,
                h: 20
            }]
        );
    }
}
