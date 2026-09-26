//! Tray icon: left click toggles the connection, right click opens the menu.

use std::time::{Duration, Instant};

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LPARAM, WPARAM};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, MSG, PostQuitMessage, PostThreadMessageW, SetTimer,
    TranslateMessage, WM_APP, WM_TIMER,
};
use windows::core::w;

use crate::{bt_audio, startup};

const WM_TOGGLE: u32 = WM_APP + 1;
const WM_QUIT_REQUEST: u32 = WM_APP + 2;
const WM_STARTUP_TOGGLE: u32 = WM_APP + 3;

const REFRESH_INTERVAL_MS: u32 = 2000;
const SPIN_INTERVAL_MS: u32 = 80;
/// While switching, poll the real state every N spinner frames.
const SPIN_POLL_EVERY: u32 = 5;
const SWITCH_TIMEOUT: Duration = Duration::from_secs(15);
const SPIN_FRAMES: usize = 12;

const ICON_SIZE: u32 = 32;
const BLUE: [u8; 3] = [10, 132, 255];
const GRAY: [u8; 3] = [150, 150, 150];

enum State {
    Connected,
    Disconnected,
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

pub fn run(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Single instance: the mutex lives until the process exits.
    let _mutex = unsafe { CreateMutexW(None, true, w!(r"Local\easy-handoff"))? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(());
    }

    let thread_id = unsafe { GetCurrentThreadId() };

    let toggle_item = MenuItem::new("接続 / 切断", true, None);
    let startup_item = CheckMenuItem::new("スタートアップに登録", true, startup::is_enabled(), None);
    let quit_item = MenuItem::new("終了", true, None);
    let menu = Menu::new();
    menu.append_items(&[&toggle_item, &startup_item, &PredefinedMenuItem::separator(), &quit_item])?;

    let icon_on = static_icon(true);
    let icon_off = static_icon(false);
    let spin_connecting: Vec<Icon> = (0..SPIN_FRAMES).map(|f| spinner_icon(f, BLUE)).collect();
    let spin_disconnecting: Vec<Icon> = (0..SPIN_FRAMES).map(|f| spinner_icon(f, GRAY)).collect();

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
    let (toggle_id, startup_id, quit_id) =
        (toggle_item.id().clone(), startup_item.id().clone(), quit_item.id().clone());
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if e.id == toggle_id {
            post(WM_TOGGLE);
        } else if e.id == startup_id {
            post(WM_STARTUP_TOGGLE);
        } else if e.id == quit_id {
            post(WM_QUIT_REQUEST);
        }
    }));

    let query = || match bt_audio::list_devices() {
        Ok(devices) => match devices.into_iter().find(|d| d.name.to_lowercase().contains(&target.to_lowercase())) {
            Some(d) if d.connected => State::Connected,
            Some(_) => State::Disconnected,
            None => State::Missing,
        },
        Err(e) => State::Error(e.to_string()),
    };
    let show = |state: &State, note: Option<&str>| {
        let (icon, text) = match state {
            State::Connected => (&icon_on, format!("{target}: 接続中")),
            State::Disconnected => (&icon_off, format!("{target}: 切断中")),
            State::Missing => (&icon_off, format!("{target} が見つかりません")),
            State::Error(e) => (&icon_off, format!("エラー: {e}")),
        };
        let _ = tray.set_icon(Some(icon.clone()));
        let tip = match note {
            Some(n) => format!("{text}\n{n}"),
            None => text,
        };
        let _ = tray.set_tooltip(Some(tip));
    };

    show(&query(), None);
    let refresh_timer = unsafe { SetTimer(None, 0, REFRESH_INTERVAL_MS, None) };
    let mut switching: Option<Switching> = None;

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        match msg.message {
            // Ignore clicks while a switch is in flight.
            WM_TOGGLE if switching.is_none() => match bt_audio::toggle(target) {
                Ok(want_connected) => {
                    let timer = unsafe { SetTimer(None, 0, SPIN_INTERVAL_MS, None) };
                    switching = Some(Switching { want_connected, started: Instant::now(), frame: 0, timer });
                    let _ = tray.set_tooltip(Some(if want_connected { "接続中…" } else { "切断中…" }));
                }
                Err(e) => show(&State::Error(e.to_string()), None),
            },
            WM_TOGGLE => {}
            WM_STARTUP_TOGGLE => {
                // The check item flips itself on click; apply it and resync with the registry.
                let _ = startup::set_enabled(startup_item.is_checked());
                startup_item.set_checked(startup::is_enabled());
            }
            WM_QUIT_REQUEST => unsafe { PostQuitMessage(0) },
            WM_TIMER if msg.hwnd.is_invalid() => {
                let id = msg.wParam.0;
                match switching.as_mut() {
                    Some(s) if id == s.timer => {
                        s.frame += 1;
                        let frames = if s.want_connected { &spin_connecting } else { &spin_disconnecting };
                        let _ = tray.set_icon(Some(frames[s.frame as usize % SPIN_FRAMES].clone()));

                        if s.frame % SPIN_POLL_EVERY == 0 {
                            let state = query();
                            let done = matches!(
                                (&state, s.want_connected),
                                (State::Connected, true) | (State::Disconnected, false)
                            );
                            let timed_out = s.started.elapsed() > SWITCH_TIMEOUT;
                            if done || timed_out {
                                unsafe {
                                    let _ = KillTimer(None, s.timer);
                                }
                                switching = None;
                                show(&state, (!done).then_some("切り替えに失敗しました"));
                            }
                        }
                    }
                    None if id == refresh_timer => show(&query(), None),
                    _ => {}
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
