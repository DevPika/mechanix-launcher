use mecha_wayland::prelude::*;

use crate::{
    card::CardBuilder,
    notification_store::{Entry, NotificationStore},
    style::space,
};

pub struct CardListBuilder;

pub struct CardList;

impl Build for CardListBuilder {
    type Widget = CardList;
}

impl Widget for CardList {
    type Builder = CardListBuilder;

    fn build(_: CardListBuilder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .column()
            .width(percent(1.0))
            .gap(px(space::CARD_GAP.dp()));
        CardList
    }
}

pub fn notifications_changed(app: &mut App, _: &OnChanged<NotificationStore>) {
    let Some((list, _)) = app.widgets::<CardList>().next() else {
        return;
    };

    let old = app
        .children(list)
        .map(<[NodeId]>::to_vec)
        .unwrap_or_default();

    for card in old {
        app.remove(card);
    }

    let entries: Vec<Entry> = app
        .resource::<NotificationStore>()
        .entries
        .iter()
        .rev()
        .cloned()
        .collect();
    for entry in entries {
        app.spawn(list, CardBuilder { entry });
    }
}
