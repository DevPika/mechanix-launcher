use crate::notification_store::{Entry, NotificationStore};
use mecha_wayland::prelude::*;
use std::time::SystemTime;

#[derive(Default, Debug, Clone)]
pub struct Notification {
    pub desktop_entry: String,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<(String, String)>,
    pub expire_timeout: u32,
    pub hints: Categories,
}

#[derive(Default, Debug, Clone)]
pub enum Categories {
    #[default]
    LOW,
    NORMAL,
    CRITICAL,
}

pub struct Notify {
    pub replaces_id: Option<u32>,
    pub notification: Notification,
}

impl Signal for Notify {}

pub fn append_notifications(app: &mut App, notify: &Notify) {
    let mut notifications_store = app.resource_mut::<NotificationStore>();
    let entry = Entry {
        app_id: (notifications_store.entries.len() + 1) as u32,
        received_time: SystemTime::now(),
        notification: notify.notification.clone(),
    };
    notifications_store.entries.push(entry);
}
