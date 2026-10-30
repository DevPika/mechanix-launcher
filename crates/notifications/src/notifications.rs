use crate::card_list::CardListBuilder;
use crate::notify_signal::NotificationSignal;
use crate::style::text;
use mecha_wayland::mechanix_widgets::{self as mw, button};
use mecha_wayland::prelude::*;

pub struct NotificationsCenter;
pub struct NotificationsCenterBuilder;

impl Build for NotificationsCenterBuilder {
    type Widget = NotificationsCenter;
}

impl Widget for NotificationsCenter {
    type Builder = NotificationsCenterBuilder;
    fn build(_b: NotificationsCenterBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .column()
            .width(percent(100.0))
            .height(percent(100.0));

        let header = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .row()
                    .justify(Justify::SpaceBetween)
                    .align_items(Align::Center)
                    .width(percent(1.0)),
            ),
        );
        s.spawn(
            header,
            mw::text("Notifications")
                .variant(text::SCREEN_TITLE.variant)
                .color(text::SCREEN_TITLE.color),
        );
        let close_all_notifications = s.spawn(header, button().label("close all"));
        s.on::<Clicked>(close_all_notifications, move |ctx, _| {
            ctx.signal(NotificationSignal::CloseAllNotifications)
        });
        s.spawn(me, CardListBuilder);

        NotificationsCenter
    }
}
