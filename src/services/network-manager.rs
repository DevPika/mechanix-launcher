use mecha_wayland::dbus::{Decode, Type};
use mecha_wayland::prelude::*;
use std::collections::HashMap;

const NM: &str = "org.freedesktop.NetworkManager";
const NM_MAIN_OBJ: &str = "/org/freedesktop/NetworkManager";
const NM_DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const NM_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";

// enum
mod nm_device_type {
    type EType = u32;
    const UNKNOWN: EType = 0;
    pub(super) const WIFI: EType = 2;
}

mod nm_device_state {
    type EType = u32;
    pub(super) const UNKNOWN: EType = 0;
    pub(super) const ACTIVE: EType = 100;
}

enum CallType {
    GetAll, // GetAll on NM_MAIN_OBJ
    /// GetAll on ObjectPath of a a device
    /// Use to determine ActiveConnection and DeviceType
    DeviceProperties(ObjectPath),
    /// Get ActiveAccessPoint of a device described by ObjectPath
    /// This is only called once a WifiDevice is confirmed
    ActiveAccessPoint(ObjectPath),
}

enum Subscription {
    /// Device.StateChanged; contains the owning device's object path.
    DeviceState(ObjectPath),
    /// Device.Wireless.PropertiesChanged; contains the owning device's object path.
    WirelessProperties(ObjectPath),
    /// AccessPoint.PropertiesChanged; contains the owning device's object path.
    AccessPointProperties(ObjectPath),
}

struct WifiDevice {
    /// Device.StateChanged subscription.
    /// Duplicates the key in NMState.subscriptions for removal and unsubscription.
    state_changed: Option<MatchId>,
    /// Device.Wireless.PropertiesChanged subscription.
    /// Duplicates the key in NMState.subscriptions for removal and unsubscription.
    wireless_properties_changed: Option<MatchId>,
    /// Tracked access point while the device is activated.
    /// Unregister its subscription before clearing or replacing it.
    active_ap: Option<ActiveAccessPoint>,
}

struct ActiveAccessPoint {
    /// Object path of this device's active access point.
    path: ObjectPath,
    /// Raw SSID bytes; not necessarily valid UTF-8.
    ssid: Vec<u8>,
    /// Signal strength as a percentage from 0 to 100.
    strength: u8,
    /// AccessPoint PropertiesChanged match for tracking SSID and strength.
    /// Duplicates the key in NMState.subscriptions for removal and unsubscription.
    properties_changed: Option<MatchId>,
}

pub struct NMState {
    /// is networking enabled
    pub networking_enabled: bool,
    /// if wireless hardware is enabled (useful for kill switch)
    pub wireless_hardware_enabled: bool,
    /// if wireless networking is enabled
    /// only relevant if networking_enabled and wireless_hardware_enabled are
    /// also true. Toggle this on and off in DBus to enable and disable wifi
    pub wireless_enabled: bool,
    /// Discovered Wi-Fi devices keyed by their object paths.
    wifi_devices: HashMap<ObjectPath, WifiDevice>,
    /// Routes device and access point subscriptions to their owning Wi-Fi devices.
    subscriptions: HashMap<MatchId, Subscription>,
    /// Match for main-object PropertiesChanged on the NetworkManager interface.
    /// Take and unregister this match when clearing subscriptions for rediscovery.
    properties_changed: Option<MatchId>,
    /// Match for DeviceAdded on the NetworkManager main object.
    /// Take and unregister this match when clearing subscriptions for rediscovery.
    device_added: Option<MatchId>,
    /// Match for DeviceRemoved on the NetworkManager main object.
    /// Take and unregister this match when clearing subscriptions for rediscovery.
    device_removed: Option<MatchId>,
    pending: HashMap<CallId, CallType>,
}
impl Resource for NMState {}

pub struct NetworkManagerModule;
impl Module for NetworkManagerModule {
    fn install(self, app: &mut App) {
        let (properties_changed, device_added, device_removed, call) = {
            let mut dbus = app.resource_mut::<DBus>();
            let mut bus = dbus.system();
            let properties_changed = bus.watch_properties(NM, NM_MAIN_OBJ, NM);
            let device_added = bus.add_match(
                MatchRule::signal()
                    .sender(NM)
                    .path(NM_MAIN_OBJ)
                    .interface(NM)
                    .member("DeviceAdded"),
            );
            let device_removed = bus.add_match(
                MatchRule::signal()
                    .sender(NM)
                    .path(NM_MAIN_OBJ)
                    .interface(NM)
                    .member("DeviceRemoved"),
            );
            let call = bus.object(NM, NM_MAIN_OBJ).get_all(NM);
            (properties_changed, device_added, device_removed, call)
        };

        app.insert_resource(NMState {
            networking_enabled: false,
            wireless_hardware_enabled: false,
            wireless_enabled: false,
            wifi_devices: HashMap::new(),
            subscriptions: HashMap::new(),
            properties_changed: Some(properties_changed),
            device_added: Some(device_added),
            device_removed: Some(device_removed),
            pending: HashMap::from([(call, CallType::GetAll)]),
        });
        app.system(on_method_reply);
    }
}

fn on_method_reply(app: &mut App, event: &MethodReply) {
    let (call_type, message) = {
        let mut state = app.resource_mut::<NMState>();
        let Some(call_type) = state.pending.remove(&event.call) else {
            return; // another module's DBus call
        };

        let message = match &event.result {
            Ok(message) => message,
            // cleanly handle network manager not being available
            Err(error)
                if error.is("org.freedesktop.DBus.Error.ServiceUnknown")
                    || error.is("org.freedesktop.DBus.Error.NameHasNoOwner") =>
            {
                eprintln!("NetworkManager unavailable, clearing all pending calls");
                state.pending.clear();
                return;
            }
            Err(error) => {
                eprintln!("NetworkManager request failed: {error:?}");
                return;
            }
        };

        (call_type, message)
    };

    // handle messages
    match call_type {
        CallType::GetAll => {
            let (properties,) = match message.body::<(PropMap,)>() {
                Ok(body) => body,
                Err(_) => {
                    println!("NetworkManager: invalid GetAll reply");
                    return;
                }
            };
            let mut state = app.resource_mut::<NMState>();

            // read necessary state
            state.networking_enabled = unwrap_property(&properties, "NetworkingEnabled");
            state.wireless_hardware_enabled =
                unwrap_property(&properties, "WirelessHardwareEnabled");
            state.wireless_enabled = unwrap_property(&properties, "WirelessEnabled");

            // Todo: Event here

            // Request all properties on devices
            let devices: Vec<ObjectPath> = unwrap_property(&properties, "Devices");
            for path in devices {
                let call = app
                    .resource_mut::<DBus>()
                    .system()
                    .object(NM, path.clone())
                    .get_all(NM_DEVICE);
                app.resource_mut::<NMState>()
                    .pending
                    .insert(call, CallType::DeviceProperties(path));
            }
        }
        CallType::DeviceProperties(path) => {
            let (properties,) = match message.body::<(PropMap,)>() {
                Ok(body) => body,
                Err(error) => {
                    eprintln!("NetworkManager: invalid device GetAll reply: {error:?}");
                    return;
                }
            };

            let device_type: u32 = unwrap_property(&properties, "DeviceType");
            if device_type != nm_device_type::WIFI {
                return;
            }

            let device_state: u32 = unwrap_property(&properties, "State");
            if device_state == nm_device_state::ACTIVE {
                let call = app
                    .resource_mut::<DBus>()
                    .system()
                    .object(NM, path.clone())
                    .get_property(NM_WIRELESS, "ActiveAccessPoint");
                app.resource_mut::<NMState>()
                    .pending
                    .insert(call, CallType::ActiveAccessPoint(path.clone()));
            };

            app.resource_mut::<NMState>().wifi_devices.insert(
                path,
                WifiDevice {
                    state_changed: None,
                    wireless_properties_changed: None,
                    active_ap: None,
                },
            );
        }
        _ => {}
    }
}

/// Helper function
/// Decode a property, logging and returning its default if missing or invalid.
fn unwrap_property<T: Decode + Type + Default>(properties: &PropMap, name: &str) -> T {
    match properties.get_as::<T>(name) {
        Some(Ok(value)) => value,
        Some(Err(error)) => {
            eprintln!("NetworkManager: invalid {name}: {error:?}");
            T::default()
        }
        None => {
            eprintln!("NetworkManager: missing {name}");
            T::default()
        }
    }
}

#[cfg(test)]
mod tests;
