use super::DisplayModule;
use super::backend::DisplayStatus;
use super::interface::DISPLAY;
use mecha_wayland::prelude::*;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[test]
fn gaining_display_owner_refreshes_brightness() {
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
        .add_module(DBusModule::new().over(BusId::System, client));
    let mut auth = Vec::new();
    while !auth.ends_with(b"BEGIN\r\n") {
        let mut byte = [0];
        peer.read_exact(&mut byte).unwrap();
        auth.push(byte[0]);
    }
    app.add_module(DisplayModule);
    let initial = app.resource::<DisplayStatus>().read.unwrap();
    let owner_changed = app.resource::<DisplayStatus>().owner_changed;
    let signal = |new_owner: &str| SignalReceived {
        bus: BusId::System,
        matches: [owner_changed].into_iter().collect(),
        msg: Message::signal(
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
        )
        .with_body(&(DISPLAY, ":1.2", new_owner)),
    };

    app.signal(signal(""));
    app.flush();
    assert_eq!(app.resource::<DisplayStatus>().read, None);

    app.signal(signal(":1.3"));
    app.flush();
    assert_ne!(app.resource::<DisplayStatus>().read, Some(initial));
    assert!(app.resource::<DisplayStatus>().read.is_some());
}
