use mecha_wayland::prelude::*;

use crate::util::full_width;

mod svg {
    pub const POWER: &[u8] = include_bytes!("../assets/power.svg");
}

pub const HEADER_HEIGHT: f32 = 80.0;
const DATE: &str = "Today, 19 Oct";
const POWER_ICON_SIZE: u32 = 26;

pub struct Header;

pub struct HeaderBuilder {
    pub font: FontId,
}

impl Build for HeaderBuilder {
    type Widget = Header;
}

impl Widget for Header {
    type Builder = HeaderBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = full_width(HEADER_HEIGHT)
            .row()
            .align_items(Align::Center)
            .justify(Justify::SpaceBetween)
            .padding(Insets::new(px(0.0), percent(4.0), px(0.0), percent(4.0)));

        s.spawn(
            me,
            text(b.font, DATE)
                .size(24)
                .color(s.color(ColorRole::OnSurfaceVariant)),
        );

        let bitmap = Bitmap::from_svg(svg::POWER, POWER_ICON_SIZE)
            .expect("embedded power icon should render");
        let sprite = s
            .resource_mut::<Atlas>()
            .insert(Class::Icon, &bitmap)
            .expect("power icon should fit in the atlas");

        let power = s.spawn(
            me,
            button().style(
                LayoutStyle::default()
                    .height(px(44.0))
                    .row()
                    .center()
                    .gap(px(8.0))
                    .shrink(0.0),
            ),
        );
        s.spawn(power, icon(sprite).color(s.color(ColorRole::OnSurface)));
        s.spawn(
            power,
            text(b.font, "POWER")
                .size(20)
                .color(s.color(ColorRole::OnSurface)),
        );

        Header
    }
}
