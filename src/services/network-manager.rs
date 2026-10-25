use mecha_wayland::dbus::{Decode, Type};
use mecha_wayland::prelude::*;
use std::collections::HashMap;

const NM: &str = "org.freedesktop.NetworkManager";
const NM_MAIN_OBJ: &str = "/org/freedesktop/NetworkManager";
const NM_DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const NM_WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const NM_ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";

// enum
mod nm_device_type {
    type EType = u32;
    const UNKNOWN: EType = 0;
    pub(super) const WIFI: EType = 2;
}

mod nm_device_state {
    type EType = u32;
    pub(super) const UNKNOWN: EType = 0;
    /// has a local or global network connection
    pub(super) const ACTIVE: EType = 100;
    /// in the process of disconnecting from a network connection
    pub(super) const DEACTIVATING: EType = 110;
    /// disconnected and can connect to another network
    pub(super) const DISCONNECTED: EType = 30;
}

enum CallType {
    GetAll, // GetAll on NM_MAIN_OBJ
    /// GetAll on ObjectPath of a a device
    /// Use to determine ActiveConnection and DeviceType
    DeviceProperties(ObjectPath),
    /// Get ActiveAccessPoint of a device described by ObjectPath
    /// This is only called once a WifiDevice is confirmed
    ActiveAccessPoint(ObjectPath),
    /// GetAll on the access point itself to get strength and SSID
    AccessPointProperties {
        device_path: ObjectPath,
        access_point_path: ObjectPath,
    },
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
    /// Wi-Fi device selected for UI events; the first discovered device is kept.
    current_device: Option<ObjectPath>,
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
            current_device: None,
            subscriptions: HashMap::new(),
            properties_changed: Some(properties_changed),
            device_added: Some(device_added),
            device_removed: Some(device_removed),
            pending: HashMap::from([(call, CallType::GetAll)]),
        });
        app.system(on_method_reply);
        app.system(on_signal_received);
    }
}

fn on_signal_received(app: &mut App, event: &SignalReceived) {
    // thanks LLM, very important guard
    if event.bus != BusId::System {
        return;
    }

    for match_id in &event.matches {
        let mut state = app.resource_mut::<NMState>();
        if state.device_added == Some(*match_id) {
            let (path,) = match event.msg.body::<(ObjectPath,)>() {
                Ok(body) => body,
                Err(error) => {
                    eprintln!("NetworkManager: invalid DeviceAdded: {error:?}");
                    return;
                }
            };
            let call = app
                .resource_mut::<DBus>()
                .system()
                .object(NM, path.clone())
                .get_all(NM_DEVICE);
            app.resource_mut::<NMState>()
                .pending
                .insert(call, CallType::DeviceProperties(path));
            continue;
        }
        if state.device_removed == Some(*match_id) {
            let (path,) = match event.msg.body::<(ObjectPath,)>() {
                Ok(body) => body,
                Err(error) => {
                    eprintln!("NetworkManager: invalid DeviceRemoved: {error:?}");
                    return;
                }
            };

            // remove stored matches for this device
            let matches = match state.wifi_devices.remove(&path) {
                Some(device) => [
                    device.state_changed,
                    device.wireless_properties_changed,
                    device.active_ap.and_then(|ap| ap.properties_changed),
                ],
                None => [None; 3], // don't continue here, pending calls cleanup needs to happen
            };
            for id in matches.iter().flatten() {
                state.subscriptions.remove(id);
            }

            /*
            Remove pending calls.

            This is to guard against a very rare case: if NetworkManager sends
            something to DBus, and before DBus sends it to us, we receive a
            DeviceRemoved.

            We should then clean up all of these things so that DBus doesn't
            send us data from an expired device
            */
            state.pending.retain(|_, call| match call {
                CallType::DeviceProperties(device_path)
                | CallType::ActiveAccessPoint(device_path)
                | CallType::AccessPointProperties { device_path, .. } => device_path != &path,
                CallType::GetAll => true,
            });

            // modify current device to next device
            // TODO: do some sorting here to prefer something that's Active
            if state.current_device.as_ref() == Some(&path) {
                state.current_device = state.wifi_devices.keys().next().cloned();
            }

            let mut dbus = app.resource_mut::<DBus>();
            let mut bus = dbus.system();
            for id in matches.into_iter().flatten() {
                bus.remove_match(id);
            }

            // TODO: send events here
            continue;
        }
        if state.properties_changed == Some(*match_id) {
            let (_, properties, invalidated) = match PropertiesChanged::parse(&event.msg) {
                Ok(body) => body,
                Err(error) => {
                    eprintln!("NetworkManager: invalid PropertiesChanged: {error:?}");
                    return;
                }
            };
            let [
                networking_enabled,
                wireless_hardware_enabled,
                wireless_enabled,
            ] = [
                "NetworkingEnabled",
                "WirelessHardwareEnabled",
                "WirelessEnabled",
            ]
            .map(|name| {
                if invalidated.iter().any(|property| property == name) {
                    Some(false) // if it's invalidated, set to false
                } else {
                    changed_property::<bool>(&properties, name)
                }
            });
            if let Some(enabled) = networking_enabled {
                state.networking_enabled = enabled;
            }
            if let Some(enabled) = wireless_hardware_enabled {
                state.wireless_hardware_enabled = enabled;
            }
            if let Some(enabled) = wireless_enabled {
                state.wireless_enabled = enabled;
            }

            // TODO: events here
            continue;
        }

        let Some(subscription) = state.subscriptions.get(match_id) else {
            continue;
        };

        match subscription {
            Subscription::DeviceState(_path) => {}
            Subscription::WirelessProperties(_path) => {}
            Subscription::AccessPointProperties(_path) => {}
        }
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
                // TODO: clear all Wifi related things as well, not just the
                // pending list
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
            // return if the device is a wifi device or if this device already
            // exists in the hash map.
            // the device can exist in the hash map if DeviceAdded's signal ran
            // before this GetAll on this device.
            if device_type != nm_device_type::WIFI
                || app
                    .resource_mut::<NMState>()
                    .wifi_devices
                    .contains_key(&path)
            {
                return;
            }

            // subscribe to state change for tracking and NM_WIRELESS
            // PropertiesChanged for AccessPoint changes
            let mut dbus = app.resource_mut::<DBus>();
            let mut dbus = dbus.system();
            let state_changed = dbus.add_match(
                MatchRule::signal()
                    .sender(NM)
                    .path(path.clone())
                    .interface(NM_DEVICE)
                    .member("StateChanged"),
            );
            let wireless_properties_changed = dbus.watch_properties(NM, path.clone(), NM_WIRELESS);

            let mut state = app.resource_mut::<NMState>();
            state
                .subscriptions
                .insert(state_changed, Subscription::DeviceState(path.clone()));
            state.subscriptions.insert(
                wireless_properties_changed,
                Subscription::WirelessProperties(path.clone()),
            );
            state.wifi_devices.insert(
                path.clone(),
                WifiDevice {
                    state_changed: Some(state_changed),
                    wireless_properties_changed: Some(wireless_properties_changed),
                    active_ap: None,
                },
            );
            if state.current_device.is_none() {
                state.current_device = Some(path.clone());
            }

            // if we're currently connected, get access point information
            let device_state: u32 = unwrap_property(&properties, "State");
            if device_state == nm_device_state::ACTIVE {
                let call = app
                    .resource_mut::<DBus>()
                    .system()
                    .object(NM, path.clone())
                    .get_property(NM_WIRELESS, "ActiveAccessPoint");
                app.resource_mut::<NMState>()
                    .pending
                    .insert(call, CallType::ActiveAccessPoint(path));
            };
        }
        CallType::ActiveAccessPoint(path) => {
            // Properties.Get returns a variant containing the object path.
            let access_point_path = match message
                .body::<(Value,)>()
                .and_then(|(value,)| value.get::<ObjectPath>())
            {
                Ok(access_point_path) => access_point_path,
                Err(error) => {
                    eprintln!(
                        "NetworkManager: invalid ActiveAccessPoint reply for {path:?}: {error:?}"
                    );
                    return;
                }
            };
            /*
            An LLM has told me that the access point path can just be "/", even
            if the state is ACTIVE, so check for that here
            */
            if access_point_path.as_str() == "/" {
                return;
            }

            if app
                .resource::<NMState>()
                .wifi_devices
                .get(&path)
                .is_some_and(|device| device.active_ap.is_some())
            {
                return; // make sure access point hasn't changed from beneath us while initialising
            }

            // Subscribe to PropertiesChanged to track this access point's SSID and strength.
            let properties_changed = app.resource_mut::<DBus>().system().watch_properties(
                NM,
                access_point_path.clone(),
                NM_ACCESS_POINT,
            );
            let mut state = app.resource_mut::<NMState>();
            let Some(device) = state.wifi_devices.get_mut(&path) else {
                // this else guard should never run ideally
                // It could run if device is removed while getting this
                // information
                // This is just efficient book keeping, DeviceRemoved handler
                // handles this either way
                app.resource_mut::<DBus>()
                    .system()
                    .remove_match(properties_changed);
                return;
            };
            device.active_ap = Some(ActiveAccessPoint {
                path: access_point_path.clone(),
                ssid: Vec::new(),
                strength: 0,
                properties_changed: Some(properties_changed),
            });
            state.subscriptions.insert(
                properties_changed,
                Subscription::AccessPointProperties(path.clone()),
            );

            let call = app
                .resource_mut::<DBus>()
                .system()
                .object(NM, access_point_path.clone())
                .get_all(NM_ACCESS_POINT);
            app.resource_mut::<NMState>().pending.insert(
                call,
                CallType::AccessPointProperties {
                    device_path: path,
                    access_point_path,
                },
            );
        }
        CallType::AccessPointProperties {
            // getall on access point
            device_path,
            access_point_path,
        } => {
            let (properties,) = match message.body::<(PropMap,)>() {
                Ok(body) => body,
                Err(error) => {
                    eprintln!("NetworkManager: invalid access point GetAll reply: {error:?}");
                    return;
                }
            };

            let mut state = app.resource_mut::<NMState>();
            let Some(device) = state.wifi_devices.get_mut(&device_path) else {
                return;
            };
            let Some(access_point) = device.active_ap.as_mut() else {
                return;
            };
            if access_point.path != access_point_path {
                return; // if access point has changed during this point
            }

            access_point.ssid = unwrap_property(&properties, "Ssid");
            access_point.strength = unwrap_property(&properties, "Strength");
        }
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

fn changed_property<T: Decode + Type>(properties: &PropMap, name: &str) -> Option<T> {
    match properties.get_as::<T>(name) {
        Some(Ok(value)) => Some(value),
        Some(Err(error)) => {
            eprintln!("NetworkManager: invalid {name}: {error:?}");
            None
        }
        None => None,
    }
}

#[cfg(test)]
mod tests;
