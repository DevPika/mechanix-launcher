use mecha_wayland::prelude::*;
use screen::Screen;

pub mod card;
pub mod card_list;
pub mod dbus;
pub mod dbus_spec;
pub mod expiry_timeout;
pub mod markup;
pub mod module;
pub mod notification_store;
pub mod notifications;
pub mod notify;
pub mod notify_signal;
pub mod rich_text;
pub mod style;

use crate::notifications::NotificationsCenterBuilder;
pub use module::NotificationsModule;

pub struct Notifications;

impl Screen for Notifications {
    fn build(&self, app: &mut App, parent: NodeId) -> NodeId {
        app.spawn(parent, NotificationsCenterBuilder).id()
    }
}
