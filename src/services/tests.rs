use super::*;
use std::io::Write;
use std::ops::{Deref, DerefMut};
use std::os::unix::net::UnixStream;

struct TestApp(App);

impl Deref for TestApp {
    type Target = App;

    fn deref(&self) -> &App {
        &self.0
    }
}

impl DerefMut for TestApp {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.0
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        // Drain pending I/O while DBus still owns its socket. App resources have
        // no guaranteed drop order; closing the socket first permits fd reuse
        // by another test before the old ring submits/cancels its queued reads.
        drop(self.0.insert_resource(Ring::new(8)));
    }
}

fn pending_call(app: &App, matches: impl Fn(&CallType) -> bool) -> CallId {
    let state = app.resource::<NMState>();
    let calls: Vec<_> = state
        .pending
        .iter()
        .filter(|(_, kind)| matches(kind))
        .map(|(call, _)| *call)
        .collect();
    assert_eq!(calls.len(), 1, "expected exactly one matching pending call");
    assert_eq!(calls[0].bus, BusId::System);
    calls[0]
}

fn device_call(app: &App, path: &ObjectPath) -> CallId {
    pending_call(
        app,
        |kind| matches!(kind, CallType::DeviceProperties(device) if device == path),
    )
}

fn active_ap_call(app: &App, path: &ObjectPath) -> CallId {
    pending_call(
        app,
        |kind| matches!(kind, CallType::ActiveAccessPoint(device) if device == path),
    )
}

fn ap_properties_call(app: &App, device: &ObjectPath, ap: &ObjectPath) -> CallId {
    pending_call(app, |kind| {
        matches!(kind, CallType::AccessPointProperties { device_path, access_point_path }
            if device_path == device && access_point_path == ap)
    })
}

fn reply(app: &mut App, call: CallId, result: Result<Message, CallError>) {
    assert!(app.resource::<NMState>().pending.contains_key(&call));
    app.signal(MethodReply { call, result });
    app.flush();
    assert!(!app.resource::<NMState>().pending.contains_key(&call));
}

fn reply_properties(app: &mut App, call: CallId, properties: PropMap) {
    reply(
        app,
        call,
        Ok(Message::method_return(call.serial).with_body(&(properties,))),
    );
}

fn manager_properties(devices: &[ObjectPath]) -> PropMap {
    PropMap::from([
        ("NetworkingEnabled".into(), Value::Bool(true)),
        ("WirelessHardwareEnabled".into(), Value::Bool(true)),
        ("WirelessEnabled".into(), Value::Bool(true)),
        (
            "Devices".into(),
            Value::Array {
                elem: "o".into(),
                items: devices.iter().cloned().map(Value::ObjectPath).collect(),
            },
        ),
    ])
}

fn discover(app: &mut App, devices: &[ObjectPath]) {
    let call = pending_call(app, |kind| matches!(kind, CallType::GetAll));
    reply_properties(app, call, manager_properties(devices));
    let state = app.resource::<NMState>();
    assert!(state.networking_enabled);
    assert!(state.wireless_hardware_enabled);
    assert!(state.wireless_enabled);
    assert!(state.wifi_devices.is_empty());
    assert!(state.subscriptions.is_empty());
    assert_eq!(state.pending.len(), devices.len());
    for path in devices {
        device_call(app, path);
    }
}

fn device_properties(device_type: u32, state: u32) -> PropMap {
    PropMap::from([
        ("DeviceType".into(), Value::U32(device_type)),
        ("State".into(), Value::U32(state)),
    ])
}

fn ssid_value(bytes: &[u8]) -> Value {
    Value::Array {
        elem: "y".into(),
        items: bytes.iter().copied().map(Value::U8).collect(),
    }
}

fn read_active_ap(app: &mut App, device: &ObjectPath, ap: &ObjectPath) {
    let call = active_ap_call(app, device);
    reply(
        app,
        call,
        Ok(Message::method_return(call.serial).with_body(&(Value::ObjectPath(ap.clone()),))),
    );
}

fn remove_device(app: &mut App, path: &ObjectPath) {
    let id = app.resource::<NMState>().device_removed.unwrap();
    app.signal(SignalReceived {
        bus: BusId::System,
        matches: [id].into_iter().collect(),
        msg: Message::signal(NM_MAIN_OBJ, NM, "DeviceRemoved")
            .with_sender(NM)
            .with_body(&(path.clone(),)),
    });
    app.flush();
}

fn add_device(app: &mut App, path: &ObjectPath) {
    let id = app.resource::<NMState>().device_added.unwrap();
    app.signal(SignalReceived {
        bus: BusId::System,
        matches: [id].into_iter().collect(),
        msg: Message::signal(NM_MAIN_OBJ, NM, "DeviceAdded")
            .with_sender(NM)
            .with_body(&(path.clone(),)),
    });
    app.flush();
}

fn connected_device(app: &mut App, device: &ObjectPath) {
    discover(app, std::slice::from_ref(device));
    let call = device_call(app, device);
    reply_properties(app, call, device_properties(2, 100));
    assert_device(app, device, None);
    assert_eq!(app.resource::<NMState>().subscriptions.len(), 2);
    assert_eq!(app.resource::<NMState>().pending.len(), 1);
    active_ap_call(app, device);
}

fn assert_device(app: &App, path: &ObjectPath, ap: Option<(&ObjectPath, &[u8], u8)>) {
    let state = app.resource::<NMState>();
    let device = state.wifi_devices.get(path).expect("missing Wi-Fi device");
    let state_match = device.state_changed.expect("missing state subscription");
    let wireless_match = device
        .wireless_properties_changed
        .expect("missing wireless subscription");
    assert_ne!(state_match, wireless_match);
    assert_eq!(state_match.bus, BusId::System);
    assert_eq!(wireless_match.bus, BusId::System);
    assert!(matches!(state.subscriptions.get(&state_match),
        Some(Subscription::DeviceState(owner)) if owner == path));
    assert!(matches!(state.subscriptions.get(&wireless_match),
        Some(Subscription::WirelessProperties(owner)) if owner == path));
    match ap {
        None => assert!(device.active_ap.is_none()),
        Some((ap_path, ssid, strength)) => {
            let ap = device.active_ap.as_ref().expect("missing access point");
            assert_eq!(&ap.path, ap_path);
            assert_eq!(ap.ssid, ssid);
            assert_eq!(ap.strength, strength);
            let ap_match = ap.properties_changed.expect("missing AP subscription");
            assert_eq!(ap_match.bus, BusId::System);
            assert_ne!(ap_match, state_match);
            assert_ne!(ap_match, wireless_match);
            assert!(matches!(state.subscriptions.get(&ap_match),
                Some(Subscription::AccessPointProperties(owner)) if owner == path));
        }
    }
}

fn setup() -> (TestApp, UnixStream) {
    // Supply the startup handshake over a private socket, not the system bus.
    let (client, mut peer) = UnixStream::pair().unwrap();
    peer.write_all(b"OK 0123456789abcdef0123456789abcdef\r\nERROR no fd passing\r\n")
        .unwrap();
    let hello = Message::method_return(1)
        .with_sender("org.freedesktop.DBus")
        .with_destination(":1.1")
        .with_body(&(":1.1",));
    peer.write_all(&hello.encode(1).0).unwrap();
    let mut app = App::new();
    app.add_module(RingModule::default())
        .add_module(DBusModule::new().over(BusId::System, client))
        .add_module(NetworkManagerModule);
    (TestApp(app), peer)
}

#[test]
fn get_all_reads_networking_hardware_and_wireless_flags_independently() {
    let (mut app, _peer) = setup();
    let call = *app.resource::<NMState>().pending.keys().next().unwrap();
    let cases = [
        (Some(Value::Bool(true)), true),
        (Some(Value::Bool(false)), false),
        (None, false),
        (Some(Value::Str("true".into())), false),
    ];
    for (networking, networking_expected) in cases.clone() {
        for (hardware, hardware_expected) in cases.clone() {
            for (wireless, wireless_expected) in cases.clone() {
                app.resource_mut::<NMState>()
                    .pending
                    .insert(call, CallType::GetAll);
                let mut properties = PropMap::new();
                if let Some(value) = networking.clone() {
                    properties.insert("NetworkingEnabled".into(), value);
                }
                if let Some(value) = hardware.clone() {
                    properties.insert("WirelessHardwareEnabled".into(), value);
                }
                if let Some(value) = wireless {
                    properties.insert("WirelessEnabled".into(), value);
                }
                app.signal(MethodReply {
                    call,
                    result: Ok(Message::method_return(call.serial).with_body(&(properties,))),
                });
                app.flush();

                let state = app.resource::<NMState>();
                assert_eq!(state.networking_enabled, networking_expected);
                assert_eq!(state.wireless_hardware_enabled, hardware_expected);
                assert_eq!(state.wireless_enabled, wireless_expected);
                assert!(state.pending.is_empty());
            }
        }
    }
}

#[test]
fn install_starts_with_disabled_flags_manager_matches_and_one_request() {
    let (app, _peer) = setup();
    let state = app.resource::<NMState>();
    assert!(!state.networking_enabled);
    assert!(!state.wireless_hardware_enabled);
    assert!(!state.wireless_enabled);
    assert!(state.wifi_devices.is_empty());
    assert!(state.current_device.is_none());
    assert!(state.subscriptions.is_empty());
    assert_eq!(state.pending.len(), 1);
    pending_call(&app, |kind| matches!(kind, CallType::GetAll));
    let ids = [
        state.properties_changed.unwrap(),
        state.device_added.unwrap(),
        state.device_removed.unwrap(),
    ];
    assert!(ids.iter().all(|id| id.bus == BusId::System));
    assert_eq!(
        ids.into_iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
}

#[test]
fn empty_missing_or_invalid_devices_leave_discovery_empty() {
    for devices in [
        Some(Value::Array {
            elem: "o".into(),
            items: Vec::new(),
        }),
        None,
        Some(Value::Str("not an object-path array".into())),
    ] {
        let (mut app, _peer) = setup();
        let mut properties = manager_properties(&[]);
        properties.remove("Devices");
        if let Some(devices) = devices {
            properties.insert("Devices".into(), devices);
        }
        let call = pending_call(&app, |kind| matches!(kind, CallType::GetAll));
        reply_properties(&mut app, call, properties);
        let state = app.resource::<NMState>();
        assert!(
            state.networking_enabled && state.wireless_hardware_enabled && state.wireless_enabled
        );
        assert!(state.pending.is_empty());
        assert!(state.wifi_devices.is_empty());
        assert!(state.current_device.is_none());
        assert!(state.subscriptions.is_empty());
    }
}

#[test]
fn failed_or_malformed_manager_reply_leaves_initial_state() {
    let failures = [
        Some(CallError::Remote {
            name: "org.freedesktop.DBus.Error.ServiceUnknown".into(),
            message: "NetworkManager is not installed".into(),
        }),
        Some(CallError::Remote {
            name: "org.freedesktop.DBus.Error.NameHasNoOwner".into(),
            message: "NetworkManager is not running".into(),
        }),
        Some(CallError::Timeout),
        Some(CallError::Disconnected),
        None,
    ];
    for failure in failures {
        let (mut app, _peer) = setup();
        let call = pending_call(&app, |kind| matches!(kind, CallType::GetAll));
        let result = match failure {
            Some(error) => Err(error),
            None => Ok(Message::method_return(call.serial).with_body(&(false,))),
        };
        reply(&mut app, call, result);
        let state = app.resource::<NMState>();
        assert!(!state.networking_enabled);
        assert!(!state.wireless_hardware_enabled);
        assert!(!state.wireless_enabled);
        assert!(state.pending.is_empty());
        assert!(state.wifi_devices.is_empty());
        assert!(state.current_device.is_none());
        assert!(state.subscriptions.is_empty());
        assert!(state.properties_changed.is_some());
        assert!(state.device_added.is_some());
        assert!(state.device_removed.is_some());
    }
}

#[test]
fn non_wifi_missing_or_invalid_device_type_is_not_stored() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    for device_type in [
        Some(Value::U32(0)),
        Some(Value::U32(1)),
        Some(Value::U32(5)),
        None,
        Some(Value::Str("2".into())),
    ] {
        let (mut app, _peer) = setup();
        discover(&mut app, std::slice::from_ref(&path));
        let call = device_call(&app, &path);
        let mut properties = PropMap::from([("State".into(), Value::U32(100))]);
        if let Some(device_type) = device_type {
            properties.insert("DeviceType".into(), device_type);
        }
        reply_properties(&mut app, call, properties);
        let state = app.resource::<NMState>();
        assert!(state.pending.is_empty());
        assert!(state.wifi_devices.is_empty());
        assert!(state.current_device.is_none());
        assert!(state.subscriptions.is_empty());
    }
}

#[test]
fn current_device_stays_none_until_a_wifi_device_is_confirmed() {
    let ethernet = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let wifi = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/2");
    for wifi_state in [30, 100] {
        let (mut app, _peer) = setup();
        assert!(app.resource::<NMState>().current_device.is_none());

        discover(&mut app, &[ethernet.clone(), wifi.clone()]);
        assert!(app.resource::<NMState>().current_device.is_none());
        assert_eq!(app.resource::<NMState>().pending.len(), 2);

        let call = device_call(&app, &ethernet);
        reply_properties(&mut app, call, device_properties(1, 100));
        let state = app.resource::<NMState>();
        assert!(state.current_device.is_none());
        assert!(state.wifi_devices.is_empty());
        assert!(state.subscriptions.is_empty());
        assert_eq!(state.pending.len(), 1);

        let call = device_call(&app, &wifi);
        reply_properties(&mut app, call, device_properties(2, wifi_state));
        let state = app.resource::<NMState>();
        assert_eq!(state.current_device.as_ref(), Some(&wifi));
        assert!(state.wifi_devices.contains_key(&wifi));
        assert_eq!(state.wifi_devices.len(), 1);
        assert_eq!(state.pending.len(), usize::from(wifi_state == 100));
        assert_device(&app, &wifi, None);
    }
}

#[test]
fn current_device_stays_none_when_all_device_reads_fail() {
    let malformed = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let timed_out = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/2");
    let disappeared = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/3");
    let (mut app, _peer) = setup();
    discover(
        &mut app,
        &[malformed.clone(), timed_out.clone(), disappeared.clone()],
    );
    assert!(app.resource::<NMState>().current_device.is_none());

    let malformed_call = device_call(&app, &malformed);
    let timed_out_call = device_call(&app, &timed_out);
    let disappeared_call = device_call(&app, &disappeared);
    let replies = [
        (
            disappeared_call,
            Err(CallError::Remote {
                name: "org.freedesktop.DBus.Error.UnknownObject".into(),
                message: "device disappeared".into(),
            }),
        ),
        (
            malformed_call,
            Ok(Message::method_return(malformed_call.serial).with_body(&(false,))),
        ),
        (timed_out_call, Err(CallError::Timeout)),
    ];
    for (index, (call, result)) in replies.into_iter().enumerate() {
        reply(&mut app, call, result);
        let state = app.resource::<NMState>();
        assert!(state.current_device.is_none());
        assert!(state.wifi_devices.is_empty());
        assert!(state.subscriptions.is_empty());
        assert_eq!(state.pending.len(), 2 - index);
    }
}

#[test]
fn wifi_state_controls_initial_ap_read_but_not_device_subscriptions() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let mut cases: Vec<_> = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120]
        .into_iter()
        .map(|state| (Some(Value::U32(state)), state == 100))
        .collect();
    cases.extend([(None, false), (Some(Value::Str("100".into())), false)]);
    for (value, activated) in cases {
        let (mut app, _peer) = setup();
        discover(&mut app, std::slice::from_ref(&path));
        let call = device_call(&app, &path);
        let mut properties = PropMap::from([("DeviceType".into(), Value::U32(2))]);
        if let Some(value) = value {
            properties.insert("State".into(), value);
        }
        reply_properties(&mut app, call, properties);
        assert_device(&app, &path, None);
        let state = app.resource::<NMState>();
        assert_eq!(state.wifi_devices.len(), 1);
        assert_eq!(state.subscriptions.len(), 2);
        assert_eq!(state.pending.len(), usize::from(activated));
        if activated {
            active_ap_call(&app, &path);
        }
    }
}

#[test]
fn mixed_devices_initialize_independently_with_out_of_order_replies() {
    let first = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let second = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/2");
    let disconnected = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/3");
    let ethernet = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/4");
    let first_ap = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/10");
    let second_ap = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/20");
    let (mut app, _peer) = setup();
    discover(
        &mut app,
        &[
            first.clone(),
            second.clone(),
            disconnected.clone(),
            ethernet.clone(),
        ],
    );
    for (path, device_type, state) in [
        (&disconnected, 2, 30),
        (&second, 2, 100),
        (&ethernet, 1, 100),
        (&first, 2, 100),
    ] {
        let call = device_call(&app, path);
        reply_properties(&mut app, call, device_properties(device_type, state));
        assert_eq!(
            app.resource::<NMState>().current_device.as_ref(),
            Some(&disconnected)
        );
    }
    assert_eq!(app.resource::<NMState>().wifi_devices.len(), 3);
    assert!(
        !app.resource::<NMState>()
            .wifi_devices
            .contains_key(&ethernet)
    );
    assert_eq!(app.resource::<NMState>().subscriptions.len(), 6);
    assert_eq!(app.resource::<NMState>().pending.len(), 2);
    read_active_ap(&mut app, &first, &first_ap);
    read_active_ap(&mut app, &second, &second_ap);
    assert_device(&app, &first, Some((&first_ap, b"", 0)));
    assert_device(&app, &second, Some((&second_ap, b"", 0)));
    assert_eq!(app.resource::<NMState>().subscriptions.len(), 8);
    assert_eq!(app.resource::<NMState>().pending.len(), 2);

    let call = ap_properties_call(&app, &second, &second_ap);
    reply_properties(
        &mut app,
        call,
        PropMap::from([
            ("Ssid".into(), ssid_value(b"Guest")),
            ("Strength".into(), Value::U8(28)),
        ]),
    );
    assert_device(&app, &first, Some((&first_ap, b"", 0)));
    assert_device(&app, &second, Some((&second_ap, b"Guest", 28)));
    assert_eq!(app.resource::<NMState>().pending.len(), 1);

    let call = ap_properties_call(&app, &first, &first_ap);
    reply_properties(
        &mut app,
        call,
        PropMap::from([
            ("Ssid".into(), ssid_value(b"Office\xff\0")),
            ("Strength".into(), Value::U8(93)),
        ]),
    );
    assert_device(&app, &first, Some((&first_ap, b"Office\xff\0", 93)));
    assert_device(&app, &second, Some((&second_ap, b"Guest", 28)));
    assert_device(&app, &disconnected, None);
    let state = app.resource::<NMState>();
    assert_eq!(state.current_device.as_ref(), Some(&disconnected));
    assert!(state.pending.is_empty());
    assert_eq!(state.wifi_devices.len(), 3);
    assert_eq!(state.subscriptions.len(), 8);
    assert!(state.networking_enabled && state.wireless_hardware_enabled && state.wireless_enabled);
}

#[test]
fn root_active_ap_path_leaves_device_without_an_ap() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let (mut app, _peer) = setup();
    connected_device(&mut app, &path);
    read_active_ap(&mut app, &path, &ObjectPath::from("/"));
    assert_device(&app, &path, None);
    let state = app.resource::<NMState>();
    assert!(state.pending.is_empty());
    assert_eq!(state.wifi_devices.len(), 1);
    assert_eq!(state.subscriptions.len(), 2);
}

#[test]
fn malformed_or_failed_active_ap_reply_preserves_device_subscriptions() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let ap_path = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/10");
    for case in 0..4 {
        let (mut app, _peer) = setup();
        connected_device(&mut app, &path);
        let call = active_ap_call(&app, &path);
        let result = match case {
            0 => Ok(Message::method_return(call.serial).with_body(&(ap_path.clone(),))),
            1 => Ok(Message::method_return(call.serial)
                .with_body(&(Value::Str(ap_path.as_str().into()),))),
            2 => Err(CallError::Timeout),
            _ => Err(CallError::Remote {
                name: "org.freedesktop.DBus.Error.UnknownObject".into(),
                message: "device disappeared".into(),
            }),
        };
        reply(&mut app, call, result);
        assert_device(&app, &path, None);
        let state = app.resource::<NMState>();
        assert!(state.pending.is_empty());
        assert_eq!(state.wifi_devices.len(), 1);
        assert_eq!(state.subscriptions.len(), 2);
    }
}

#[test]
fn failed_device_discovery_does_not_discard_other_device_requests() {
    let failed = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let other = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/2");
    for case in 0..3 {
        let (mut app, _peer) = setup();
        discover(&mut app, &[failed.clone(), other.clone()]);
        let call = device_call(&app, &failed);
        let result = match case {
            0 => Ok(Message::method_return(call.serial).with_body(&(false,))),
            1 => Err(CallError::Timeout),
            _ => Err(CallError::Remote {
                name: "org.freedesktop.DBus.Error.UnknownObject".into(),
                message: "device disappeared".into(),
            }),
        };
        reply(&mut app, call, result);
        assert!(app.resource::<NMState>().wifi_devices.is_empty());
        assert!(app.resource::<NMState>().subscriptions.is_empty());
        assert_eq!(app.resource::<NMState>().pending.len(), 1);
        let call = device_call(&app, &other);
        reply_properties(&mut app, call, device_properties(2, 30));
        assert_device(&app, &other, None);
        let state = app.resource::<NMState>();
        assert!(state.pending.is_empty());
        assert_eq!(state.wifi_devices.len(), 1);
        assert!(!state.wifi_devices.contains_key(&failed));
        assert_eq!(state.subscriptions.len(), 2);
    }
}

#[test]
fn ap_properties_initialize_ssid_and_strength_independently() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let ap_path = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/10");
    let ssid_cases = [
        (Some(ssid_value(b"WiFi\xff\0")), b"WiFi\xff\0".to_vec()),
        (Some(ssid_value(b"")), Vec::new()),
        (None, Vec::new()),
        (Some(Value::Str("wrong wire type".into())), Vec::new()),
    ];
    let strength_cases = [
        (Some(Value::U8(0)), 0),
        (Some(Value::U8(73)), 73),
        (Some(Value::U8(100)), 100),
        (None, 0),
        (Some(Value::U32(73)), 0),
    ];
    for (ssid, expected_ssid) in ssid_cases {
        for (strength, expected_strength) in strength_cases.clone() {
            let (mut app, _peer) = setup();
            connected_device(&mut app, &path);
            read_active_ap(&mut app, &path, &ap_path);
            assert_device(&app, &path, Some((&ap_path, b"", 0)));
            assert_eq!(app.resource::<NMState>().pending.len(), 1);
            let call = ap_properties_call(&app, &path, &ap_path);
            let mut properties = PropMap::new();
            if let Some(ssid) = ssid.clone() {
                properties.insert("Ssid".into(), ssid);
            }
            if let Some(strength) = strength {
                properties.insert("Strength".into(), strength);
            }
            reply_properties(&mut app, call, properties);
            assert_device(
                &app,
                &path,
                Some((&ap_path, &expected_ssid, expected_strength)),
            );
            let state = app.resource::<NMState>();
            assert!(state.pending.is_empty());
            assert_eq!(state.wifi_devices.len(), 1);
            assert_eq!(state.subscriptions.len(), 3);
        }
    }
}

#[test]
fn failed_or_malformed_ap_properties_keep_initial_ap_defaults() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let ap_path = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/10");
    for case in 0..3 {
        let (mut app, _peer) = setup();
        connected_device(&mut app, &path);
        read_active_ap(&mut app, &path, &ap_path);
        let call = ap_properties_call(&app, &path, &ap_path);
        let result = match case {
            0 => Ok(Message::method_return(call.serial).with_body(&(false,))),
            1 => Err(CallError::Timeout),
            _ => Err(CallError::Remote {
                name: "org.freedesktop.DBus.Error.UnknownObject".into(),
                message: "access point disappeared".into(),
            }),
        };
        reply(&mut app, call, result);
        assert_device(&app, &path, Some((&ap_path, b"", 0)));
        let state = app.resource::<NMState>();
        assert!(state.pending.is_empty());
        assert_eq!(state.wifi_devices.len(), 1);
        assert_eq!(state.subscriptions.len(), 3);
    }
}

#[test]
fn device_removed_cleans_subscriptions_and_only_replaces_the_removed_selection() {
    let selected = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let other = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/2");
    let remaining = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/3");
    let other_ap = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/20");
    let remaining_ap = ObjectPath::from("/org/freedesktop/NetworkManager/AccessPoint/30");
    let (mut app, _peer) = setup();
    discover(
        &mut app,
        &[selected.clone(), other.clone(), remaining.clone()],
    );
    for (path, state) in [(&selected, 30), (&other, 100), (&remaining, 100)] {
        let call = device_call(&app, path);
        reply_properties(&mut app, call, device_properties(2, state));
    }
    read_active_ap(&mut app, &other, &other_ap);
    read_active_ap(&mut app, &remaining, &remaining_ap);
    let other_call = ap_properties_call(&app, &other, &other_ap);
    let remaining_call = ap_properties_call(&app, &remaining, &remaining_ap);

    remove_device(&mut app, &other);
    let state = app.resource::<NMState>();
    assert_eq!(state.current_device.as_ref(), Some(&selected));
    assert_eq!(state.wifi_devices.len(), 2);
    assert!(!state.wifi_devices.contains_key(&other));
    assert_eq!(state.subscriptions.len(), 5);
    assert_eq!(state.pending.len(), 1);
    assert!(state.pending.contains_key(&remaining_call));
    assert_device(&app, &selected, None);
    assert_device(&app, &remaining, Some((&remaining_ap, b"", 0)));

    app.signal(MethodReply {
        call: other_call,
        result: Ok(
            Message::method_return(other_call.serial).with_body(&(PropMap::from([
                ("Ssid".into(), ssid_value(b"Removed AP")),
                ("Strength".into(), Value::U8(100)),
            ]),)),
        ),
    });
    app.flush();
    assert!(!app.resource::<NMState>().wifi_devices.contains_key(&other));
    assert_device(&app, &remaining, Some((&remaining_ap, b"", 0)));

    remove_device(&mut app, &selected);
    let state = app.resource::<NMState>();
    assert_eq!(state.current_device.as_ref(), Some(&remaining));
    assert_eq!(state.wifi_devices.len(), 1);
    assert_eq!(state.subscriptions.len(), 3);
    assert_eq!(state.pending.len(), 1);
    assert_device(&app, &remaining, Some((&remaining_ap, b"", 0)));

    remove_device(&mut app, &remaining);
    let state = app.resource::<NMState>();
    assert!(state.current_device.is_none());
    assert!(state.wifi_devices.is_empty());
    assert!(state.subscriptions.is_empty());
    assert!(state.pending.is_empty());
    assert!(state.properties_changed.is_some());
    assert!(state.device_added.is_some());
    assert!(state.device_removed.is_some());
}

#[test]
fn device_removed_during_discovery_prevents_late_reply_from_recreating_it() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let (mut app, _peer) = setup();
    discover(&mut app, std::slice::from_ref(&path));
    let call = device_call(&app, &path);
    remove_device(&mut app, &path);
    assert!(app.resource::<NMState>().pending.is_empty());

    app.signal(MethodReply {
        call,
        result: Ok(Message::method_return(call.serial).with_body(&(device_properties(2, 100),))),
    });
    app.flush();
    let state = app.resource::<NMState>();
    assert!(state.current_device.is_none());
    assert!(state.wifi_devices.is_empty());
    assert!(state.subscriptions.is_empty());
    assert!(state.pending.is_empty());
}

#[test]
fn device_removed_clears_pending_active_ap_read() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    let (mut app, _peer) = setup();
    connected_device(&mut app, &path);
    let call = active_ap_call(&app, &path);
    remove_device(&mut app, &path);
    app.signal(MethodReply {
        call,
        result: Ok(
            Message::method_return(call.serial).with_body(&(Value::ObjectPath(ObjectPath::from(
                "/org/freedesktop/NetworkManager/AccessPoint/10",
            )),)),
        ),
    });
    app.flush();
    let state = app.resource::<NMState>();
    assert!(state.current_device.is_none());
    assert!(state.wifi_devices.is_empty());
    assert!(state.subscriptions.is_empty());
    assert!(state.pending.is_empty());
}

#[test]
fn device_added_uses_discovery_before_storing_or_selecting_wifi() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    for (device_type, device_state) in [(1, 100), (2, 30), (2, 100)] {
        let (mut app, _peer) = setup();
        discover(&mut app, &[]);
        add_device(&mut app, &path);
        let state = app.resource::<NMState>();
        assert!(state.wifi_devices.is_empty());
        assert!(state.current_device.is_none());
        assert!(state.subscriptions.is_empty());
        assert_eq!(state.pending.len(), 1);

        let call = device_call(&app, &path);
        reply_properties(&mut app, call, device_properties(device_type, device_state));
        let state = app.resource::<NMState>();
        if device_type == 2 {
            assert_device(&app, &path, None);
            assert_eq!(state.current_device.as_ref(), Some(&path));
            assert_eq!(state.wifi_devices.len(), 1);
            assert_eq!(state.subscriptions.len(), 2);
            assert_eq!(state.pending.len(), usize::from(device_state == 100));
            if device_state == 100 {
                active_ap_call(&app, &path);
            }
        } else {
            assert!(state.wifi_devices.is_empty());
            assert!(state.current_device.is_none());
            assert!(state.subscriptions.is_empty());
            assert!(state.pending.is_empty());
        }
    }
}

#[test]
fn device_added_overlapping_initial_discovery_keeps_one_device_and_subscription_set() {
    let path = ObjectPath::from("/org/freedesktop/NetworkManager/Devices/1");
    for added_reply_first in [true, false] {
        let (mut app, _peer) = setup();
        add_device(&mut app, &path);
        let added_call = device_call(&app, &path);
        let manager_call = pending_call(&app, |kind| matches!(kind, CallType::GetAll));
        reply_properties(
            &mut app,
            manager_call,
            manager_properties(std::slice::from_ref(&path)),
        );
        let initial_call = *app
            .resource::<NMState>()
            .pending
            .keys()
            .find(|call| **call != added_call)
            .unwrap();
        assert_eq!(app.resource::<NMState>().pending.len(), 2);
        let (first, second) = if added_reply_first {
            (added_call, initial_call)
        } else {
            (initial_call, added_call)
        };
        reply_properties(&mut app, first, device_properties(2, 100));
        let ap_call = active_ap_call(&app, &path);
        let state = app.resource::<NMState>();
        let device = &state.wifi_devices[&path];
        let matches = (device.state_changed, device.wireless_properties_changed);

        reply_properties(&mut app, second, device_properties(2, 100));
        assert_device(&app, &path, None);
        let state = app.resource::<NMState>();
        let device = &state.wifi_devices[&path];
        assert_eq!(
            matches,
            (device.state_changed, device.wireless_properties_changed)
        );
        assert_eq!(state.current_device.as_ref(), Some(&path));
        assert_eq!(state.wifi_devices.len(), 1);
        assert_eq!(state.subscriptions.len(), 2);
        assert_eq!(state.pending.len(), 1);
        assert!(state.pending.contains_key(&ap_call));
    }
}
