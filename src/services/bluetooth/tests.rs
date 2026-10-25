use super::BluetoothState;
use super::backend::decode_change;
use super::interface::{ADAPTER, DEVICE, OBJECT_MANAGER};
use super::types::{BluetoothObservation, ManagedObjects};
use mecha_wayland::dbus::Value;
use mecha_wayland::prelude::*;
use std::collections::HashMap;

#[test]
fn snapshot_and_property_signals() {
    let adapter = ObjectPath::new("/org/bluez/hci0").unwrap();
    let device = ObjectPath::new("/org/bluez/hci0/dev_00_11_22_33_44_55").unwrap();
    let objects = ManagedObjects::from([
        (
            adapter.clone(),
            HashMap::from([(
                ADAPTER.into(),
                PropMap::from([("Powered".into(), Value::Bool(false))]),
            )]),
        ),
        (
            device.clone(),
            HashMap::from([(
                DEVICE.into(),
                PropMap::from([("Connected".into(), Value::Bool(false))]),
            )]),
        ),
    ]);
    let mut observed = BluetoothObservation::from_managed_objects(objects);
    assert_eq!(observed.state(), BluetoothState::Off);
    assert!(observed.connected_devices.is_empty());

    let property = |path: ObjectPath, interface: &str, name: &str, value: bool| {
        Message::signal(path, "org.freedesktop.DBus.Properties", "PropertiesChanged").with_body(&(
            interface,
            PropMap::from([(name.to_string(), Value::Bool(value))]),
            Vec::<String>::new(),
        ))
    };
    assert!(observed.apply_change(
        decode_change(&property(adapter.clone(), ADAPTER, "Powered", true)).unwrap()
    ));
    assert_eq!(observed.state(), BluetoothState::On);
    assert!(observed.apply_change(
        decode_change(&property(device.clone(), DEVICE, "Connected", true)).unwrap()
    ));
    assert_eq!(observed.state(), BluetoothState::Connected(1));
    assert_eq!(observed.connected_devices.len(), 1);
    assert!(
        observed
            .apply_change(decode_change(&property(device, DEVICE, "Connected", false)).unwrap())
    );
    assert_eq!(observed.state(), BluetoothState::On);
    assert!(observed.connected_devices.is_empty());
    assert!(decode_change(&property(adapter, ADAPTER, "Alias", true)).is_none());
}

#[test]
fn object_manager_removal_uses_body_path() {
    let adapter = ObjectPath::new("/org/bluez/hci0").unwrap();
    let device = ObjectPath::new("/org/bluez/hci0/dev_00_11_22_33_44_55").unwrap();
    let mut observed = BluetoothObservation::default();
    observed.adapters.insert(adapter.clone(), true);
    observed.connected_devices.insert(device.clone());

    let removed = |path: ObjectPath, interface: &str| {
        Message::signal("/", OBJECT_MANAGER, "InterfacesRemoved")
            .with_body(&(path, vec![interface.to_string()]))
    };
    let msg = removed(device, DEVICE);
    assert_eq!(msg.path().unwrap().as_str(), "/");
    assert!(observed.apply_change(decode_change(&msg).unwrap()));
    assert_eq!(observed.state(), BluetoothState::On);

    assert!(observed.apply_change(decode_change(&removed(adapter, ADAPTER)).unwrap()));
    assert_eq!(observed.state(), BluetoothState::Off);
}

#[test]
fn added_devices_keep_connected_until_last_one_is_removed() {
    let adapter = ObjectPath::new("/org/bluez/hci0").unwrap();
    let first = ObjectPath::new("/org/bluez/hci0/dev_00_11_22_33_44_55").unwrap();
    let second = ObjectPath::new("/org/bluez/hci0/dev_00_11_22_33_44_66").unwrap();
    let mut observed = BluetoothObservation::default();
    observed.adapters.insert(adapter, true);

    for (i, path) in [&first, &second].into_iter().enumerate() {
        let added = Message::signal("/", OBJECT_MANAGER, "InterfacesAdded").with_body(&(
            path.clone(),
            HashMap::from([(
                DEVICE.to_string(),
                PropMap::from([("Connected".into(), Value::Bool(true))]),
            )]),
        ));
        assert!(observed.apply_change(decode_change(&added).unwrap()));
        assert_eq!(observed.state(), BluetoothState::Connected(i + 1));
    }
    for (path, expected) in [
        (first, BluetoothState::Connected(1)),
        (second, BluetoothState::On),
    ] {
        let removed = Message::signal("/", OBJECT_MANAGER, "InterfacesRemoved")
            .with_body(&(path, vec![DEVICE.to_string()]));
        assert!(observed.apply_change(decode_change(&removed).unwrap()));
        assert_eq!(observed.state(), expected);
    }
}

#[test]
fn invalidation_requests_snapshot() {
    let adapter = ObjectPath::new("/org/bluez/hci0").unwrap();
    let mut observed = BluetoothObservation::default();
    observed.adapters.insert(adapter.clone(), true);
    let msg = Message::signal(
        adapter,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
    )
    .with_body(&(ADAPTER, PropMap::new(), vec!["Powered".to_string()]));
    assert!(!observed.apply_change(decode_change(&msg).unwrap()));
    assert_eq!(observed.state(), BluetoothState::On);
}

#[test]
fn adapter_power_toggle_transitions() {
    let adapter = ObjectPath::new("/org/bluez/hci0").unwrap();
    let mut observed = BluetoothObservation::default();
    observed.adapters.insert(adapter.clone(), true);
    assert_eq!(observed.state(), BluetoothState::On);

    let power_signal = |powered: bool| {
        Message::signal(
            adapter.clone(),
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        )
        .with_body(&(
            ADAPTER,
            PropMap::from([("Powered".to_string(), Value::Bool(powered))]),
            Vec::<String>::new(),
        ))
    };

    // Turning off
    assert!(observed.apply_change(decode_change(&power_signal(false)).unwrap()));
    assert_eq!(observed.state(), BluetoothState::Off);

    // Turning back on
    assert!(observed.apply_change(decode_change(&power_signal(true)).unwrap()));
    assert_eq!(observed.state(), BluetoothState::On);
}

#[test]
fn toggle_updates_only_after_bluez_property_signal() {
    let mut app = App::new();
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().system())
        .add_module(super::BluetoothModule);

    let adapter = ObjectPath::new("/org/bluez/hci1").unwrap();
    {
        let mut status = app.resource_mut::<super::BluetoothStatus>();
        status.observed.adapters.insert(adapter.clone(), false);
        status.pending = None;
    }
    super::backend::toggle(&mut app);
    assert_eq!(
        app.resource::<super::BluetoothStatus>().state,
        BluetoothState::Off
    );

    let watch = app.resource::<super::BluetoothStatus>().matches.properties;
    app.signal(SignalReceived {
        bus: BusId::System,
        matches: [watch].into_iter().collect(),
        msg: Message::signal(
            adapter,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        )
        .with_body(&(
            ADAPTER,
            PropMap::from([("Powered".into(), Value::Bool(true))]),
            Vec::<String>::new(),
        )),
    });
    app.flush();
    assert_eq!(
        app.resource::<super::BluetoothStatus>().state,
        BluetoothState::On
    );
}

#[test]
fn toggle_without_known_adapters_does_not_toggle_state() {
    let mut app = App::new();
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().system())
        .add_module(super::BluetoothModule);

    // Initial snapshot was requested on module install
    assert!(app.resource::<super::BluetoothStatus>().pending.is_some());

    // Toggle when no adapters are known should be ignored without changing state
    super::backend::toggle(&mut app);
    assert_eq!(
        app.resource::<super::BluetoothStatus>().state,
        BluetoothState::Off
    );

    // Discover adapter with Powered: false
    let call = app.resource::<super::BluetoothStatus>().pending.unwrap();
    app.signal(MethodReply {
        call,
        result: Ok(
            Message::method_return(call.serial).with_body(&(ManagedObjects::from([(
                ObjectPath::new("/org/bluez/hci1").unwrap(),
                HashMap::from([(
                    ADAPTER.into(),
                    PropMap::from([("Powered".into(), Value::Bool(false))]),
                )]),
            )]),)),
        ),
    });
    app.flush();

    let status = app.resource::<super::BluetoothStatus>();
    // Since the previous toggle was ignored, state remains Off
    assert_eq!(status.state, BluetoothState::Off);
    assert_eq!(status.observed.adapters.len(), 1);
}

#[test]
fn failed_snapshot_preserves_last_observed_state() {
    let mut app = App::new();
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().system())
        .add_module(super::BluetoothModule);

    let adapter = ObjectPath::new("/org/bluez/hci0").unwrap();
    {
        let mut status = app.resource_mut::<super::BluetoothStatus>();
        status.observed.adapters.insert(adapter, true);
        status.state = BluetoothState::On;
    }

    for result in [
        Err(CallError::Timeout),
        Ok(Message::method_return(0).with_body(&(42u32,))),
    ] {
        let call = app.resource::<super::BluetoothStatus>().pending.unwrap();
        app.signal(MethodReply { call, result });
        app.flush();
        let status = app.resource::<super::BluetoothStatus>();
        assert_eq!(status.state, BluetoothState::On);
        assert_eq!(status.observed.state(), BluetoothState::On);
        assert!(status.pending.is_none());
        super::backend::request_snapshot(&mut app);
    }

    let call = app.resource::<super::BluetoothStatus>().pending.unwrap();
    app.signal(MethodReply {
        call,
        result: Ok(Message::method_return(call.serial).with_body(&(ManagedObjects::new(),))),
    });
    app.flush();
    assert_eq!(
        app.resource::<super::BluetoothStatus>().state,
        BluetoothState::Off
    );
}
