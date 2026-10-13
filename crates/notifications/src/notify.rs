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
    pub expire_timeout: i32,
    pub hints: Categories,
}

#[derive(Default, Debug, Clone)]
pub enum Categories {
    #[default]
    LOW,
    NORMAL,
    CRITICAL,
}

#[derive(Debug)]
pub struct Notify {
    pub replaces_id: Option<u32>,
    pub notification: Notification,
}

impl Signal for Notify {}

pub type NotifyArgs = (
    String,
    u32,
    String,
    String,
    String,
    Vec<String>,
    PropMap,
    i32,
);

impl From<NotifyArgs> for Notify {
    fn from(args: NotifyArgs) -> Self {
        let (app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout) = args;

        let replaces_id = (replaces_id != 0).then_some(replaces_id);

        let actions = actions
            .chunks_exact(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect();

        let urgency = match hints.get_as::<u8>("urgency") {
            Some(Ok(0)) => Categories::LOW,
            Some(Ok(2)) => Categories::CRITICAL,
            _ => Categories::NORMAL,
        };
        let desktop_entry = hints
            .get_as::<String>("desktop-entry")
            .and_then(Result::ok)
            .unwrap_or_default();

        Notify {
            replaces_id,
            notification: Notification {
                desktop_entry,
                app_name,
                app_icon,
                summary,
                body,
                actions,
                expire_timeout,
                hints: urgency,
            },
        }
    }
}

pub fn append_notifications(app: &mut App, notify: &Notify) -> u32 {
    let mut notifications_store = app.resource_mut::<NotificationStore>();
    notifications_store.next_id += 1;
    let id = notifications_store.next_id;
    let entry = Entry {
        id,
        received_time: SystemTime::now(),
        notification: notify.notification.clone(),
    };
    notifications_store.entries.push(entry);
    return id;
}
