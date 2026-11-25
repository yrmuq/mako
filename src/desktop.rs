// Copyright (C) 2026 yrmuq
// SPDX-License-Identifier: GPL-3.0-only

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use ksni::blocking::TrayMethods;
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
};

use crate::about::LogoTaps;
use crate::config::{InputMode, LanguagePref, PrecipitationMode, Settings, ThemePref};
use crate::game::{Command, Startup};
use crate::i18n::{Catalog, ID_NAME, ID_QUIT};
use crate::icons;
use crate::pack::{self, Pack};
use crate::season::Date;
use crate::updates::{self, Status};
use crate::visitors::{Kind as VisitorKind, LostChibi};
use crate::weather::{self, Weather};
use crate::{Fault, Session};

slint::include_modules!();

thread_local! {
    static CREATING_POPUP: Cell<bool> = const { Cell::new(false) };
}

const POPUP: &str = "mako-popup";
const POPUP_DURATION: Duration = Duration::from_millis(180);
const ICON: &[u8] = include_bytes!("../assets/icon.png");
static TERMINATING: AtomicBool = AtomicBool::new(false);

extern "C" fn request_shutdown(_signal: libc::c_int) {
    TERMINATING.store(true, Ordering::Relaxed);
}

fn watch_shutdown() -> Result<(), Fault> {
    // The handler leaves scene I/O to the regular event-loop shutdown.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = request_shutdown as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut action.sa_mask);
        if libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut()) != 0 {
            return Err(Fault::new(
                "MAKO-UI-0005",
                format!(
                    "Could not register the shutdown handler: {}",
                    std::io::Error::last_os_error()
                ),
            ));
        }
    }
    Ok(())
}

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
    Import,
    Imported(Option<Result<Pack, Fault>>),
    Uninstall(String),
    Language(i32),
    Theme(i32),
    InputMode(i32),
    Precipitation(i32),
    SearchCity(String),
    ChooseCity(i32),
    Snow(i32, i32),
    Scale(f32),
    Proportions(i32, i32),
    Preview(bool),
    CallVisitor(VisitorKind),
    Date(i32, i32, i32),
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

struct PopupTransition {
    started: Instant,
    from: f32,
    closing: bool,
}

impl PopupTransition {
    fn reveal(&self, t: f32) -> f32 {
        if self.closing {
            self.from * (1.0 - t.powi(3))
        } else {
            1.0 - (1.0 - self.from) * (1.0 - t).powi(3)
        }
    }
}

struct TrayIcons {
    theme: icons::Theme,
    configure: Option<icons::Icon>,
    pin: Option<icons::Icon>,
    add: Option<icons::Icon>,
    remove: Option<icons::Icon>,
}

impl TrayIcons {
    fn load(theme: icons::Theme) -> Self {
        Self {
            configure: theme.icon("configure"),
            pin: theme.icon("window-pin"),
            add: theme.icon("list-add"),
            remove: theme.icon("list-remove"),
            theme,
        }
    }

    fn apply(&self, popup: &Popup) {
        if let Some(icon) = &self.configure {
            popup.set_configure_icon(icon.image.clone());
            popup.set_tint_configure(icon.tint);
        }
        if let Some(icon) = &self.pin {
            popup.set_pin_icon(icon.image.clone());
            popup.set_tint_pin(icon.tint);
        }
        if let Some(icon) = &self.add {
            popup.set_add_icon(icon.image.clone());
            popup.set_tint_add(icon.tint);
        }
        if let Some(icon) = &self.remove {
            popup.set_remove_icon(icon.image.clone());
            popup.set_tint_remove(icon.tint);
        }
    }
}

struct Desktop {
    action_tx: Sender<Action>,
    commands: Sender<Command>,
    session: Session,
    config: Settings,
    weather: Weather,
    dark: bool,
    catalog: Catalog,
    entries: Vec<Entry>,
    counts: HashMap<String, i32>,
    ordinary_counts: Option<HashMap<String, i32>>,
    preview_generation: u64,
    tray: Option<ksni::blocking::Handle<Tray>>,
    tray_icons: TrayIcons,
    next_icon_check: Instant,
    popup: Option<Popup>,
    popup_transition: Option<PopupTransition>,
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
    fn lose_chibi(&mut self, lost: LostChibi) {
        if lost.preview == 0 {
            if let Some(counts) = &mut self.ordinary_counts {
                if let Some(count) = counts.get_mut(&lost.pack) {
                    *count = (*count - 1).max(0);
                }
                return;
            }
        } else if self.ordinary_counts.is_none() || self.preview_generation != lost.preview {
            return;
        }
        if let Some(count) = self.counts.get_mut(&lost.pack) {
            *count = (*count - 1).max(0);
            self.refresh_rows();
        }
    }

    fn handle(&mut self, action: Action) {
        match action {
            Action::Toggle(x, y) => {
                if let Some(popup) = &self.popup {
                    if self
                        .popup_transition
                        .as_ref()
                        .is_some_and(|transition| transition.closing)
                    {
                        self.focus_lost.set(false);
                        self.shown = Instant::now();
                        popup
                            .window()
                            .with_winit_window(|window| window.focus_window());
                        self.animate_popup(false);
                    } else {
                        self.hide_popup();
                    }
                } else {
                    self.tray_point = (x, y);
                    if let Err(error) = self.show_popup() {
                        eprintln!("MAKO-UI-0002: Could not open the tray panel: {error}");
                    }
                }
            }
            Action::Dismiss => self.hide_popup(),
            Action::Configure(chibis) => {
                if !self.pinned {
                    self.hide_popup();
                }
                if let Err(error) = self.show_settings(chibis) {
                    eprintln!("MAKO-UI-0002: Could not open settings: {error}");
                }
            }
            Action::Pin => {
                self.pinned = !self.pinned;
                if let Some(popup) = &self.popup {
                    popup.set_pinned(self.pinned);
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
            Action::Uninstall(id) => {
                match pack::uninstall(&id) {
                    Ok(()) => {
                        let _ = self.commands.send(Command::Uninstall(id.clone()));
                        self.entries.retain(|entry| entry.pack.id != id);
                        self.counts.remove(&id);
                        if let Some(counts) = &mut self.ordinary_counts {
                            counts.remove(&id);
                        }
                        self.config.favorites.retain(|favorite| *favorite != id);
                        if self.config.chibi == id {
                            self.config.chibi = pack::BUILTIN_ID.to_owned();
                        }
                        self.save();
                        self.import_result = Some(62);
                        self.refresh_rows();
                    }
                    Err(error) => {
                        eprintln!("{error}");
                        self.import_result = Some(63);
                    }
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
                self.refresh_weather();
            }
            Action::Theme(index) => {
                self.config.theme = match index {
                    1 => ThemePref::Light,
                    2 => ThemePref::Dark,
                    _ => ThemePref::System,
                };
                self.schedule_save();
                self.sync_theme();
            }
            Action::InputMode(index) => {
                self.config.input_mode = match index {
                    1 => InputMode::Background,
                    2 => InputMode::Auto,
                    _ => InputMode::Main,
                };
                self.apply_settings();
            }
            Action::Precipitation(index) => {
                self.config.precipitation = match index {
                    0 => PrecipitationMode::Off,
                    2 => PrecipitationMode::Auto,
                    _ => PrecipitationMode::On,
                };
                self.weather
                    .set_enabled(self.config.precipitation == PrecipitationMode::Auto);
                self.apply_settings();
                let _ = self.commands.send(Command::Weather(self.weather.kind()));
                self.refresh_weather();
            }
            Action::SearchCity(query) => {
                self.weather.edit(query);
                self.refresh_weather();
            }
            Action::ChooseCity(index) => {
                if let Some(place) = usize::try_from(index)
                    .ok()
                    .and_then(|index| self.weather.select(index))
                {
                    self.config.weather_location = Some(place);
                    self.schedule_save();
                    let _ = self.commands.send(Command::Weather(self.weather.kind()));
                    self.refresh_weather();
                }
            }
            Action::Snow(size, speed) => {
                self.config.snow_size = size.clamp(25, 200) as u32;
                self.config.snow_speed = speed.clamp(25, 200) as u32;
                self.apply_settings();
            }
            Action::Scale(pixels) => {
                self.config.set_size_px(pixels);
                self.apply_settings();
            }
            Action::Proportions(width, height) => {
                self.config.width = width.clamp(25, 200) as u32;
                self.config.height = height.clamp(25, 200) as u32;
                self.refresh_size_controls();
                self.apply_settings();
            }
            Action::CallVisitor(guest) => {
                let _ = self.commands.send(Command::Visit(guest));
            }
            Action::Preview(enabled) => {
                if enabled != self.config.preview_date.is_some() {
                    if enabled {
                        self.preview_generation += 1;
                        self.ordinary_counts = Some(self.counts.clone());
                        self.config.preview_date = Some(Date::today());
                    } else {
                        self.config.preview_date = None;
                        self.counts = self.ordinary_counts.take().expect("ordinary scene counts");
                    }
                    let _ = self.commands.send(Command::Appearance(self.config.clone()));
                    self.refresh_rows();
                    self.refresh_date_controls();
                }
            }
            Action::Date(day, month, year) => {
                if self.config.preview_date.is_some() {
                    self.config.preview_date = Some(Date::new(day, month, year));
                    let _ = self.commands.send(Command::Appearance(self.config.clone()));
                }
                self.refresh_date_controls();
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
        let previous = self.weather.kind();
        if self.weather.tick(|| self.config.code()) {
            if previous != self.weather.kind() {
                let _ = self.commands.send(Command::Weather(self.weather.kind()));
            }
            self.refresh_weather();
        }
        if self.save_at.is_some_and(|when| Instant::now() >= when) {
            self.save();
        }
        if self.popup.is_some()
            && !self.pinned
            && self.focus_lost.get()
            && self.shown.elapsed() > POPUP_DURATION
        {
            self.hide_popup();
        }
        if let (Some(popup), Some(transition)) = (&self.popup, &self.popup_transition) {
            let t = (transition.started.elapsed().as_secs_f32() / POPUP_DURATION.as_secs_f32())
                .min(1.0);
            popup.set_reveal(transition.reveal(t));
            if t >= 1.0 {
                if transition.closing {
                    let _ = popup.hide();
                    self.popup = None;
                }
                self.popup_transition = None;
            }
        }
        if self.popup_transition.is_none() && Instant::now() >= self.next_icon_check {
            if self.config.preview_date.is_none()
                && self
                    .window
                    .as_ref()
                    .is_some_and(|window| window.get_dev_open())
            {
                self.refresh_date_controls();
            }
            self.next_icon_check = Instant::now() + Duration::from_secs(1);
            self.sync_theme();
            let theme = icons::Theme::load();
            if theme != self.tray_icons.theme {
                self.tray_icons = TrayIcons::load(theme);
            }
        }
    }

    fn sync_theme(&mut self) {
        let dark = self.config.dark();
        if self.dark != dark {
            self.dark = dark;
            if let Some(tray) = &self.tray {
                let icon = tray_icon(dark);
                tray.update(|tray| tray.icon = icon);
            }
            self.refresh_style();
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

    fn apply_settings(&mut self) {
        self.schedule_save();
        let _ = self.commands.send(Command::Appearance(self.config.clone()));
    }

    fn tray_labels(&self) -> [String; 2] {
        [
            self.catalog.text(16).into(),
            self.catalog.text(ID_QUIT).into(),
        ]
    }

    fn animate_popup(&mut self, closing: bool) {
        if let Some(popup) = &self.popup {
            self.popup_transition = Some(PopupTransition {
                started: Instant::now(),
                from: popup.get_reveal(),
                closing,
            });
        }
    }

    fn hide_popup(&mut self) {
        if !self
            .popup_transition
            .as_ref()
            .is_some_and(|transition| transition.closing)
        {
            self.animate_popup(true);
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
        popup.set_reveal(0.0);
        self.tray_icons.apply(&popup);
        popup.set_rows(model(self.favorite_rows()));
        style_popup(&popup, self.translations(), self.config.dark());
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
        let point = self.tray_point;
        let x11 = matches!(self.session, Session::X11);
        popup.window().on_winit_window_event(move |window, event| {
            if x11 && matches!(event, winit::event::WindowEvent::Resized(_)) {
                window.with_winit_window(|native| crate::place::x11(native, point));
            }
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
            slint::CloseRequestResponse::KeepWindowShown
        });
        self.shown = Instant::now();
        popup.show()?;
        if matches!(self.session, Session::X11) {
            let point = self.tray_point;
            popup
                .window()
                .with_winit_window(|window| crate::place::x11(window, point));
        }
        self.popup = Some(popup);
        self.animate_popup(false);
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
            connect!(on_input_mode_selected, Action::InputMode(index), index);
            connect!(
                on_precipitation_selected,
                Action::Precipitation(index),
                index
            );
            connect!(on_city_edited, Action::SearchCity(query.to_string()), query);
            connect!(on_city_selected, Action::ChooseCity(index), index);
            connect!(on_snow_selected, Action::Snow(size, speed), size, speed);
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
            connect!(on_import_zip, Action::Import);
            connect!(on_uninstall_chibi, Action::Uninstall(id.to_string()), id);
            connect!(on_preview_selected, Action::Preview(enabled), enabled);
            connect!(on_call_santa, Action::CallVisitor(VisitorKind::Santa));
            connect!(on_call_ufo, Action::CallVisitor(VisitorKind::Ufo));
            let date_tx = tx.clone();
            window.on_date_selected(move |day, month, year| {
                let _ = date_tx.send(Action::Date(day, month, year));
            });
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
        window.set_input_mode_index(match self.config.input_mode {
            InputMode::Main => 0,
            InputMode::Background => 1,
            InputMode::Auto => 2,
        });
        window.set_precipitation_mode(self.config.precipitation.index());
        window.set_snow_size(self.config.snow_size as f32);
        window.set_snow_speed(self.config.snow_speed as f32);
        window.set_width_value(self.config.width as i32);
        window.set_height_value(self.config.height as i32);
        self.refresh_size_controls();
        self.refresh_date_controls();
        self.refresh_style();
        self.refresh_rows();
        self.refresh_status();
        self.refresh_weather();
        window.show()?;
        window
            .window()
            .with_winit_window(|native| native.focus_window());
        Ok(())
    }

    fn refresh_size_controls(&self) {
        if let Some(window) = &self.window {
            let (min, max) = self.config.size_range_px();
            window.set_size_minimum(min);
            window.set_size_maximum(max);
            window.set_size_value(self.config.size_px());
        }
    }

    fn refresh_date_controls(&self) {
        if let Some(window) = &self.window {
            let date = self.config.preview_date.unwrap_or_else(Date::today);
            window.set_preview_date(self.config.preview_date.is_some());
            window.set_date_year(date.year);
            window.set_date_month(date.month);
            window.set_date_days(date.days());
            window.set_date_day(date.day);
        }
    }

    fn row(&self, entry: &Entry) -> ChibiRow {
        let id = &entry.pack.id;
        ChibiRow {
            id: id.into(),
            name: match id.as_str() {
                pack::BUILTIN_ID => self.catalog.text(ID_NAME).into(),
                pack::LION_ID => self.catalog.text(64).into(),
                pack::TURTLE_ID => self.catalog.text(68).into(),
                _ => entry.pack.name.as_str().into(),
            },
            face: entry.face.clone(),
            count: self.counts.get(id).copied().unwrap_or(0),
            added: self.config.favorites.contains(id),
            builtin: pack::is_builtin(id),
        }
    }

    fn favorite_rows(&self) -> Vec<ChibiRow> {
        self.config
            .favorites
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| entry.pack.id == *id))
            .map(|entry| self.row(entry))
            .collect()
    }

    fn refresh_rows(&self) {
        let favorites = self.favorite_rows();
        let favorite_count = favorites.len() as i32;
        let rows = model(favorites);
        if let Some(popup) = &self.popup {
            popup.set_rows(rows.clone());
        }
        if let Some(window) = &self.window {
            let mut installed = self.favorite_rows();
            installed.extend(
                self.entries
                    .iter()
                    .filter(|entry| !self.config.favorites.contains(&entry.pack.id))
                    .map(|entry| self.row(entry)),
            );
            window.set_favorite_count(favorite_count);
            window.set_installed(model(installed));
        }
    }

    fn refresh_weather(&self) {
        let Some(window) = &self.window else { return };
        window.set_city_query(self.weather.query().into());
        window.set_city_suggestions(model(
            self.weather
                .matches()
                .iter()
                .map(|place| place.label().into())
                .collect(),
        ));
        let status = self.weather.status();
        let text = match status {
            weather::Status::Idle => "",
            weather::Status::Searching => self.catalog.text(74),
            weather::Status::NoMatches => self.catalog.text(75),
            weather::Status::SearchFailed => self.catalog.text(76),
            weather::Status::Loading => self.catalog.text(77),
            weather::Status::Ready => self.catalog.text(78),
            weather::Status::WeatherFailed => self.catalog.text(79),
        };
        window.set_weather_status(text.into());
        window.set_weather_ready(status == weather::Status::Ready);
    }

    fn translations(&self) -> ModelRc<SharedString> {
        model((0..=79).map(|id| self.catalog.text(id).into()).collect())
    }

    fn refresh_style(&self) {
        let strings = self.translations();
        let dark = self.dark;
        if let Some(popup) = &self.popup {
            style_popup(popup, strings.clone(), dark);
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

fn style_popup(popup: &Popup, strings: ModelRc<SharedString>, dark: bool) {
    popup.global::<Translations>().set_strings(strings);
    style(&popup.global::<Breeze>(), dark);
    popup.set_dark(dark);
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

fn tray_icon(dark: bool) -> Vec<ksni::Icon> {
    let color = if dark {
        [234, 238, 245, 255]
    } else {
        [38, 41, 51, 255]
    };
    [22, 32]
        .map(|size| {
            let mut icon = tiny_skia::Pixmap::new(32, 32).unwrap();
            crate::props::oval(&mut icon, (5.0, 13.5, 22.0, 12.0), color);
            crate::props::oval(&mut icon, (2.0, 11.0, 28.0, 11.0), color);
            crate::props::oval(&mut icon, (9.0, 5.0, 14.0, 13.0), color);
            let mut path = tiny_skia::PathBuilder::new();
            path.move_to(6.0, 15.5);
            path.quad_to(16.0, 19.0, 26.0, 15.5);
            for x in [9.0, 16.0, 23.0] {
                path.move_to(x - 0.15, 20.8);
                path.line_to(x + 0.15, 20.8);
            }
            icon.stroke_path(
                &path.finish().unwrap(),
                &tiny_skia::Paint {
                    blend_mode: tiny_skia::BlendMode::Clear,
                    ..tiny_skia::Paint::default()
                },
                &tiny_skia::Stroke {
                    width: 1.8,
                    line_cap: tiny_skia::LineCap::Round,
                    ..tiny_skia::Stroke::default()
                },
                tiny_skia::Transform::identity(),
                None,
            );
            let mut scaled = tiny_skia::Pixmap::new(size, size).unwrap();
            scaled.draw_pixmap(
                0,
                0,
                icon.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::from_scale(size as f32 / 32.0, size as f32 / 32.0),
                None,
            );
            let mut data = scaled.take();
            for pixel in data.as_chunks_mut::<4>().0 {
                let alpha = pixel[3];
                *pixel = [alpha, color[0], color[1], color[2]];
            }
            ksni::Icon {
                width: size as i32,
                height: size as i32,
                data,
            }
        })
        .into()
}

pub(crate) fn run(session: Session, open_settings: bool) -> Result<(), Fault> {
    use winit::platform::{wayland::EventLoopBuilderExtWayland, x11::EventLoopBuilderExtX11};
    watch_shutdown()?;
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
    let (fullscreen, fullscreen_rx) = mpsc::channel();
    let _placement = match session {
        Session::Wayland => match crate::place::Kwin::install(fullscreen) {
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
    let tray_icons = TrayIcons::load(icons::Theme::load());
    let entries: Vec<_> = pack::installed().into_iter().map(Entry::new).collect();
    let first = entries
        .iter()
        .find(|entry| entry.pack.id == config.chibi)
        .map(|entry| entry.pack.clone())
        .unwrap_or_else(Pack::builtin);
    let world = crate::world::World::load(&first.id);
    let packs: Vec<_> = world
        .chibis()
        .iter()
        .filter_map(|id| {
            entries
                .iter()
                .find(|entry| entry.pack.id == *id)
                .map(|entry| entry.pack.clone())
        })
        .collect();
    let (tx, rx) = mpsc::channel();
    let (commands, game_rx) = mpsc::channel();
    let dark = config.dark();
    let tray = Tray {
        tx: tx.clone(),
        labels: [catalog.text(16).into(), catalog.text(ID_QUIT).into()],
        icon: tray_icon(dark),
    };
    let tray = match tray.assume_sni_available(true).spawn() {
        Ok(handle) => Some(handle),
        Err(error) => {
            eprintln!("MAKO-UI-0004: Could not register the tray icon: {error}");
            None
        }
    };
    let show = open_settings || tray.is_none();
    let mut counts = HashMap::new();
    for pack in &packs {
        *counts.entry(pack.id.clone()).or_default() += 1;
    }
    let (lost_tx, lost_rx) = mpsc::channel();
    let startup = Startup {
        packs,
        world,
        lost_chibi: Some(lost_tx),
    };
    let game_config = config.clone();
    let game_tx = tx.clone();
    let host = std::thread::spawn(move || {
        let result = match session {
            Session::Wayland => {
                crate::host_wayland::run(game_config, startup, game_rx, fullscreen_rx)
            }
            Session::X11 => crate::host_x11::run(game_config, startup, game_rx),
        };
        let _ = game_tx.send(Action::HostFinished(result));
    });
    let weather = Weather::new(
        config.weather_location.clone(),
        config.precipitation == PrecipitationMode::Auto,
    );
    let state = Rc::new(RefCell::new(Desktop {
        action_tx: tx,
        commands,
        session,
        config,
        weather,
        dark,
        catalog,
        entries,
        counts,
        ordinary_counts: None,
        preview_generation: 0,
        tray,
        tray_icons,
        next_icon_check: Instant::now() + Duration::from_secs(1),
        popup: None,
        popup_transition: None,
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
                while let Ok(lost) = lost_rx.try_recv() {
                    state.lose_chibi(lost);
                }
                while let Ok(action) = rx.try_recv() {
                    state.handle(action);
                }
                if TERMINATING.load(Ordering::Relaxed) {
                    state.handle(Action::Quit);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tray_icon_uses_the_same_monochrome_silhouette_in_both_themes() {
        let light = tray_icon(false);
        let dark = tray_icon(true);
        for (light, dark) in light.iter().zip(&dark) {
            assert_eq!(light.width, dark.width);
            assert!(light
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[0] == 255));
            for (a, b) in light
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .zip(dark.data.as_chunks::<4>().0)
            {
                assert_eq!(a[0], b[0]);
                if a[0] > 0 {
                    assert!(a[1] < b[1]);
                }
            }
        }
    }

    #[test]
    fn reversing_the_popup_transition_keeps_its_current_reveal() {
        let closing = PopupTransition {
            from: 1.0,
            closing: true,
            started: Instant::now(),
        };
        let interrupted = closing.reveal(0.8);
        let reopening = PopupTransition {
            from: interrupted,
            closing: false,
            started: Instant::now(),
        };
        assert!((reopening.reveal(0.0) - interrupted).abs() <= f32::EPSILON);
        assert!(reopening.reveal(0.5) > interrupted);
        assert_eq!(reopening.reveal(1.0), 1.0);
        assert_eq!(closing.reveal(1.0), 0.0);
    }
}
