// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::io::Write;
use std::sync::mpsc::{self, Sender, SyncSender};
use std::time::Duration;

use slint::winit_030::winit::{dpi::PhysicalPosition, window::Window};
use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
use zbus::blocking::{connection::Builder, Connection as Bus, Proxy};

const SERVICE: &str = "org.kde.KWin";
const LEGACY_SCRIPT: &str = "mako-tray-placement";
const PLACER: &str = "/mako/Placer";

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
pub struct FullscreenOutput {
    pub name: String,
    pub x: i32,
    pub y: i32,
}

struct Placer {
    ready: SyncSender<()>,
    fullscreen: Sender<Vec<FullscreenOutput>>,
}

#[zbus::interface(name = "io.github.yrmuq.mako.Placer")]
impl Placer {
    fn ready(&self) {
        let _ = self.ready.try_send(());
    }

    fn fullscreen(&self, outputs: &str) {
        if let Ok(outputs) = serde_json::from_str(outputs) {
            let _ = self.fullscreen.send(outputs);
        }
    }
}

fn ready_connection(
    ready: SyncSender<()>,
    fullscreen: Sender<Vec<FullscreenOutput>>,
) -> zbus::Result<Bus> {
    Builder::session()?
        .serve_at(PLACER, Placer { ready, fullscreen })?
        .build()
}

pub struct Kwin {
    bus: Bus,
    plugin: String,
    _script: tempfile::NamedTempFile,
}

impl Kwin {
    pub fn install(
        fullscreen: Sender<Vec<FullscreenOutput>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (ready, registered) = mpsc::sync_channel(1);
        let bus = ready_connection(ready, fullscreen)?;
        let destination = bus.unique_name().ok_or("missing D-Bus unique name")?;
        let destination = serde_json::to_string(destination.as_str())?;
        let mut script = tempfile::Builder::new()
            .prefix("mako-tray-")
            .suffix(".js")
            .tempfile()?;
        writeln!(script, "const makoReadyBus = {destination};")?;
        script.write_all(include_bytes!("../ui/place.js"))?;
        let scripting = Proxy::new(&bus, SERVICE, "/Scripting", "org.kde.kwin.Scripting")?;
        let name = script.path().to_str().ok_or("invalid KWin script path")?;
        let plugin = script
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("invalid KWin script name")?
            .to_owned();
        let _ = scripting.call::<_, _, bool>("unloadScript", &(LEGACY_SCRIPT,));
        let id: i32 = scripting.call("loadScript", &(name, plugin.as_str()))?;
        if id < 0 {
            return Err("KWin rejected the positioning script".into());
        }
        let installed = Self {
            bus: bus.clone(),
            plugin,
            _script: script,
        };
        let path = format!("/Scripting/Script{id}");
        let object = Proxy::new(&bus, SERVICE, path.as_str(), "org.kde.kwin.Script")?;
        if object.call::<_, _, ()>("run", &()).is_err() {
            Proxy::new(&bus, SERVICE, format!("/{id}"), "org.kde.kwin.Script")?
                .call::<_, _, ()>("run", &())?;
        }
        registered
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "KWin did not initialize the tray positioning script")?;
        Ok(installed)
    }
}

impl Drop for Kwin {
    fn drop(&mut self) {
        if let Ok(scripting) =
            Proxy::new(&self.bus, SERVICE, "/Scripting", "org.kde.kwin.Scripting")
        {
            let _ = scripting.call::<_, _, bool>("unloadScript", &(self.plugin.as_str(),));
        }
    }
}

fn cursor_position() -> Option<(i32, i32)> {
    let (conn, screen) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots.get(screen)?.root;
    let pointer = conn.query_pointer(root).ok()?.reply().ok()?;
    Some((i32::from(pointer.root_x), i32::from(pointer.root_y)))
}

pub fn x11(window: &Window, tray_point: (i32, i32)) {
    let (x, y) = if tray_point == (0, 0) {
        cursor_position().unwrap_or(tray_point)
    } else {
        tray_point
    };
    let Some(monitor) = window
        .available_monitors()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            x >= position.x
                && y >= position.y
                && x < position.x + size.width as i32
                && y < position.y + size.height as i32
        })
        .or_else(|| window.current_monitor())
    else {
        return;
    };
    let size = window.outer_size();
    let origin = monitor.position();
    let screen = monitor.size();
    let left = origin.x + 4;
    let right = (origin.x + screen.width as i32 - size.width as i32 - 4).max(left);
    let top = origin.y + 4;
    let bottom = (origin.y + screen.height as i32 - size.height as i32 - 4).max(top);
    let edge_padding = 22;
    let y = if y > origin.y + screen.height as i32 / 2 {
        y - size.height as i32 - edge_padding
    } else {
        y + edge_padding
    };
    let x = (x - size.width as i32 / 2).clamp(left, right);
    window.set_outer_position(PhysicalPosition::new(x, y.clamp(top, bottom)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a private D-Bus session"]
    fn placement_callback_runs_without_an_application_runtime() {
        let (ready, registered) = mpsc::sync_channel(1);
        let (fullscreen, updates) = mpsc::channel();
        let service = ready_connection(ready, fullscreen).expect("start the placement callback");
        let caller = Bus::session().expect("connect as the placement script");
        let proxy = Proxy::new(
            &caller,
            service.unique_name().unwrap().as_str(),
            PLACER,
            "io.github.yrmuq.mako.Placer",
        )
        .unwrap();

        proxy.call::<_, _, ()>("Ready", &()).unwrap();
        registered.recv_timeout(Duration::from_secs(3)).unwrap();
        proxy
            .call::<_, _, ()>("Fullscreen", &("[{\"name\":\"DP-1\",\"x\":1920,\"y\":0}]",))
            .unwrap();
        assert_eq!(
            updates.recv_timeout(Duration::from_secs(3)).unwrap(),
            [FullscreenOutput {
                name: "DP-1".into(),
                x: 1920,
                y: 0
            }]
        );
    }
}
