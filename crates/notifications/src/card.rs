use mecha_wayland::mechanix_widgets::{self as mw};
use mecha_wayland::prelude::*;
use std::time::SystemTime;

use crate::notification_store::Entry;
use crate::style::{color, shape, size, space, text};

pub struct CardBuilder {
    pub entry: Entry,
}

pub struct Card {
    pub notification_id: u32,
}

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
        s.spawn(
            title_row,
            mw::text(b.entry.notification.summary)
                .variant(text::CARD_TITLE.variant)
                .color(text::CARD_TITLE.color)
                .emphasised()
                .max_lines(1)
                .overflow(TextOverflow::Ellipsis),
        );

        s.spawn(
            title_row,
            mw::text(ago(b.entry.received_time))
                .variant(text::CARD_TIME.variant)
                .color(text::CARD_TIME.color),
        );

        s.spawn(
            column,
            mw::text(b.entry.notification.body)
                .variant(text::CARD_BODY.variant)
                .color(text::CARD_BODY.color)
                .max_lines(1)
                .overflow(TextOverflow::Ellipsis),
        );

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
