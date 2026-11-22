// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod config;
mod draw;
mod host_wayland;
mod host_x11;
mod i18n;
mod sim;

use std::env;
use std::fmt;

pub use app::startup_room;

#[derive(Debug)]
pub struct Fault {
    pub code: &'static str,
    message: String,
}

impl Fault {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

pub fn run() -> Result<(), Fault> {
    let mut args = env::args().skip(1);
    if args.any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "mako — a desktop pet for KDE Plasma\n\n\
             bear girl walks along the bottom edge of the screen.\n\
             Drag with the left button. Right-click opens language, theme and quit.\n\n\
             --help    Show this help"
        );
        return Ok(());
    }
    match session()? {
        Session::Wayland => host_wayland::run(),
        Session::X11 => host_x11::run(),
    }
}

enum Session {
    Wayland,
    X11,
}

fn session() -> Result<Session, Fault> {
    let kind = env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let wayland = env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = env::var_os("DISPLAY").is_some();
    if kind == "wayland" || (wayland && kind != "x11") {
        if !wayland {
            return Err(Fault::new(
                "MAKO-SESS-0001",
                "This Wayland session has no WAYLAND_DISPLAY, and X11 will not be used",
            ));
        }
        return Ok(Session::Wayland);
    }
    if kind == "x11" || x11 {
        return Ok(Session::X11);
    }
    Err(Fault::new(
        "MAKO-SESS-0002",
        "No Wayland or X11 session is available",
    ))
}
