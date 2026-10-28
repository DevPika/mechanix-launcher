use mecha_wayland::mechanix_widgets::prelude::{ButtonVariant, button, text};
use mecha_wayland::prelude::*;

use crate::util::full_width;

mod svg {
    pub const POWER: &[u8] = include_bytes!("../assets/power.svg");
}

pub const HEADER_HEIGHT: f32 = 80.0;
const DATE: &str = "Today,19 Oct";
const POWER_ICON_SIZE: u32 = 20;

pub struct Header;

pub struct HeaderBuilder;

impl Build for HeaderBuilder {
    type Widget = Header;
}

impl Widget for Header {
    type Builder = HeaderBuilder;

    fn build(_: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(HEADER_HEIGHT)
            .row()
            .align_items(Align::Center)
            .justify(Justify::SpaceBetween)
            .padding(Insets::new(px(0.0), px(20.0), px(0.0), px(20.0)));

        s.spawn(
            me,
            text(DATE)
                .variant(TextVariant::TitleSmall)
                .color(ColorRole::OnSurfaceVariant),
        );

        let bitmap = Bitmap::from_svg(svg::POWER, POWER_ICON_SIZE)
            .expect("embedded power icon should render");
        let sprite = s
            .resource_mut::<Atlas>()
            .insert(Class::Icon, &bitmap)
            .expect("power icon should fit in the atlas");

        s.spawn(
            me,
            button()
                .variant(ButtonVariant::Text)
                .icon(sprite)
                .label("POWER")
                .icon_gap(px(10.0))
                .hover_background(Color::TRANSPARENT)
                .pressed_background(Color::TRANSPARENT)
                .height(px(32.))
                .text_color(ColorRole::OnSurface),
        );

        Header
    }
}
