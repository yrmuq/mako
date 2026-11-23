// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use ksni::blocking::TrayMethods;
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};

use crate::about::LogoTaps;
use crate::config::{LanguagePref, Settings, ThemePref};
use crate::game::Command;
use crate::i18n::{Catalog, ID_NAME, ID_QUIT};
use crate::pack::{self, Pack};
use crate::updates::{self, Status};
use crate::{Fault, Session};

slint::include_modules!();

thread_local! {
    static CREATING_POPUP: Cell<bool> = const { Cell::new(false) };
}

const POPUP: &str = "mako-popup";
const ICON: &[u8] = include_bytes!("../assets/icon.png");

enum Action {
    Toggle(i32, i32),
    Dismiss,
    Configure(bool),
    Pin,
    Quit,
    Add(String),
    Remove(String),
    Favorite(String, bool),
    Move(i32, i32),
    Reload,
    Import,
    Imported(Option<Result<Pack, Fault>>),
    Language(i32),
    Theme(i32),
    Scale(f32),
    Proportions(i32, i32),
    Logo,
    Check,
    Updated(Result<Option<String>, ()>),
    Link(String),
    HostFinished(Result<(), Fault>),
}

struct Tray {
    tx: Sender<Action>,
    labels: [String; 2],
    icon: Vec<ksni::Icon>,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "mako".into()
    }
    fn title(&self) -> String {
        "mako".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon.clone()
    }
    fn activate(&mut self, x: i32, y: i32) {
        let _ = self.tx.send(Action::Toggle(x, y));
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        self.labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                ksni::menu::StandardItem {
                    label: label.clone(),
                    activate: Box::new(move |this: &mut Self| {
                        let _ = this.tx.send(if index == 0 {
                            Action::Configure(false)
                        } else {
                            Action::Quit
                        });
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect()
    }
}

struct Entry {
    pack: Pack,
    face: Image,
}

impl Entry {
    fn new(pack: Pack) -> Self {
        let face = crate::draw::portrait(&pack)
            .or_else(|| tiny_skia::Pixmap::decode_png(ICON).ok())
            .map(|pixmap| {
                Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    pixmap.data(),
                    pixmap.width(),
                    pixmap.height(),
                ))
            })
            .unwrap_or_default();
        Self { pack, face }
    }
}

struct Desktop {
    action_tx: Sender<Action>,
    commands: Sender<Command>,
    session: Session,
    config: Settings,
    catalog: Catalog,
    entries: Vec<Entry>,
    counts: HashMap<String, i32>,
    tray: Option<ksni::blocking::Handle<Tray>>,
    popup: Option<Popup>,
    window: Option<SettingsWindow>,
    focus_lost: Rc<Cell<bool>>,
    shown: Instant,
    tray_point: (i32, i32),
    pinned: bool,
    taps: LogoTaps,
    updating: Status,
    importing: bool,
    import_result: Option<u32>,
    save_at: Option<Instant>,
    fault: Option<Fault>,
}

impl Desktop {
    fn handle(&mut self, action: Action) {
        match action {
            Action::Toggle(x, y) => {
                if self.popup.is_some() {
                    self.hide_popup();
                } else {
                    self.tray_point = (x, y);
                    if let Err(error) = self.show_popup() {
                        eprintln!("MAKO-UI-0002: Could not open the tray panel: {error}");
                    }
                }
            }
            Action::Dismiss => self.hide_popup(),
            Action::Configure(chibis) => {
                self.hide_popup();
                if let Err(error) = self.show_settings(chibis) {
                    eprintln!("MAKO-UI-0002: Could not open settings: {error}");
                }
            }
            Action::Pin => {
                self.pinned = !self.pinned;
                if let Some(popup) = &self.popup {
                    popup.set_pinned(self.pinned);
                    let theme = crate::icons::Theme::load();
                    if let Some(icon) = theme.icon("configure") {
                        popup.set_configure_icon(icon.image);
                        popup.set_tint_configure(icon.tint);
                    }
                    if let Some(icon) = theme.icon("window-pin") {
                        popup.set_pin_icon(icon.image);
                        popup.set_tint_pin(icon.tint);
                    }
                    if let Some(icon) = theme.icon("list-add") {
                        popup.set_add_icon(icon.image);
                        popup.set_tint_add(icon.tint);
                    }
                    if let Some(icon) = theme.icon("list-remove") {
                        popup.set_remove_icon(icon.image);
                        popup.set_tint_remove(icon.tint);
                    }
                }
            }
            Action::Quit => {
                let _ = slint::quit_event_loop();
            }
            Action::HostFinished(result) => {
                self.fault = result.err();
                let _ = slint::quit_event_loop();
            }
            Action::Add(id) => {
                if let Some(entry) = self.entries.iter().find(|entry| entry.pack.id == id) {
                    if self.commands.send(Command::Add(entry.pack.clone())).is_ok() {
                        *self.counts.entry(id).or_default() += 1;
                        self.refresh_rows();
                    }
                }
            }
            Action::Remove(id) => {
                if let Some(count) = self.counts.get_mut(&id) {
                    if *count > 0 && self.commands.send(Command::Remove(id)).is_ok() {
                        *count -= 1;
                        self.refresh_rows();
                    }
                }
            }
            Action::Favorite(id, add) => {
                if add && !self.config.favorites.contains(&id) {
                    self.config.favorites.push(id);
                } else if !add {
                    self.config.favorites.retain(|favorite| *favorite != id);
                }
                self.schedule_save();
                self.refresh_rows();
            }
            Action::Move(from, to) => {
                let visible: Vec<_> = self
                    .config
                    .favorites
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| self.entries.iter().any(|entry| &entry.pack.id == *id))
                    .map(|(index, _)| index)
                    .collect();
                if let (Some(&from), Some(&to)) =
                    (visible.get(from as usize), visible.get(to as usize))
                {
                    let id = self.config.favorites.remove(from);
                    self.config.favorites.insert(to, id);
                    self.schedule_save();
                    self.refresh_rows();
                }
            }
            Action::Reload => {
                self.entries = pack::installed().into_iter().map(Entry::new).collect();
                self.refresh_rows();
            }
            Action::Import => {
                if self.importing {
                    return;
                }
                self.import_result = None;
                self.importing = true;
                self.refresh_status();
                let tx = self.action_tx.clone();
                let label = self.catalog.text(40).to_owned();
                std::thread::spawn(move || {
                    let selected = rfd::FileDialog::new()
                        .add_filter(label, &["zip"])
                        .pick_file();
                    let result = selected.map(|path| pack::import(&path));
                    let _ = tx.send(Action::Imported(result));
                });
            }
            Action::Imported(result) => {
                self.importing = false;
                match result {
                    Some(Ok(pack)) => {
                        self.entries.push(Entry::new(pack));
                        self.import_result = Some(41);
                        self.refresh_rows();
                    }
                    Some(Err(error)) => {
                        eprintln!("{error}");
                        self.import_result = Some(42);
                    }
                    None => {}
                }
                self.refresh_status();
            }
            Action::Language(index) => {
                self.config.language = match index {
                    1 => LanguagePref::En,
                    2 => LanguagePref::Ru,
                    _ => LanguagePref::System,
                };
                self.catalog = Catalog::load(&self.config.code());
                let labels = self.tray_labels();
                if let Some(tray) = &self.tray {
                    tray.update(|tray| tray.labels = labels);
                }
                self.schedule_save();
                self.refresh_style();
                self.refresh_rows();
                self.refresh_status();
            }
            Action::Theme(index) => {
                self.config.theme = match index {
                    1 => ThemePref::Light,
                    2 => ThemePref::Dark,
                    _ => ThemePref::System,
                };
                self.schedule_save();
                self.refresh_style();
            }
            Action::Scale(percent) => {
                self.config.scale = (percent.clamp(20.0, 200.0) / 100.0).to_string();
                self.apply_dimensions();
            }
            Action::Proportions(width, height) => {
                self.config.width = width.clamp(25, 200) as u32;
                self.config.height = height.clamp(25, 200) as u32;
                self.apply_dimensions();
            }
            Action::Logo => {
                if self.taps.push(Instant::now()) {
                    if let Some(window) = &self.window {
                        let open = !window.get_dev_open();
                        window.set_dev_open(open);
                        window.set_tab(if open { 3 } else { 2 });
                    }
                }
            }
            Action::Check => {
                if !self.updating.can_check() {
                    return;
                }
                self.updating = Status::Checking;
                self.refresh_status();
                let tx = self.action_tx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Action::Updated(updates::check()));
                });
            }
            Action::Updated(result) => {
                self.updating = match result {
                    Ok(Some(version)) => Status::Available(version),
                    Ok(None) => Status::Current,
                    Err(()) => Status::Failed,
                };
                self.refresh_status();
            }
            Action::Link(url) => {
                std::thread::spawn(move || {
                    if let Err(error) = updates::open_link(&url) {
                        eprintln!("MAKO-UI-0005: Could not open a link: {error}");
                    }
                });
            }
        }
    }

    fn tick(&mut self) {
        if self.save_at.is_some_and(|when| Instant::now() >= when) {
            self.save();
        }
        if self.popup.is_some()
            && !self.pinned
            && self.focus_lost.get()
            && self.shown.elapsed() > Duration::from_millis(180)
        {
            self.hide_popup();
        }
    }

    fn schedule_save(&mut self) {
        self.save_at = Some(Instant::now() + Duration::from_millis(250));
    }

    fn save(&mut self) {
        self.save_at = None;
        if let Err(error) = self.config.save() {
            eprintln!("{error}");
        }
    }

    fn apply_dimensions(&mut self) {
        self.schedule_save();
        let _ = self.commands.send(Command::Appearance(self.config.clone()));
    }

    fn tray_labels(&self) -> [String; 2] {
        [
            self.catalog.text(16).into(),
            self.catalog.text(ID_QUIT).into(),
        ]
    }

    fn hide_popup(&mut self) {
        if let Some(popup) = self.popup.take() {
            let _ = popup.hide();
        }
    }

    fn show_popup(&mut self) -> Result<(), slint::PlatformError> {
        CREATING_POPUP.set(true);
        let result = self.create_popup();
        CREATING_POPUP.set(false);
        result
    }

    fn create_popup(&mut self) -> Result<(), slint::PlatformError> {
        let popup = Popup::new()?;
        popup.set_pinned(self.pinned);
        let tx = self.action_tx.clone();
        popup.on_configure(move |chibis| {
            let _ = tx.send(Action::Configure(chibis));
        });
        let tx = self.action_tx.clone();
        popup.on_pin_toggled(move || {
            let _ = tx.send(Action::Pin);
        });
        let tx = self.action_tx.clone();
        popup.on_add_instance(move |id| {
            let _ = tx.send(Action::Add(id.to_string()));
        });
        let tx = self.action_tx.clone();
        popup.on_remove_instance(move |id| {
            let _ = tx.send(Action::Remove(id.to_string()));
        });
        let focused = self.focus_lost.clone();
        focused.set(false);
        let seen_focus = Cell::new(false);
        let tx = self.action_tx.clone();
        popup.window().on_winit_window_event(move |_, event| {
            if let winit::event::WindowEvent::Focused(active) = event {
                if *active {
                    seen_focus.set(true);
                }
                focused.set(!*active && seen_focus.get());
            }
            if let winit::event::WindowEvent::KeyboardInput { event, .. } = event {
                if event.state.is_pressed()
                    && event.logical_key == winit::keyboard::NamedKey::Escape
                {
                    let _ = tx.send(Action::Dismiss);
                    return EventResult::PreventDefault;
                }
            }
            EventResult::Propagate
        });
        let tx = self.action_tx.clone();
        popup.window().on_close_requested(move || {
            let _ = tx.send(Action::Dismiss);
            slint::CloseRequestResponse::HideWindow
        });
        self.popup = Some(popup.clone_strong());
        self.refresh_rows();
        self.refresh_style();
        self.shown = Instant::now();
        if let Err(error) = popup.show() {
            self.popup = None;
            return Err(error);
        }
        if matches!(self.session, Session::X11) {
            let point = self.tray_point;
            popup
                .window()
                .with_winit_window(|window| crate::place::x11(window, point));
        }
        Ok(())
    }

    fn show_settings(&mut self, chibis: bool) -> Result<(), slint::PlatformError> {
        if self.window.is_none() {
            let window = SettingsWindow::new()?;
            let tx = &self.action_tx;
            macro_rules! connect {
                ($callback:ident, $action:expr) => {{
                    let tx = tx.clone();
                    window.$callback(move || {
                        let _ = tx.send($action);
                    });
                }};
                ($callback:ident, $action:expr, $value:ident) => {{
                    let tx = tx.clone();
                    window.$callback(move |$value| {
                        let _ = tx.send($action);
                    });
                }};
                ($callback:ident, $action:expr, $left:ident, $right:ident) => {{
                    let tx = tx.clone();
                    window.$callback(move |$left, $right| {
                        let _ = tx.send($action);
                    });
                }};
            }
            connect!(on_language_selected, Action::Language(index), index);
            connect!(on_theme_selected, Action::Theme(index), index);
            connect!(on_size_selected, Action::Scale(value), value);
            connect!(
                on_proportions_selected,
                Action::Proportions(width, height),
                width,
                height
            );
            connect!(on_add_favorite, Action::Favorite(id.to_string(), true), id);
            connect!(
                on_remove_favorite,
                Action::Favorite(id.to_string(), false),
                id
            );
            connect!(on_move_favorite, Action::Move(from, to), from, to);
            connect!(on_refresh_installed, Action::Reload);
            connect!(on_import_zip, Action::Import);
            connect!(on_logo_tapped, Action::Logo);
            connect!(on_check_update, Action::Check);
            connect!(on_open_link, Action::Link(url.to_string()), url);
            let tx = tx.clone();
            let quit_on_close = self.tray.is_none();
            window.window().on_close_requested(move || {
                if quit_on_close {
                    let _ = tx.send(Action::Quit);
                }
                slint::CloseRequestResponse::HideWindow
            });
            window.set_version(env!("CARGO_PKG_VERSION").into());
            window.set_parts(model(
                crate::about::parts()
                    .into_iter()
                    .map(|part| Attribution {
                        name: part.name.into(),
                        upstream: part.upstream.into(),
                        detail: part.detail.into(),
                        url: part.url.into(),
                    })
                    .collect(),
            ));
            self.window = Some(window);
        }
        let window = self.window.as_ref().expect("settings window");
        if chibis {
            window.set_tab(1);
        }
        window.set_language_index(match self.config.language {
            LanguagePref::System => 0,
            LanguagePref::En => 1,
            LanguagePref::Ru => 2,
        });
        window.set_theme_index(match self.config.theme {
            ThemePref::System => 0,
            ThemePref::Light => 1,
            ThemePref::Dark => 2,
        });
        window.set_size_value(self.config.pet_scale() * 100.0);
        window.set_width_value(self.config.width as i32);
        window.set_height_value(self.config.height as i32);
        self.refresh_style();
        self.refresh_rows();
        self.refresh_status();
        window.show()?;
        window
            .window()
            .with_winit_window(|native| native.focus_window());
        Ok(())
    }

    fn refresh_rows(&self) {
        let row = |entry: &Entry| {
            let id = &entry.pack.id;
            ChibiRow {
                id: id.into(),
                name: if id == pack::BUILTIN_ID {
                    self.catalog.text(ID_NAME).into()
                } else {
                    entry.pack.name.as_str().into()
                },
                face: entry.face.clone(),
                count: self.counts.get(id).copied().unwrap_or(0),
                added: self.config.favorites.contains(id),
            }
        };
        let rows: Vec<_> = self
            .config
            .favorites
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| entry.pack.id == *id))
            .map(row)
            .collect();
        if let Some(popup) = &self.popup {
            popup.set_rows(model(rows.clone()));
            if matches!(self.session, Session::X11) {
                let point = self.tray_point;
                popup
                    .window()
                    .with_winit_window(|window| crate::place::x11(window, point));
            }
        }
        if let Some(window) = &self.window {
            window.set_rows(model(rows));
            window.set_installed(model(self.entries.iter().map(row).collect()));
        }
    }

    fn refresh_style(&self) {
        let strings: ModelRc<SharedString> =
            model((0..=42).map(|id| self.catalog.text(id).into()).collect());
        let dark = self.config.dark();
        if let Some(popup) = &self.popup {
            popup.global::<Translations>().set_strings(strings.clone());
            style(&popup.global::<Breeze>(), dark);
            popup.set_dark(dark);
        }
        if let Some(window) = &self.window {
            window.global::<Translations>().set_strings(strings);
            style(&window.global::<Breeze>(), dark);
            window.set_dark(dark);
        }
    }

    fn refresh_status(&self) {
        let Some(window) = &self.window else { return };
        let status = match &self.updating {
            Status::Idle => String::new(),
            Status::Checking => self.catalog.text(36).into(),
            Status::Current => self.catalog.text(37).into(),
            Status::Failed => self.catalog.text(38).into(),
            Status::Available(version) => format!("{} {version}", self.catalog.text(39)),
        };
        window.set_update_status(status.into());
        window.set_update_available(matches!(self.updating, Status::Available(_)));
        window.set_checking(!self.updating.can_check());
        window.set_importing(self.importing);
        window.set_status(
            self.import_result
                .map(|id| self.catalog.text(id))
                .unwrap_or("")
                .into(),
        );
    }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::from(Rc::new(VecModel::from(items)))
}

fn style(breeze: &Breeze<'_>, dark: bool) {
    use slint::Color;
    let [window, button, hover, fg, inactive, frame] = if dark {
        [0x232629, 0x31363b, 0x3b4248, 0xfcfcfc, 0xa1a9b1, 0x4a5158]
    } else {
        [0xeff0f1, 0xfcfcfc, 0xf6f6f7, 0x232629, 0x707d8a, 0xc6c8c9]
    }
    .map(|rgb| Color::from_argb_encoded(0xff000000 | rgb));
    breeze.set_dark(dark);
    breeze.set_window(window);
    breeze.set_button(button);
    breeze.set_button_hover(hover);
    breeze.set_fg(fg);
    breeze.set_inactive(inactive);
    breeze.set_frame(frame);
}

fn window_attributes(
    attributes: winit::window::WindowAttributes,
) -> winit::window::WindowAttributes {
    use winit::platform::x11::{WindowAttributesExtX11, WindowType};
    let popup = CREATING_POPUP.get();
    let name = if popup { POPUP } else { "mako" };
    let attributes = WindowAttributesExtX11::with_name(attributes, name, name);
    let attributes =
        winit::platform::wayland::WindowAttributesExtWayland::with_name(attributes, name, name);
    let attributes = attributes
        .with_title(name)
        .with_resizable(!popup)
        .with_transparent(popup);
    if popup {
        attributes
            .with_decorations(false)
            .with_x11_window_type(vec![WindowType::PopupMenu])
    } else {
        attributes
    }
}

fn tray_icon() -> Vec<ksni::Icon> {
    let Ok(icon) = tiny_skia::Pixmap::decode_png(ICON) else {
        return Vec::new();
    };
    let (width, height) = (icon.width() as i32, icon.height() as i32);
    let mut data = icon.take();
    for pixel in data.as_chunks_mut::<4>().0 {
        let alpha = u16::from(pixel[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
        pixel.rotate_right(1);
    }
    vec![ksni::Icon {
        width,
        height,
        data,
    }]
}

pub(crate) fn run(session: Session, open_settings: bool) -> Result<(), Fault> {
    use winit::platform::{wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11};
    let mut event_loop = winit::event_loop::EventLoop::with_user_event();
    match session {
        Session::Wayland => {
            event_loop.with_wayland();
        }
        Session::X11 => {
            event_loop.with_x11();
        }
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .with_winit_event_loop_builder(event_loop)
        .with_winit_window_attributes_hook(window_attributes)
        .select()
        .map_err(|error| Fault::new("MAKO-UI-0001", error.to_string()))?;
    let _placement = match session {
        Session::Wayland => match crate::place::Kwin::install() {
            Ok(script) => Some(script),
            Err(error) => {
                eprintln!("MAKO-UI-0003: KWin positioning unavailable: {error}");
                None
            }
        },
        Session::X11 => None,
    };
    let config = Settings::load();
    let catalog = Catalog::load(&config.code());
    let entries: Vec<_> = pack::installed().into_iter().map(Entry::new).collect();
    let first = entries
        .iter()
        .find(|entry| entry.pack.id == config.chibi)
        .map(|entry| entry.pack.clone())
        .unwrap_or_else(Pack::builtin);
    let (tx, rx) = mpsc::channel();
    let (commands, game_rx) = mpsc::channel();
    let tray = Tray {
        tx: tx.clone(),
        labels: [catalog.text(16).into(), catalog.text(ID_QUIT).into()],
        icon: tray_icon(),
    };
    let tray = match tray.assume_sni_available(true).spawn() {
        Ok(handle) => Some(handle),
        Err(error) => {
            eprintln!("MAKO-UI-0004: Could not register the tray icon: {error}");
            None
        }
    };
    let show = open_settings || tray.is_none();
    let counts = HashMap::from([(first.id.clone(), 1)]);
    let game_config = config.clone();
    let game_tx = tx.clone();
    let host = std::thread::spawn(move || {
        let result = match session {
            Session::Wayland => crate::host_wayland::run(game_config, first, game_rx),
            Session::X11 => crate::host_x11::run(game_config, first, game_rx),
        };
        let _ = game_tx.send(Action::HostFinished(result));
    });
    let state = Rc::new(RefCell::new(Desktop {
        action_tx: tx,
        commands,
        session,
        config,
        catalog,
        entries,
        counts,
        tray,
        popup: None,
        window: None,
        focus_lost: Rc::new(Cell::new(false)),
        shown: Instant::now(),
        tray_point: (0, 0),
        pinned: false,
        taps: LogoTaps::default(),
        updating: Status::Idle,
        importing: false,
        import_result: None,
        save_at: None,
        fault: None,
    }));
    let timer = slint::Timer::default();
    let weak = Rc::downgrade(&state);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            if let Some(state) = weak.upgrade() {
                let mut state = state.borrow_mut();
                while let Ok(action) = rx.try_recv() {
                    state.handle(action);
                }
                state.tick();
            }
        },
    );
    let opened = if show {
        state.borrow_mut().show_settings(false)
    } else {
        Ok(())
    };
    let result = opened.and_then(|_| slint::run_event_loop_until_quit());
    timer.stop();
    let fault = {
        let mut state = state.borrow_mut();
        if state.save_at.is_some() {
            state.save();
        }
        if let Some(tray) = state.tray.take() {
            tray.shutdown();
        }
        state.fault.take()
    };
    drop(state);
    let _ = host.join();
    if let Some(fault) = fault {
        return Err(fault);
    }
    result.map_err(|error| Fault::new("MAKO-UI-0001", error.to_string()))
}
