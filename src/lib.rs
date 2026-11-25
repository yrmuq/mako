// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

mod about;
mod app;
mod config;
mod desktop;
mod draw;
mod festive;
mod game;
mod host_wayland;
mod host_x11;
mod i18n;
mod icons;
mod interactions;
mod pack;
mod place;
mod props;
mod rag;
mod ramps;
mod season;
mod sim;
mod sled;
mod snowman;
mod updates;
mod visitor_art;
mod visitors;
mod weather;
mod world;
mod x11_input;

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
    let args: Vec<_> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("mako — a desktop pet for KDE Plasma\n\nDrag with the left button and release to throw. Open settings from the tray.\n\n  --settings  Open settings at startup\n  --help      Show this help");
        return Ok(());
    }
    desktop::run(session()?, args.iter().any(|arg| arg == "--settings"))
}

#[derive(Clone, Copy)]
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
