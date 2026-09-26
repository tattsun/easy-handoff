//! Launch at login: HKCU\...\Run for the regular build, StartupTask for the MSIX (Store) build.
//!
//! A packaged app's HKCU writes are virtualized, so the Run key would have no effect there.

use windows::ApplicationModel::{StartupTask, StartupTaskState};
use windows::Win32::Foundation::{APPMODEL_ERROR_NO_PACKAGE, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ, RegCloseKey,
    RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::{HSTRING, Result, w};

const RUN_KEY: windows::core::PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const VALUE_NAME: windows::core::PCWSTR = w!("easy-handoff");
/// Must match TaskId in packaging/msix/AppxManifest.xml.
const STARTUP_TASK_ID: &str = "EasyHandoffStartup";

pub fn is_enabled() -> bool {
    if is_packaged() {
        return startup_task()
            .and_then(|t| t.State())
            .is_ok_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy);
    }
    let Ok(key) = open(KEY_QUERY_VALUE) else { return false };
    let err = unsafe { RegQueryValueExW(key, VALUE_NAME, None, None, None, None) };
    let _ = unsafe { RegCloseKey(key) };
    err == ERROR_SUCCESS
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if is_packaged() {
        let task = startup_task()?;
        // If the user disabled it in Task Manager / Settings, this is a no-op; is_enabled() reflects that.
        return if enabled { task.RequestEnableAsync()?.join().map(|_| ()) } else { task.Disable() };
    }
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

fn is_packaged() -> bool {
    let mut len = 0u32;
    let err = unsafe { GetCurrentPackageFullName(&mut len, None) };
    err != APPMODEL_ERROR_NO_PACKAGE
}

fn startup_task() -> Result<StartupTask> {
    StartupTask::GetAsync(&HSTRING::from(STARTUP_TASK_ID))?.join()
}

fn open(access: REG_SAM_FLAGS) -> Result<HKEY> {
    let mut key = HKEY::default();
    unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key) }.ok()?;
    Ok(key)
}
