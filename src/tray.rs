//! Tray icon: left click toggles the connection, right click opens the menu.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LPARAM, WPARAM};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, MSG, PostQuitMessage, PostThreadMessageW, SetTimer,
    TranslateMessage, WM_APP, WM_TIMER,
};
use windows::core::w;

use crate::{bt_audio, config, startup};

const WM_TOGGLE: u32 = WM_APP + 1;
const WM_MENU: u32 = WM_APP + 2;

const REFRESH_INTERVAL_MS: u32 = 2000;
const SPIN_INTERVAL_MS: u32 = 80;
/// While switching, poll the real state every N spinner frames.
const SPIN_POLL_EVERY: u32 = 5;
const SWITCH_TIMEOUT: Duration = Duration::from_secs(15);
const SPIN_FRAMES: usize = 12;

/// Picked when nothing has been selected yet.
const PREFERRED_DEFAULT: &str = "AirPods";
const DEVICE_ID_PREFIX: &str = "device:";

const ICON_SIZE: u32 = 32;
const BLUE: [u8; 3] = [10, 132, 255];
const GRAY: [u8; 3] = [150, 150, 150];

enum State {
    Connected,
    Disconnected,
    NoDevice,
    Missing,
    Error(String),
}

/// A connect/disconnect request that has been sent but not yet reflected in the device state.
struct Switching {
    want_connected: bool,
    started: Instant,
    frame: u32,
    timer: usize,
}

struct App {
    tray: TrayIcon,
    device_menu: Submenu,
    /// Current entries of `device_menu`, in display order.
    device_items: Vec<(String, CheckMenuItem)>,
    no_device_item: MenuItem,
    target: Option<String>,
    switching: Option<Switching>,
    icon_on: Icon,
    icon_off: Icon,
    spin_connecting: Vec<Icon>,
    spin_disconnecting: Vec<Icon>,
}

pub fn run(initial_target: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Single instance: the mutex lives until the process exits.
    let _mutex = unsafe { CreateMutexW(None, true, w!(r"Local\easy-handoff"))? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(());
    }

    let thread_id = unsafe { GetCurrentThreadId() };

    let toggle_item = MenuItem::new("接続 / 切断", true, None);
    let device_menu = Submenu::new("デバイス", true);
    let no_device_item = MenuItem::new("(Bluetooth オーディオ機器がありません)", false, None);
    device_menu.append(&no_device_item)?;
    let startup_item = CheckMenuItem::new("スタートアップに登録", true, startup::is_enabled(), None);
    let quit_item = MenuItem::new("終了", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &toggle_item,
        &device_menu,
        &startup_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ])?;

    let icon_off = static_icon(false);
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_icon(icon_off.clone())
        .with_tooltip("easy-handoff")
        .build()?;

    // Handlers run on arbitrary threads; forward to the UI thread's message queue.
    let post = move |msg: u32| unsafe {
        let _ = PostThreadMessageW(thread_id, msg, WPARAM(0), LPARAM(0));
    };
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            post(WM_TOGGLE);
        }
    }));
    let (menu_tx, menu_rx) = mpsc::channel::<MenuId>();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if menu_tx.send(e.id).is_ok() {
            post(WM_MENU);
        }
    }));

    let mut app = App {
        tray,
        device_menu,
        device_items: Vec::new(),
        no_device_item,
        target: initial_target.map(str::to_string).or_else(config::device),
        switching: None,
        icon_on: static_icon(true),
        icon_off,
        spin_connecting: (0..SPIN_FRAMES).map(|f| spinner_icon(f, BLUE)).collect(),
        spin_disconnecting: (0..SPIN_FRAMES).map(|f| spinner_icon(f, GRAY)).collect(),
    };

    let state = app.refresh();
    app.show(&state, None);
    let refresh_timer = unsafe { SetTimer(None, 0, REFRESH_INTERVAL_MS, None) };

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        match msg.message {
            WM_TOGGLE => app.toggle(),
            WM_MENU => {
                while let Ok(id) = menu_rx.try_recv() {
                    if id == *toggle_item.id() {
                        app.toggle();
                    } else if id == *startup_item.id() {
                        // The check item flips itself on click; apply it and resync with the registry.
                        let _ = startup::set_enabled(startup_item.is_checked());
                        startup_item.set_checked(startup::is_enabled());
                    } else if id == *quit_item.id() {
                        unsafe { PostQuitMessage(0) };
                    } else if let Some(name) = id.as_ref().strip_prefix(DEVICE_ID_PREFIX) {
                        app.select(name.to_string());
                    }
                }
            }
            WM_TIMER if msg.hwnd.is_invalid() => {
                let id = msg.wParam.0;
                if app.switching.as_ref().is_some_and(|s| s.timer == id) {
                    app.spin();
                } else if app.switching.is_none() && id == refresh_timer {
                    let state = app.refresh();
                    app.show(&state, None);
                }
            }
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
    Ok(())
}

impl App {
    /// Re-enumerate devices, sync the device submenu, and return the target's state.
    fn refresh(&mut self) -> State {
        let devices = match bt_audio::list_devices() {
            Ok(d) => d,
            Err(e) => return State::Error(e.to_string()),
        };

        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        if self.target.is_none() {
            self.target = names
                .iter()
                .find(|n| n.to_lowercase().contains(&PREFERRED_DEFAULT.to_lowercase()))
                .or(names.first())
                .map(|n| n.to_string());
        }
        if !self.device_items.iter().map(|(n, _)| n.as_str()).eq(names.iter().copied()) {
            self.rebuild_device_menu(&names);
        }

        let Some(target) = &self.target else { return State::NoDevice };
        match bt_audio::find_in(devices, target) {
            Some(d) if d.connected => State::Connected,
            Some(_) => State::Disconnected,
            None => State::Missing,
        }
    }

    fn rebuild_device_menu(&mut self, names: &[&str]) {
        let _ = self.device_menu.remove(&self.no_device_item);
        for (_, item) in self.device_items.drain(..) {
            let _ = self.device_menu.remove(&item);
        }
        for name in names {
            let checked = self.target.as_deref() == Some(*name);
            let item = CheckMenuItem::with_id(format!("{DEVICE_ID_PREFIX}{name}"), name, true, checked, None);
            let _ = self.device_menu.append(&item);
            self.device_items.push((name.to_string(), item));
        }
        if names.is_empty() {
            let _ = self.device_menu.append(&self.no_device_item);
        }
    }

    fn select(&mut self, name: String) {
        let _ = config::set_device(&name);
        for (n, item) in &self.device_items {
            item.set_checked(*n == name);
        }
        self.target = Some(name);
        if self.switching.is_none() {
            let state = self.refresh();
            self.show(&state, None);
        }
    }

    fn toggle(&mut self) {
        // Ignore clicks while a switch is in flight.
        if self.switching.is_some() {
            return;
        }
        let Some(target) = self.target.clone() else { return };
        match bt_audio::toggle(&target) {
            Ok(want_connected) => {
                let timer = unsafe { SetTimer(None, 0, SPIN_INTERVAL_MS, None) };
                self.switching = Some(Switching { want_connected, started: Instant::now(), frame: 0, timer });
                let verb = if want_connected { "接続中…" } else { "切断中…" };
                let _ = self.tray.set_tooltip(Some(format!("{target}: {verb}")));
            }
            Err(e) => self.show(&State::Error(e.to_string()), None),
        }
    }

    /// One spinner frame; finishes the switch once the state matches or it times out.
    fn spin(&mut self) {
        let Some(s) = self.switching.as_mut() else { return };
        s.frame += 1;
        let frames = if s.want_connected { &self.spin_connecting } else { &self.spin_disconnecting };
        let _ = self.tray.set_icon(Some(frames[s.frame as usize % SPIN_FRAMES].clone()));
        if s.frame % SPIN_POLL_EVERY != 0 {
            return;
        }

        let (want_connected, started, timer) = (s.want_connected, s.started, s.timer);
        let state = self.refresh();
        let done = matches!((&state, want_connected), (State::Connected, true) | (State::Disconnected, false));
        if done || started.elapsed() > SWITCH_TIMEOUT {
            unsafe {
                let _ = KillTimer(None, timer);
            }
            self.switching = None;
            self.show(&state, (!done).then_some("切り替えに失敗しました"));
        }
    }

    fn show(&self, state: &State, note: Option<&str>) {
        let target = self.target.as_deref().unwrap_or_default();
        let (icon, text) = match state {
            State::Connected => (&self.icon_on, format!("{target}: 接続中")),
            State::Disconnected => (&self.icon_off, format!("{target}: 切断中")),
            State::NoDevice => (&self.icon_off, "Bluetooth オーディオ機器がありません".to_string()),
            State::Missing => (&self.icon_off, format!("{target} が見つかりません")),
            State::Error(e) => (&self.icon_off, format!("エラー: {e}")),
        };
        let _ = self.tray.set_icon(Some(icon.clone()));
        let tip = match note {
            Some(n) => format!("{text}\n{n}"),
            None => text,
        };
        let _ = self.tray.set_tooltip(Some(tip));
    }
}

/// Filled blue badge when connected, gray ring when disconnected.
fn static_icon(connected: bool) -> Icon {
    draw(|d, _| {
        if connected {
            (BLUE, cover(14.0, d))
        } else {
            (GRAY, cover(14.0, d) - cover(9.0, d))
        }
    })
}

/// Faint ring with a rotating arc.
fn spinner_icon(frame: usize, color: [u8; 3]) -> Icon {
    let start = frame as f32 / SPIN_FRAMES as f32 * std::f32::consts::TAU;
    let arc_len = std::f32::consts::TAU * 0.3;
    draw(|d, angle| {
        let ring = cover(14.0, d) - cover(9.0, d);
        let rel = (angle - start).rem_euclid(std::f32::consts::TAU);
        if rel < arc_len { (color, ring) } else { (color, ring * 0.25) }
    })
}

/// Coverage of a disc of radius `r` at distance `d`, with a 1px anti-aliased edge.
fn cover(r: f32, d: f32) -> f32 {
    (r - d + 0.5).clamp(0.0, 1.0)
}

/// Rasterize from a per-pixel shader taking (distance from center, angle clockwise from top).
fn draw(shade: impl Fn(f32, f32) -> ([u8; 3], f32)) -> Icon {
    let c = ICON_SIZE as f32 / 2.0 - 0.5;
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            let d = (dx * dx + dy * dy).sqrt();
            let angle = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU);
            let (rgb, alpha) = shade(d, angle);
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], (alpha.clamp(0.0, 1.0) * 255.0) as u8]);
        }
    }
    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).expect("valid icon")
}
