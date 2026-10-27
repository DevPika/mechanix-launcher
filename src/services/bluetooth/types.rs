use std::collections::HashMap;

use mecha_wayland::prelude::*;

use super::interface::{ADAPTER, DEVICE};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BluetoothState {
    /// `name` is the device alias when exactly one device is connected.
    Connected {
        count: usize,
        name: Option<String>,
    },
    On,
    #[default]
    Off,
}

pub(super) struct Matches {
    pub(super) properties: MatchId,
    pub(super) interfaces_added: MatchId,
    pub(super) interfaces_removed: MatchId,
    pub(super) owner: MatchId,
}

/// Latest observed Bluetooth state, derived from BlueZ adapters and devices.
pub struct BluetoothStatus {
    pub state: BluetoothState,
    pub(super) observed: BluetoothObservation,
    pub(super) pending: Option<CallId>,
    pub(super) dirty: bool,
    pub(super) matches: Matches,
}
impl Resource for BluetoothStatus {}

pub(super) type ManagedObjects = HashMap<ObjectPath, HashMap<String, PropMap>>;

/// Known BlueZ device; aliases are kept for disconnected devices so a later
/// `Connected` signal, which carries no name, can still be labelled.
#[derive(Debug, Default)]
pub(super) struct Device {
    pub(super) alias: String,
    pub(super) connected: bool,
}

#[derive(Default)]
pub(super) struct BluetoothObservation {
    pub(super) adapters: HashMap<ObjectPath, bool>,
    pub(super) devices: HashMap<ObjectPath, Device>,
}

impl BluetoothObservation {
    pub(super) fn from_managed_objects(objects: ManagedObjects) -> Self {
        let mut observed = Self::default();
        for (path, interfaces) in objects {
            observed.add_interfaces(path, interfaces);
        }
        observed
    }

    fn add_interfaces(&mut self, path: ObjectPath, interfaces: HashMap<String, PropMap>) {
        if let Some(props) = interfaces.get(ADAPTER) {
            self.adapters.insert(
                path.clone(),
                props
                    .get_as::<bool>("Powered")
                    .and_then(Result::ok)
                    .unwrap_or(false),
            );
        }
        if let Some(props) = interfaces.get(DEVICE) {
            self.devices.insert(
                path,
                Device {
                    alias: props
                        .get_as::<String>("Alias")
                        .and_then(Result::ok)
                        .unwrap_or_default(),
                    connected: props.get_as::<bool>("Connected").and_then(Result::ok) == Some(true),
                },
            );
        }
    }

    pub(super) fn connected_count(&self) -> usize {
        self.devices
            .values()
            .filter(|device| device.connected)
            .count()
    }

    pub(super) fn apply_change(&mut self, change: BluetoothChange) -> bool {
        match change {
            BluetoothChange::Added(path, interfaces) => self.add_interfaces(path, interfaces),
            BluetoothChange::Removed(path, interfaces) => {
                for interface in interfaces {
                    match interface.as_str() {
                        ADAPTER => {
                            self.adapters.remove(&path);
                        }
                        DEVICE => {
                            self.devices.remove(&path);
                        }
                        _ => {}
                    }
                }
            }
            BluetoothChange::Properties(path, props) => {
                for prop in props {
                    match prop {
                        PropChange::Powered(Some(powered)) => {
                            self.adapters.insert(path.clone(), powered);
                        }
                        PropChange::Connected(Some(connected)) => {
                            self.devices.entry(path.clone()).or_default().connected = connected;
                        }
                        PropChange::Alias(Some(alias)) => {
                            self.devices.entry(path.clone()).or_default().alias = alias;
                        }
                        PropChange::Powered(None)
                        | PropChange::Connected(None)
                        | PropChange::Alias(None) => return false,
                    }
                }
            }
        }
        true
    }

    pub(super) fn state(&self) -> BluetoothState {
        if !self.adapters.values().any(|&powered| powered) {
            BluetoothState::Off
        } else {
            let mut connected = self.devices.values().filter(|device| device.connected);
            match (connected.next(), connected.next()) {
                (None, _) => BluetoothState::On,
                (Some(device), None) => BluetoothState::Connected {
                    count: 1,
                    name: (!device.alias.is_empty()).then(|| device.alias.clone()),
                },
                (Some(_), Some(_)) => BluetoothState::Connected {
                    count: self.connected_count(),
                    name: None,
                },
            }
        }
    }
}

pub(super) enum BluetoothChange {
    Added(ObjectPath, HashMap<String, PropMap>),
    Removed(ObjectPath, Vec<String>),
    Properties(ObjectPath, Vec<PropChange>),
}

/// A watched property; `None` means BlueZ invalidated it and a snapshot is required.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PropChange {
    Powered(Option<bool>),
    Connected(Option<bool>),
    Alias(Option<String>),
}
