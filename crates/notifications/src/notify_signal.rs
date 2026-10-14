use crate::card::CardBuilder;
use crate::card_list::CardList;
use crate::notification_store::NotificationStore;
use mecha_wayland::prelude::*;

pub enum NotificationSignal {
    AddNotification(u32),
    RemoveNotification(u32),
    UpdateNotification(u32),
}

pub fn notify_signal(app: &mut App, notify_signal: &NotificationSignal) {
    match notify_signal {
        NotificationSignal::AddNotification(id) => {
            let Some((list, _)) = app.widgets::<CardList>().next() else {
                return;
            };
            let Some(entry) = app
                .resource::<NotificationStore>()
                .entries
                .iter()
                .find(|e| e.id == *id)
                .cloned()
            else {
                return;
            };
            app.spawn(list, CardBuilder { entry });
        }
        NotificationSignal::UpdateNotification(id) => {
            todo!()
        }
        NotificationSignal::RemoveNotification(id) => {
            todo!()
        }
    }
}

impl Signal for NotificationSignal {}
