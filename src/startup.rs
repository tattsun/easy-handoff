//! Launch-at-login via HKCU\...\Run.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ, RegCloseKey,
    RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{Result, w};

const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("easy-handoff");

pub fn is_enabled() -> bool {
    let Ok(key) = open(KEY_QUERY_VALUE) else { return false };
    let err = unsafe { RegQueryValueExW(key, VALUE_NAME, None, None, None, None) };
    let _ = unsafe { RegCloseKey(key) };
    err == ERROR_SUCCESS
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let key = open(KEY_SET_VALUE)?;
    let err = if enabled {
        let exe = std::env::current_exe().map_err(|e| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string()))?;
        let command = format!("\"{}\"", exe.display());
        // REG_SZ data: UTF-16 with terminating NUL, as bytes.
        let data: Vec<u8> = command.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        unsafe { RegSetValueExW(key, VALUE_NAME, None, REG_SZ, Some(&data)) }
    } else {
        match unsafe { RegDeleteValueW(key, VALUE_NAME) } {
            ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
            e => e,
        }
    };
    let _ = unsafe { RegCloseKey(key) };
    err.ok()
}

fn open(access: REG_SAM_FLAGS) -> Result<HKEY> {
    let mut key = HKEY::default();
    unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key) }.ok()?;
    Ok(key)
}

