use mecha_wayland::{
    dbus::{ExportId, MethodCallReceived},
    prelude::*,
};
use std::env;

use crate::dbus_spec;

pub struct NotificationServer {
    pub export_id: ExportId,
}

impl Resource for NotificationServer {}

pub fn on_call(app: &mut App, e: &MethodCallReceived) {
    if e.export != app.resource::<NotificationServer>().export_id {
        return;
    }

    match e.call.member().as_str() {
        "GetServerInformation" => {
            app.resource_mut::<DBus>().reply(&e.call, &capabilities());
            return;
        }
        _ => return,
    };
}

fn capabilities() -> (&'static str, &'static str, &'static str, &'static str) {
    (
        dbus_spec::SERVICE_NAME,
        dbus_spec::VENDOR,
        env!("CARGO_PKG_VERSION"),
        dbus_spec::SPEC_VERSION,
    )
}
