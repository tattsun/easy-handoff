#![windows_subsystem = "windows"]

mod bt_audio;
mod startup;
mod tray;

use std::process::ExitCode;

use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

const DEFAULT_TARGET: &str = "AirPods";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Err(e) = bt_audio::init_com() {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }

    // No subcommand: run as a tray app.
    let Some(cmd) = args.first().map(String::as_str) else {
        return match tray::run(DEFAULT_TARGET) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    };

    // GUI subsystem has no console; borrow the parent's so CLI output is visible.
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let target = args.get(1).map(String::as_str).unwrap_or(DEFAULT_TARGET);
    let result = match cmd {
        "tray" => tray::run(target).map_err(|e| e.to_string()),
        "list" => bt_audio::list_devices()
            .map(|devices| {
                for d in devices {
                    let state = if d.connected { "connected" } else { "disconnected" };
                    println!("[{state}] {}", d.name);
                    for e in &d.endpoints {
                        println!("    {e}");
                    }
                }
            })
            .map_err(|e| e.to_string()),
        "connect" => bt_audio::connect(target).map_err(|e| e.to_string()),
        "disconnect" => bt_audio::disconnect(target).map_err(|e| e.to_string()),
        "toggle" => bt_audio::toggle(target)
            .map(|connected| println!("{}", if connected { "connected" } else { "disconnected" }))
            .map_err(|e| e.to_string()),
        _ => {
            eprintln!("usage: easy-handoff [tray | list | connect | disconnect | toggle] [device-name]");
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
