use mecha_wayland::prelude::*;

use super::nm_device_state;
use super::trace::wifi_trace;

/// Wi-Fi availability and connection status for the selected device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiState {
    /// Networking is disabled, hardware is blocked, or no usable device is selected.
    Unavailable,
    /// Wi-Fi is available but WirelessEnabled is false.
    Off,
    /// Wi-Fi is enabled, but connection status is not yet known.
    On,
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

/// Controls the tile's state-specific presentation and enabled indicator.
/// Connected presentation uses the latest APUpdate for SSID and strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateUpdate {
    pub state: WifiState,
}
impl Event for StateUpdate {}

/// Complete AP information for the selected device, independent of connection state.
/// The UI's current WifiState takes precedence over strength-based icons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct APUpdate {
    /// Raw SSID bytes, not necessarily UTF-8. None clears the previous AP.
    /// Some(empty) means an AP is tracked but its SSID is empty or not yet known.
    pub ssid: Option<Vec<u8>>,
    /// Signal strength from 0 to 100; zero also represents an unknown strength.
    pub strength: u8,
}
impl Event for APUpdate {}

/// Map a NetworkManager device state and broadcast it to the UI.
/// The caller checks device selection and manager flags before emitting.
#[track_caller]
pub(super) fn emit_device_state_update(app: &mut App, device_state: u32) {
    let state = match device_state {
        nm_device_state::UNMANAGED | nm_device_state::UNAVAILABLE => WifiState::Unavailable,
        nm_device_state::DISCONNECTED | nm_device_state::FAILED => WifiState::Disconnected,
        nm_device_state::PREPARE
        | nm_device_state::CONFIG
        | nm_device_state::NEED_AUTH
        | nm_device_state::IP_CONFIG
        | nm_device_state::IP_CHECK
        | nm_device_state::SECONDARIES => WifiState::Connecting,
        nm_device_state::ACTIVE => WifiState::Connected,
        nm_device_state::DEACTIVATING => WifiState::Disconnecting,
        _ => WifiState::On,
    };
    emit_state_update(app, state);
}

/// Broadcast a selected-device state update to the UI.
/// The caller decides when to emit, including when the selected device changes.
#[track_caller]
pub(super) fn emit_state_update(app: &mut App, state: WifiState) {
    wifi_trace!("-> UI StateUpdate: {state:?}");
    app.emit(
        StateUpdate { state },
        app.descendants(app.root()).collect::<Vec<_>>(),
    );
}

/// Broadcast a complete AP update. Pass None to clear AP information.
/// These helpers emit every call so a device switch can refresh both UI events
/// even when the newly selected device has the same values as the previous one.
#[track_caller]
pub(super) fn emit_ap_update(app: &mut App, ssid: Option<Vec<u8>>, strength: u8) {
    let strength = if ssid.is_some() { strength } else { 0 };
    wifi_trace!(
        "-> UI APUpdate: SSID={:?}, strength={strength}",
        ssid.as_deref().map(String::from_utf8_lossy)
    );
    app.emit(
        APUpdate { ssid, strength },
        app.descendants(app.root()).collect::<Vec<_>>(),
    );
}
