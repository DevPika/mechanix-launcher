use crate::card::{Card, CardBuilder, UpdateCard};
use crate::card_list::CardList;
use crate::notification_store::NotificationStore;
use mecha_wayland::prelude::*;

pub enum NotificationSignal {
    AddNotification(u32),
    RemoveNotification(u32),
    UpdateNotification(u32),
    CloseAllNotifications,
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
            let Some((card, _)) = app
                .widgets::<Card>()
                .find(|(_, card)| card.notification_id == *id)
            else {
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
            app.emit(UpdateCard(entry), card);
        }
        NotificationSignal::RemoveNotification(id) => {
            app.resource_mut::<NotificationStore>()
                .entries
                .retain(|e| e.id != *id);
            let Some((card, _)) = app
                .widgets::<Card>()
                .find(|(_, card)| card.notification_id == *id)
            else {
                return;
            };
            app.remove(card);
        }
        NotificationSignal::CloseAllNotifications => {
            app.resource_mut::<NotificationStore>().entries.clear();
            let Some((list, _)) = app.widgets::<CardList>().next() else {
                return;
            };
            let cards = app
                .children(list)
                .map(<[NodeId]>::to_vec)
                .unwrap_or_default();
            for card in cards {
                app.remove(card);
            }
        }
    }
}

impl Signal for NotificationSignal {}
