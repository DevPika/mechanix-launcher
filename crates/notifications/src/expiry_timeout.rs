use crate::{
    NotificationStore, dbus_spec::NOTIFICATIONS_EXPIRY,
    notify_signal::NotificationSignal::RemoveNotification,
};
use mecha_wayland::prelude::{App, IoEvent, Resource, Ring, Token};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct ExpiryTimers(HashSet<Token>);

impl Resource for ExpiryTimers {}

pub fn schedule_expire(app: &mut App, notification_id: u32, expiry_timeout: i32) {
    let token = (expiry_timeout > 0).then(|| {
        let token = app
            .resource_mut::<Ring>()
            .timeout(Duration::from_millis(expiry_timeout as u64));
        app.resource_mut::<ExpiryTimers>().0.insert(token);
        token
    });

    let mut notifications_store = app.resource_mut::<NotificationStore>();
    if let Some(entry) = notifications_store
        .entries
        .iter_mut()
        .find(|e| e.id == notification_id)
    {
        entry.expiry_timeout = token;
    }
}

pub fn notification_time_expired(app: &mut App, io_event: &IoEvent) {
    if !app.resource_mut::<ExpiryTimers>().0.remove(&io_event.token) {
        return;
    }
    app.resource_mut::<Ring>().finish(io_event.token);
    let id = app
        .resource::<NotificationStore>()
        .entries
        .iter()
        .find(|e| e.expiry_timeout == Some(io_event.token))
        .map(|e| e.id);
    if let Some(id) = id {
        app.signal(RemoveNotification(id, NOTIFICATIONS_EXPIRY));
    }
}
