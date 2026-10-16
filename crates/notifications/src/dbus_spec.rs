use mecha_wayland::prelude::*;

pub const NAME: &str = "org.freedesktop.Notifications";
pub const OBJECT: &str = "/org/freedesktop/Notifications";
pub const INTERFACE: &str = "org.freedesktop.Notifications";
pub const SPEC_VERSION: &str = "1.3";

pub const VENDOR: &str = "Mecha";
pub const SERVICE_NAME: &str = "Mecha Notifications";

pub const NOTIFICATIONS_CLOSED: u32 = 3;
pub const NOTIFIFICATION_DISMISSED: u32 = 2;

fn build_bus_interface_spec() -> InterfaceSpec {
    InterfaceSpec::new(INTERFACE)
        .method("GetCapabilities", "", "as")
        .method("Notify", "susssasa{sv}i", "u")
        .method("CloseNotification", "u", "")
        .method("GetServerInformation", "", "ssss")
        .signal("NotificationClosed", "uu")
        .signal("ActionInvoked", "us")
        .signal("ActivationToken", "us")
}

pub fn register_notifications_interface(app: &mut App) -> ExportId {
    let spec = build_bus_interface_spec();
    let mut dbus = app.resource_mut::<DBus>();
    let mut bus = dbus.session();
    bus.request_name(NAME, NameFlags::DO_NOT_QUEUE);
    bus.export(OBJECT, spec).expect("export /mecha/Counter")
}
