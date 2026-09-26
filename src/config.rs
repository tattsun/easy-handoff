//! Persistent settings under HKCU\Software\easy-handoff.

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW};
use windows::core::{PCWSTR, Result, w};

const KEY: PCWSTR = w!(r"Software\easy-handoff");
const DEVICE: PCWSTR = w!("Device");

/// Name of the device selected in the tray menu, if any.
pub fn device() -> Option<String> {
    let mut size = 0u32;
    unsafe { RegGetValueW(HKEY_CURRENT_USER, KEY, DEVICE, RRF_RT_REG_SZ, None, None, Some(&mut size)) }
        .ok()
        .ok()?;
    let mut buf = vec![0u16; size as usize / 2];
    unsafe {
        RegGetValueW(HKEY_CURRENT_USER, KEY, DEVICE, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    }
    .ok()
    .ok()?;
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn set_device(name: &str) -> Result<()> {
    let data: Vec<u16> = name.encode_utf16().chain([0]).collect();
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, KEY, DEVICE, REG_SZ.0, Some(data.as_ptr().cast()), (data.len() * 2) as u32)
    }
    .ok()
}
