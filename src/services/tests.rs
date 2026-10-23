use super::*;
use std::io::Write;
use std::os::unix::net::UnixStream;

fn setup() -> (App, UnixStream) {
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
    (app, peer)
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
