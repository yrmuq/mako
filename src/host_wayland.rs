// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, registry_handlers,
};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface};
use wayland_client::{Connection, QueueHandle};

use crate::app::{startup_room, App, Pointer};
use crate::Fault;

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;

pub fn run() -> Result<(), Fault> {
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
    layer.set_anchor(Anchor::TOP | Anchor::LEFT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(200, 270);
    layer.commit();
    let pool = SlotPool::new(200 * 270 * 4, &shm).map_err(|error| {
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
        layer,
        app: App::new(startup_room(1920.0, 1080.0)),
        pointer: None,
        configured: false,
        last_time: 0,
        output_ready: false,
    };
    while !state.app.exit {
        queue.blocking_dispatch(&mut state).map_err(|error| {
            Fault::new(
                "MAKO-WL-0001",
                format!("The Wayland connection failed: {error}"),
            )
        })?;
    }
    Ok(())
}

struct Host {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    app: App,
    pointer: Option<wl_pointer::WlPointer>,
    configured: bool,
    last_time: u32,
    output_ready: bool,
}

impl Host {
    fn refresh_output(&mut self) {
        let mut found = None;
        for output in self.output_state.outputs() {
            if let Some(info) = self.output_state.info(&output) {
                let (width, height) = info.logical_size.unwrap_or((1280, 720));
                let scale = info.scale_factor.max(1) as u32;
                found = Some((width as f32, height as f32, scale));
                break;
            }
        }
        if let Some((width, height, scale)) = found {
            self.app.set_room(startup_room(width, height));
            self.app.scale = scale;
            self.output_ready = true;
        }
    }

    fn present(&mut self, qh: &QueueHandle<Self>) {
        self.layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        self.layer
            .set_margin(self.app.origin_y, 0, 0, self.app.origin_x);
        self.layer.set_size(self.app.logical_w, self.app.logical_h);
        let frame = self.app.render();
        let stride = frame.width as i32 * 4;
        let (buffer, canvas) = self
            .pool
            .create_buffer(
                frame.width as i32,
                frame.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("MAKO-WL-0003: Could not allocate a frame");
        canvas.copy_from_slice(&frame.bgra);
        let surface = self.layer.wl_surface();
        surface.set_buffer_scale(self.app.scale as i32);
        surface.damage_buffer(0, 0, frame.width as i32, frame.height as i32);
        surface.frame(qh, surface.clone());
        buffer
            .attach_to(surface)
            .expect("MAKO-WL-0003: Could not attach a frame");
        self.layer.commit();
        self.app.note_committed();
    }
}

impl CompositorHandler for Host {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        scale: i32,
    ) {
        self.app.scale = scale.max(1) as u32;
        if self.configured {
            self.present(qh);
        }
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
        self.app.tick(dt.clamp(0.0, 0.05));
        self.present(qh);
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Host {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_output();
        if self.configured {
            self.present(qh);
        }
    }

    fn update_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_output();
        if self.configured {
            self.present(qh);
        }
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Host {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.app.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if !self.output_ready {
            self.refresh_output();
        }
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
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.layer.wl_surface() {
                continue;
            }
            let (x, y) = (event.position.0 as f32, event.position.1 as f32);
            let kind = match event.kind {
                PointerEventKind::Motion { .. } => Some(Pointer::Motion),
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
                    Some(Pointer::LeftDown)
                }
                PointerEventKind::Press { button, .. } if button == BTN_RIGHT => {
                    Some(Pointer::RightDown)
                }
                PointerEventKind::Release { button, .. } if button == BTN_LEFT => {
                    Some(Pointer::LeftUp)
                }
                _ => None,
            };
            if let Some(kind) = kind {
                self.app.pointer(x, y, kind);
                self.present(qh);
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
