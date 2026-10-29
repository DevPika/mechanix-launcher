//! Display service D-Bus names and match rules.
use mecha_wayland::prelude::*;

pub(super) const DISPLAY: &str = "org.mechanix.services.Display";
pub(super) const PATH: &str = "/org/mechanix/services/Display";
pub(super) const INTERFACE: &str = "org.mechanix.services.Display";

pub(super) fn owner_changed() -> MatchRule {
    MatchRule::signal()
        .sender("org.freedesktop.DBus")
        .interface("org.freedesktop.DBus")
        .member("NameOwnerChanged")
        .arg(0, DISPLAY)
}
