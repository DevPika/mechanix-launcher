use crate::card_list::CardListBuilder;
use crate::mock;
use crate::notify::Notify;
use crate::style::{color, size, space, text};
use mecha_wayland::mechanix_widgets::{self as mw};
use mecha_wayland::prelude::*;

pub struct Panel;
pub struct PanelBuilder {
    pub root: NodeId,
}

impl Build for PanelBuilder {
    type Widget = Panel;
}

impl Widget for Panel {
    type Builder = PanelBuilder;
    fn build(b: PanelBuilder, _me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        let role = Role::Layer(LayerRole {
            layer: Layer::Top,
            anchor: Anchor::BOTTOM,
            exclusive_zone: 0,
            namespace: "notifications".into(),
            keyboard_interactivity: KeyboardInteractivity::None,
        });

        let win = s.spawn_with(
            b.root,
            window().clear(s.color(color::SCREEN)).layout(
                LayoutStyle::default()
                    .column()
                    .size(px(size::PANEL_WIDTH), px(size::PANEL_HEIGHT))
                    .padding(Insets::new(
                        px(space::SCREEN_TOP.dp()),
                        px(space::SCREEN_X.dp()),
                        px(0.0),
                        px(space::SCREEN_X.dp()),
                    ))
                    .gap(px(space::HEADER_TO_LIST.dp())),
            ),
            (role,),
        );
        s.on::<CloseRequested>(win, |ctx, _| ctx.signal(Stop));

        // This is Mock data
        // should be handled whenever dbus gets merged
        let mut mocks = mock::notifications().into_iter().cycle();
        s.on::<Clicked>(win, move |ctx, _| {
            if let Some(notification) = mocks.next() {
                ctx.signal(Notify {
                    replaces_id: None,
                    notification,
                });
            }
        });

        let header = s.spawn(
            win,
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

        s.spawn(win, CardListBuilder);

        Panel
    }
}
