//! UI strings: Japanese when the Windows display language is Japanese, English otherwise.
//! `EASY_HANDOFF_LANG=ja|en` overrides the detection (handy for screenshots).

use std::sync::OnceLock;

use windows::Win32::Globalization::GetUserDefaultUILanguage;

const LANG_JAPANESE: u16 = 0x11;

pub struct Texts {
    pub toggle: &'static str,
    pub devices: &'static str,
    pub no_devices: &'static str,
    pub startup: &'static str,
    pub quit: &'static str,
    pub connecting: &'static str,
    pub disconnecting: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub not_found: &'static str,
    pub error: &'static str,
    pub switch_failed: &'static str,
}

const JA: Texts = Texts {
    toggle: "接続 / 切断",
    devices: "デバイス",
    no_devices: "Bluetooth オーディオ機器がありません",
    startup: "スタートアップに登録",
    quit: "終了",
    connecting: "接続中…",
    disconnecting: "切断中…",
    connected: "接続中",
    disconnected: "切断中",
    not_found: "見つかりません",
    error: "エラー",
    switch_failed: "切り替えに失敗しました",
};

const EN: Texts = Texts {
    toggle: "Connect / Disconnect",
    devices: "Devices",
    no_devices: "No Bluetooth audio devices",
    startup: "Launch at sign-in",
    quit: "Quit",
    connecting: "Connecting…",
    disconnecting: "Disconnecting…",
    connected: "Connected",
    disconnected: "Disconnected",
    not_found: "Not found",
    error: "Error",
    switch_failed: "Failed to switch",
};

pub fn texts() -> &'static Texts {
    static TEXTS: OnceLock<&'static Texts> = OnceLock::new();
    TEXTS.get_or_init(|| {
        let japanese = match std::env::var("EASY_HANDOFF_LANG").as_deref() {
            Ok("ja") => true,
            Ok("en") => false,
            _ => {
                let lang_id = unsafe { GetUserDefaultUILanguage() };
                lang_id & 0x3ff == LANG_JAPANESE
            }
        };
        if japanese { &JA } else { &EN }
    })
}
