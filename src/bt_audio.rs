//! Bluetooth audio connect/disconnect via the KS property set used by the Settings app.
//!
//! Audio endpoint -> IDeviceTopology -> connector -> connected Bluetooth audio KS filter
//! -> IKsControl, then KSPROPERTY_ONESHOT_RECONNECT / KSPROPERTY_ONESHOT_DISCONNECT.

use std::collections::BTreeMap;

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, DEVICE_STATE_UNPLUGGED, IConnector, IDeviceTopology,
    IMMDevice, IMMDeviceEnumerator, IPart, MMDeviceEnumerator, eAll,
};
use windows::Win32::Media::KernelStreaming::{
    IKsControl, KSIDENTIFIER, KSIDENTIFIER_0, KSIDENTIFIER_0_0, KSPROPERTY_BTAUDIO,
    KSPROPERTY_ONESHOT_DISCONNECT, KSPROPERTY_ONESHOT_RECONNECT, KSPROPERTY_TYPE_GET,
    KSPROPSETID_BtAudio,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, STGM_READ,
};
use windows::core::{Error, Interface, PWSTR, Result};

/// One Bluetooth audio device (grouped by display name), e.g. "AirPods Pro".
pub struct BtAudioDevice {
    pub name: String,
    pub connected: bool,
    /// Endpoint friendly names, e.g. "ヘッドホン (AirPods Pro)".
    pub endpoints: Vec<String>,
    /// KS filters (A2DP, HFP, ...) that accept the BtAudio property set.
    filters: Vec<IKsControl>,
}

pub fn init_com() -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
}

pub fn list_devices() -> Result<Vec<BtAudioDevice>> {
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let collection = unsafe {
        enumerator.EnumAudioEndpoints(eAll, DEVICE_STATE(DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_UNPLUGGED.0))?
    };

    let mut devices: BTreeMap<String, BtAudioDevice> = BTreeMap::new();
    // Several endpoints share one KS filter; activate each filter once.
    let mut seen_filters = std::collections::HashSet::new();

    for i in 0..unsafe { collection.GetCount()? } {
        let endpoint = unsafe { collection.Item(i)? };
        let Some(filter_id) = connected_filter_id(&endpoint) else { continue };
        if !is_bluetooth(&filter_id) {
            continue;
        }

        let endpoint_name = friendly_name(&endpoint).unwrap_or_default();
        let device_name = device_name_from_endpoint(&endpoint_name);
        let connected = unsafe { endpoint.GetState()? } == DEVICE_STATE_ACTIVE;

        let entry = devices.entry(device_name.clone()).or_insert_with(|| BtAudioDevice {
            name: device_name,
            connected: false,
            endpoints: Vec::new(),
            filters: Vec::new(),
        });
        entry.connected |= connected;
        entry.endpoints.push(endpoint_name);

        if seen_filters.insert(filter_id.clone()) {
            let filter = unsafe { enumerator.GetDevice(&windows::core::HSTRING::from(&filter_id))? };
            if let Ok(ks) = unsafe { filter.Activate::<IKsControl>(CLSCTX_ALL, None) } {
                entry.filters.push(ks);
            }
        }
    }

    Ok(devices.into_values().collect())
}

pub fn connect(target: &str) -> Result<()> {
    send(&find(target)?, KSPROPERTY_ONESHOT_RECONNECT)
}

pub fn disconnect(target: &str) -> Result<()> {
    send(&find(target)?, KSPROPERTY_ONESHOT_DISCONNECT)
}

/// Returns the new connection state.
pub fn toggle(target: &str) -> Result<bool> {
    let device = find(target)?;
    if device.connected {
        send(&device, KSPROPERTY_ONESHOT_DISCONNECT)?;
        Ok(false)
    } else {
        send(&device, KSPROPERTY_ONESHOT_RECONNECT)?;
        Ok(true)
    }
}

fn find(target: &str) -> Result<BtAudioDevice> {
    let needle = target.to_lowercase();
    list_devices()?
        .into_iter()
        .find(|d| d.name.to_lowercase().contains(&needle))
        .ok_or_else(|| Error::new(windows::Win32::Foundation::E_INVALIDARG, format!("device not found: {target}")))
}

fn send(device: &BtAudioDevice, prop: KSPROPERTY_BTAUDIO) -> Result<()> {
    let property = KSIDENTIFIER {
        Anonymous: KSIDENTIFIER_0 {
            Anonymous: KSIDENTIFIER_0_0 { Set: KSPROPSETID_BtAudio, Id: prop.0 as u32, Flags: KSPROPERTY_TYPE_GET },
        },
    };
    let mut last_err = None;
    let mut ok = false;
    for ks in &device.filters {
        let mut returned = 0u32;
        match unsafe {
            ks.KsProperty(&property, size_of::<KSIDENTIFIER>() as u32, std::ptr::null_mut(), 0, &mut returned)
        } {
            Ok(()) => ok = true,
            Err(e) => last_err = Some(e),
        }
    }
    match (ok, last_err) {
        (true, _) => Ok(()),
        (false, Some(e)) => Err(e),
        (false, None) => Err(Error::new(windows::Win32::Foundation::E_FAIL, "no Bluetooth audio filter found")),
    }
}

/// Device id of the KS filter the endpoint's first connector is attached to.
fn connected_filter_id(endpoint: &IMMDevice) -> Option<String> {
    unsafe {
        let topology: IDeviceTopology = endpoint.Activate(CLSCTX_ALL, None).ok()?;
        let connector: IConnector = topology.GetConnector(0).ok()?;
        let peer: IConnector = connector.GetConnectedTo().ok()?;
        let part: IPart = peer.cast().ok()?;
        let filter_topology = part.GetTopologyObject().ok()?;
        take_pwstr(filter_topology.GetDeviceId().ok()?)
    }
}

fn is_bluetooth(filter_id: &str) -> bool {
    let id = filter_id.to_lowercase();
    id.contains("bthenum") || id.contains("bthhfenum") || id.contains("bthleenum")
}

fn friendly_name(device: &IMMDevice) -> Option<String> {
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        Some(value.to_string())
    }
}

/// "ヘッドホン (AirPods Pro - Find My Hands-Free)" -> "AirPods Pro - Find My"
fn device_name_from_endpoint(endpoint_name: &str) -> String {
    let inner = match (endpoint_name.find('('), endpoint_name.rfind(')')) {
        (Some(l), Some(r)) if l < r => &endpoint_name[l + 1..r],
        _ => endpoint_name,
    };
    inner.trim_end_matches(" Hands-Free").trim().to_string()
}

unsafe fn take_pwstr(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let s = unsafe { p.to_string().ok() };
    unsafe { CoTaskMemFree(Some(p.0 as _)) };
    s
}
