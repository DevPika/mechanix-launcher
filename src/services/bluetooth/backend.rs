//! BlueZ client on mecha-wayland's system D-Bus connection.
use std::collections::HashMap;

use super::BluetoothStatus;
use super::interface::{ADAPTER, BLUEZ, DEVICE, OBJECT_MANAGER};
use super::types::{BluetoothChange, BluetoothObservation, BluetoothState, ManagedObjects};
use mecha_wayland::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BluetoothStateChanged {
    pub state: BluetoothState,
}
impl Event for BluetoothStateChanged {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleBluetooth;
impl Signal for ToggleBluetooth {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetBluetoothPowered(pub bool);
impl Signal for SetBluetoothPowered {}

pub(super) fn decode_change(msg: &Message) -> Option<BluetoothChange> {
    match msg.member()?.as_str() {
        "InterfacesAdded" => {
            let (path, interfaces) = msg.body::<(ObjectPath, HashMap<String, PropMap>)>().ok()?;
            (interfaces.contains_key(ADAPTER) || interfaces.contains_key(DEVICE))
                .then_some(BluetoothChange::Added(path, interfaces))
        }
        "InterfacesRemoved" => {
            let (path, interfaces) = msg.body::<(ObjectPath, Vec<String>)>().ok()?;
            interfaces
                .iter()
                .any(|name| name == ADAPTER || name == DEVICE)
                .then_some(BluetoothChange::Removed(path, interfaces))
        }
        "PropertiesChanged" => {
            let (interface, props, invalidated) = PropertiesChanged::parse(msg).ok()?;
            let (interface, key) = match interface.as_str() {
                ADAPTER => (ADAPTER, "Powered"),
                DEVICE => (DEVICE, "Connected"),
                _ => return None,
            };
            if !props.contains_key(key) && !invalidated.iter().any(|name| name == key) {
                return None;
            }
            let value = if invalidated.iter().any(|name| name == key) {
                None
            } else {
                props.get_as::<bool>(key).and_then(Result::ok)
            };
            Some(BluetoothChange::Property(
                msg.path()?.clone(),
                interface,
                value,
            ))
        }
        _ => None,
    }
}

pub(super) fn request_snapshot(app: &mut App) {
    if app.resource::<BluetoothStatus>().pending.is_some() {
        app.resource_mut::<BluetoothStatus>().dirty = true;
        return;
    }
    let call = app.resource_mut::<DBus>().system().object(BLUEZ, "/").call(
        OBJECT_MANAGER,
        "GetManagedObjects",
        &(),
    );
    let mut bluetooth = app.resource_mut::<BluetoothStatus>();
    bluetooth.pending = Some(call);
    bluetooth.dirty = false;
}

fn publish_state(app: &mut App, state: BluetoothState) {
    if app.resource::<BluetoothStatus>().state == state {
        return;
    }
    // eprintln!(
    //     "[bluetooth] state: {:?} -> {:?}",
    //     app.resource::<BluetoothStatus>().state,
    //     state
    // );
    app.resource_mut::<BluetoothStatus>().state = state;
    app.emit(
        BluetoothStateChanged { state },
        app.descendants(app.root()).collect::<Vec<_>>(),
    );
}

pub(super) fn on_snapshot_reply(app: &mut App, event: &MethodReply) {
    if app.resource::<BluetoothStatus>().pending != Some(event.call) {
        return;
    }
    let objects = event
        .result
        .as_ref()
        .map_err(|e| eprintln!("[bluetooth] GetManagedObjects failed: {e:?}"))
        .ok()
        .and_then(|message| {
            message
                .body::<(ManagedObjects,)>()
                .map_err(|e| {
                    eprintln!("[bluetooth] failed to parse GetManagedObjects reply: {e:?}")
                })
                .ok()
        });
    let (dirty, state) = {
        let mut bluetooth = app.resource_mut::<BluetoothStatus>();
        bluetooth.pending = None;
        if !bluetooth.dirty {
            if let Some((objects,)) = objects {
                bluetooth.observed = BluetoothObservation::from_managed_objects(objects);
            }
        }
        (bluetooth.dirty, bluetooth.observed.state())
    };
    if dirty {
        request_snapshot(app);
    } else {
        publish_state(app, state);
    }
}

pub(super) fn on_signal(app: &mut App, event: &SignalReceived) {
    let bluetooth = app.resource::<BluetoothStatus>();
    let owner = event.matches.contains(&bluetooth.matches.owner);
    if owner {
        if let Ok((_, _, new_owner)) = event.msg.body::<(String, String, String)>() {
            let mut bluetooth = app.resource_mut::<BluetoothStatus>();
            bluetooth.pending = None;
            bluetooth.dirty = false;
            bluetooth.observed = BluetoothObservation::default();
            publish_state(app, BluetoothState::Off);
            if !new_owner.is_empty() {
                request_snapshot(app);
            }
        }
    } else if event.matches.contains(&bluetooth.matches.properties)
        || event.matches.contains(&bluetooth.matches.interfaces_added)
        || event
            .matches
            .contains(&bluetooth.matches.interfaces_removed)
    {
        if let Some(change) = decode_change(&event.msg) {
            if app.resource::<BluetoothStatus>().pending.is_some() {
                // The snapshot may predate this signal; request a fresh one once it completes.
                request_snapshot(app);
            } else {
                let (applied, state) = {
                    let mut bluetooth = app.resource_mut::<BluetoothStatus>();
                    let applied = bluetooth.observed.apply_change(change);
                    (applied, bluetooth.observed.state())
                };
                if applied {
                    publish_state(app, state);
                } else {
                    request_snapshot(app);
                }
            }
        }
    }
}

pub(super) fn on_disconnect(app: &mut App, event: &BusDisconnected) {
    if event.bus == BusId::System {
        eprintln!("[bluetooth] system D-Bus disconnected; resetting state");
        let mut bluetooth = app.resource_mut::<BluetoothStatus>();
        bluetooth.pending = None;
        bluetooth.dirty = false;
        bluetooth.observed = BluetoothObservation::default();
        publish_state(app, BluetoothState::Off);
    }
}

/// Powers all known BlueZ adapters on or off; requests a snapshot if none are known.
pub fn set_powered(app: &mut App, powered: bool) {
    let paths: Vec<ObjectPath> = {
        let bluetooth = app.resource::<BluetoothStatus>();
        bluetooth.observed.adapters.keys().cloned().collect()
    };

    if paths.is_empty() {
        request_snapshot(app);
        return;
    }

    let mut dbus = app.resource_mut::<DBus>();
    for path in paths {
        dbus.system()
            .object(BLUEZ, path)
            .set_property(ADAPTER, "Powered", Value::Bool(powered));
    }
}

/// Toggles Bluetooth power; refreshes the adapter snapshot and no-ops if no adapters exist yet.
pub fn toggle(app: &mut App) {
    let bluetooth = app.resource::<BluetoothStatus>();
    if bluetooth.observed.adapters.is_empty() {
        if bluetooth.pending.is_none() {
            request_snapshot(app);
        }
        return;
    }
    let current_powered = bluetooth.state != BluetoothState::Off;
    set_powered(app, !current_powered);
}

pub(super) fn on_toggle_signal(app: &mut App, _: &ToggleBluetooth) {
    toggle(app);
}

pub(super) fn on_set_powered_signal(app: &mut App, signal: &SetBluetoothPowered) {
    set_powered(app, signal.0);
}
