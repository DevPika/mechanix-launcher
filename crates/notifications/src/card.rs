use mecha_wayland::mechanix_widgets::prelude::TextContextExt;
use mecha_wayland::mechanix_widgets::{self as mw, button};
use mecha_wayland::prelude::*;
use std::time::SystemTime;

use crate::dbus_spec::NOTIFIFICATION_DISMISSED;
use crate::notification_store::Entry;
use crate::notify_signal::NotificationSignal::RemoveNotification;
use crate::style::{color, shape, size, space, text};

pub struct CardBuilder {
    pub entry: Entry,
}

pub struct Card {
    pub notification_id: u32,
}

pub struct UpdateCard(pub Entry);
impl Event for UpdateCard {}

impl Build for CardBuilder {
    type Widget = Card;
}

impl Widget for Card {
    type Builder = CardBuilder;

    fn build(b: CardBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .row()
            .align_items(Align::Center)
            .size(px(size::CARD_WIDTH), px(size::CARD_HEIGHT))
            .padding(Insets::new(
                px(0.0),
                px(space::CARD_X.dp()),
                px(0.0),
                px(space::CARD_X.dp()),
            ))
            .gap(px(space::ICON_TO_TEXT.dp()));
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(
            Quad::new(s.color(color::CARD))
                .radius(shape::CARD.resolve_radius_dp(size::CARD_HEIGHT)),
        );

        // TODO: Load icons based on the categories
        s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .size(px(size::ICON), px(size::ICON))
                    .shrink(0.0),
            ),
        );

        let column = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .column()
                    .gap(px(space::TITLE_TO_BODY.dp()))
                    .grow(1.0)
                    .shrink(1.0)
                    .min_width(px(0.0)),
            ),
        );

        let title_row = s.spawn(
            column,
            div().style(
                LayoutStyle::default()
                    .row()
                    .justify(Justify::SpaceBetween)
                    .align_items(Align::End)
                    .gap(px(space::TITLE_TO_BODY.dp())),
            ),
        );
        let close_btn = s.spawn(title_row, button().label("close"));
        let id = b.entry.id;
        s.on::<Clicked>(close_btn, move |ctx, _| {
            close_notification(ctx, id);
        });
        let summary = s.spawn(
            title_row,
            mw::text(b.entry.notification.summary)
                .variant(text::CARD_TITLE.variant)
                .color(text::CARD_TITLE.color)
                .emphasised()
                .max_lines(1)
                .overflow(TextOverflow::Ellipsis),
        );

        let time = s.spawn(
            title_row,
            mw::text(ago(b.entry.received_time))
                .variant(text::CARD_TIME.variant)
                .color(text::CARD_TIME.color),
        );

        let body = s.spawn(
            column,
            mw::text(b.entry.notification.body)
                .variant(text::CARD_BODY.variant)
                .color(text::CARD_BODY.color)
                .max_lines(1)
                .overflow(TextOverflow::Ellipsis),
        );

        s.on::<UpdateCard>(me, move |ctx, e| {
            let entry = &e.0;
            ctx.at(summary)
                .unwrap()
                .set_text(entry.notification.summary.clone());
            ctx.at(body)
                .unwrap()
                .set_text(entry.notification.body.clone());
            ctx.at(time).unwrap().set_text(ago(entry.received_time));
        });

        Card {
            notification_id: b.entry.id,
        }
    }
}

// TODO: It needs to update each minute
// currently until a new notification arrives,
// the card won't update the time
fn ago(t: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(t)
        .unwrap_or_default()
        .as_secs();
    match secs {
        0..60 => "now".into(),
        60..3_600 => format!("{} m", secs / 60),
        3_600..86_400 => format!("{} h", secs / 3_600),
        _ => format!("{} d", secs / 86_400),
    }
}

fn close_notification(ctx: &mut Context<Card>, id: u32) {
    ctx.signal(RemoveNotification(id, NOTIFIFICATION_DISMISSED));
}
