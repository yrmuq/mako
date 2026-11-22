// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::thread;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ColormapAlloc, ConfigureWindowAux, ConnectionExt,
    CreateGCAux, CreateWindowAux, EventMask, ImageFormat, PropMode, Screen, VisualClass,
    WindowClass,
};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;

use crate::app::{startup_room, App, Pointer};
use crate::Fault;

pub fn run() -> Result<(), Fault> {
    let (conn, screen_number) = x11rb::connect(None).map_err(|error| {
        Fault::new(
            "MAKO-X11-0001",
            format!("Could not connect to X11: {error}"),
        )
    })?;
    let screen = &conn.setup().roots[screen_number];
    let (visual, depth) = argb_visual(screen)
        .ok_or_else(|| Fault::new("MAKO-X11-0002", "The X11 screen has no 32-bit color visual"))?;
    let colormap = conn.generate_id().map_err(x11)?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual)
        .map_err(x11)?;
    let room = work_area(&conn, screen).unwrap_or_else(|| {
        startup_room(
            screen.width_in_pixels as f32,
            screen.height_in_pixels as f32,
        )
    });
    let mut app = App::new(room);
    let window = conn.generate_id().map_err(x11)?;
    let gc = conn.generate_id().map_err(x11)?;
    conn.create_window(
        depth,
        window,
        screen.root,
        app.origin_x as i16,
        app.origin_y as i16,
        app.logical_w as u16,
        app.logical_h as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new()
            .colormap(colormap)
            .background_pixmap(x11rb::NONE)
            .border_pixel(0)
            .override_redirect(1)
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::STRUCTURE_NOTIFY,
            ),
    )
    .map_err(x11)?;
    conn.create_gc(gc, window, &CreateGCAux::new().foreground(0))
        .map_err(x11)?;
    set_title(&conn, window)?;
    conn.map_window(window).map_err(x11)?;
    conn.flush().map_err(x11)?;
    let mut last = Instant::now();
    while !app.exit {
        while let Some(event) = conn.poll_for_event().map_err(x11)? {
            handle(&mut app, event);
        }
        let now = Instant::now();
        let dt = now.saturating_duration_since(last).as_secs_f32();
        last = now;
        app.tick(dt);
        present(&conn, window, gc, &mut app)?;
        conn.flush().map_err(x11)?;
        thread::sleep(Duration::from_millis(16));
    }
    let _ = conn.destroy_window(window);
    let _ = conn.flush();
    Ok(())
}

fn present(conn: &impl Connection, window: u32, gc: u32, app: &mut App) -> Result<(), Fault> {
    app.prepare();
    let frame = app.render();
    conn.configure_window(
        window,
        &ConfigureWindowAux::new()
            .x(app.origin_x)
            .y(app.origin_y)
            .width(frame.width)
            .height(frame.height),
    )
    .map_err(x11)?;
    // PutImage rejects a request larger than the server maximum. The pet frame stays under it.
    conn.put_image(
        ImageFormat::Z_PIXMAP,
        window,
        gc,
        frame.width as u16,
        frame.height as u16,
        0,
        0,
        0,
        32,
        &frame.bgra,
    )
    .map_err(x11)?;
    app.note_committed();
    Ok(())
}

fn handle(app: &mut App, event: Event) {
    match event {
        Event::ButtonPress(press) => {
            let kind = match press.detail {
                1 => Pointer::LeftDown,
                3 => Pointer::RightDown,
                _ => return,
            };
            app.pointer(press.event_x as f32, press.event_y as f32, kind);
        }
        Event::ButtonRelease(release) if release.detail == 1 => {
            app.pointer(
                release.event_x as f32,
                release.event_y as f32,
                Pointer::LeftUp,
            );
        }
        Event::MotionNotify(motion) => {
            app.pointer(
                motion.event_x as f32,
                motion.event_y as f32,
                Pointer::Motion,
            );
        }
        Event::KeyPress(_) => app.key_escape(),
        _ => {}
    }
}

fn work_area(conn: &impl Connection, screen: &Screen) -> Option<crate::sim::Room> {
    let atom = conn
        .intern_atom(false, b"_NET_WORKAREA")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let reply = conn
        .get_property(false, screen.root, atom, AtomEnum::CARDINAL, 0, 16)
        .ok()?
        .reply()
        .ok()?;
    let values = reply.value32()?.collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }
    Some(startup_room(values[2] as f32, values[3] as f32))
}

fn set_title(conn: &impl Connection, window: u32) -> Result<(), Fault> {
    let utf8 = conn
        .intern_atom(false, b"UTF8_STRING")
        .map_err(x11)?
        .reply()
        .map_err(x11)?
        .atom;
    let name = conn
        .intern_atom(false, b"_NET_WM_NAME")
        .map_err(x11)?
        .reply()
        .map_err(x11)?
        .atom;
    let class = conn
        .intern_atom(false, b"WM_CLASS")
        .map_err(x11)?
        .reply()
        .map_err(x11)?
        .atom;
    conn.change_property8(PropMode::REPLACE, window, name, utf8, b"mako")
        .map_err(x11)?;
    conn.change_property8(
        PropMode::REPLACE,
        window,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        b"mako",
    )
    .map_err(x11)?;
    let mut class_bytes = b"mako".to_vec();
    class_bytes.push(0);
    class_bytes.extend_from_slice(b"mako");
    class_bytes.push(0);
    conn.change_property8(
        PropMode::REPLACE,
        window,
        class,
        AtomEnum::STRING,
        &class_bytes,
    )
    .map_err(x11)?;
    conn.change_window_attributes(window, &ChangeWindowAttributesAux::new())
        .map_err(x11)?;
    Ok(())
}

fn argb_visual(screen: &Screen) -> Option<(u32, u8)> {
    for depth in &screen.allowed_depths {
        if depth.depth != 32 {
            continue;
        }
        for visual in &depth.visuals {
            if visual.class == VisualClass::TRUE_COLOR {
                return Some((visual.visual_id, depth.depth));
            }
        }
    }
    None
}

fn x11(error: impl std::fmt::Display) -> Fault {
    Fault::new("MAKO-X11-0001", format!("The X11 request failed: {error}"))
}
